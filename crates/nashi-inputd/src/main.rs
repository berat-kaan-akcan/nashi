//! nashi-inputd: detects "drag + modifier" chords from evdev and announces
//! them on the session bus (`docs/DBUS.md`). Never logs key codes or text.

mod chord;
mod config;
mod dbus;
mod device;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use zbus::fdo::{RequestNameFlags, RequestNameReply};

use crate::config::Loaded;
use crate::dbus::{InputIface, Shared, BUS_NAME, OBJECT_PATH};

const USAGE: &str = "\
Usage: nashi-inputd [OPTIONS]

Detects drag + modifier chords from /dev/input and announces them on the
session bus as io.github.beratkaanakcan.Nashi.Input.

Options:
  --config PATH  Use this config file instead of
                 $XDG_CONFIG_HOME/nashi/inputd.toml
  --check        List input devices and permission status, then exit
  --version      Print the version and exit
  -h, --help     Show this help

Logging: NASHI_INPUTD_LOG=debug (or RUST_LOG) shows drag/chord state changes.";

struct Args {
    config: Option<PathBuf>,
    check: bool,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        config: None,
        check: false,
    };
    let mut it = std::env::args_os().skip(1);
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("--version" | "-V") => {
                println!("nashi-inputd {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            Some("--help" | "-h") => {
                println!("{USAGE}");
                return Ok(None);
            }
            Some("--check") => args.check = true,
            Some("--config") => {
                let p = it.next().ok_or("--config needs a path")?;
                args.config = Some(PathBuf::from(p));
            }
            Some(s) if s.starts_with("--config=") => {
                args.config = Some(PathBuf::from(&s["--config=".len()..]));
            }
            _ => return Err(format!("unknown argument: {}", a.to_string_lossy())),
        }
    }
    Ok(Some(args))
}

fn check() -> ExitCode {
    // Ignore EPIPE (e.g. `--check | head`) instead of panicking.
    let mut o = std::io::stdout().lock();
    macro_rules! out {
        ($($t:tt)*) => {
            {
                let _ = writeln!(o, $($t)*);
            }
        };
    }
    out!("nashi-inputd {} device check", env!("CARGO_PKG_VERSION"));
    let nodes = match device::list_event_nodes(std::path::Path::new(device::INPUT_DIR)) {
        Ok(n) => n,
        Err(e) => {
            out!("cannot list {}: {e}", device::INPUT_DIR);
            return ExitCode::FAILURE;
        }
    };
    let (mut pointers, mut keyboards, mut denied) = (0, 0, 0);
    for p in nodes {
        let probe = device::probe(&p);
        let path = probe.path.display();
        match probe.result {
            Ok(caps) => {
                pointers += usize::from(caps.pointer);
                keyboards += usize::from(caps.keyboard);
                out!(
                    "  {path:<22} ok        {:<16} {}",
                    caps.describe(),
                    probe.name
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                denied += 1;
                out!("  {path:<22} DENIED    {:<16} {}", "-", probe.name);
            }
            Err(e) => out!("  {path:<22} error     {e}"),
        }
    }
    out!("readable: {pointers} pointer device(s), {keyboards} keyboard(s); {denied} permission denied");
    if denied > 0 {
        out!("hint: {}", device::PERMISSION_HINT);
    }
    if pointers > 0 && keyboards > 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn load_initial(path: Option<&std::path::Path>) -> config::Config {
    let Some(path) = path else {
        log::warn!("cannot determine config directory (no HOME); using defaults");
        return config::Config::default();
    };
    match config::load(path) {
        Loaded::Ok(c) => {
            log::info!("config: {}", path.display());
            c
        }
        Loaded::Missing => {
            log::info!("no config at {}; using defaults", path.display());
            config::Config::default()
        }
        Loaded::Invalid(e) => {
            log::warn!("invalid config {}: {e}; using defaults", path.display());
            config::Config::default()
        }
    }
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::new().filter_or(
        "NASHI_INPUTD_LOG",
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
    ))
    .format_timestamp(None)
    .init();

    let args = match parse_args() {
        Ok(Some(a)) => a,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nashi-inputd: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if args.check {
        return check();
    }

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("cannot start runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match rt.block_on(run(args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<(), String> {
    let config_path = args.config.or_else(config::default_path);
    let cfg = load_initial(config_path.as_deref());

    let (out_tx, mut out_rx) = mpsc::unbounded_channel();
    let shared = Arc::new(Shared::new(cfg, config_path, out_tx));

    let conn = zbus::connection::Builder::session()
        .and_then(|b| {
            b.serve_at(
                OBJECT_PATH,
                InputIface {
                    shared: Arc::clone(&shared),
                },
            )
        })
        .map_err(|e| format!("session bus: {e}"))?
        .build()
        .await
        .map_err(|e| format!("cannot connect to the session bus: {e}"))?;
    match conn
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
        .await
        .map_err(|e| format!("cannot own {BUS_NAME}: {e}"))?
    {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
        _ => {
            return Err(format!(
                "{BUS_NAME} is already owned; is nashi-inputd running?"
            ))
        }
    }
    log::info!("serving {BUS_NAME} at {OBJECT_PATH}");
    let iface = conn
        .object_server()
        .interface::<_, InputIface>(OBJECT_PATH)
        .await
        .map_err(|e| e.to_string())?;

    // Signal emitter: preserves the order produced by the state machine.
    let emitter_iface = iface.clone();
    let emitter = tokio::spawn(async move {
        while let Some(o) = out_rx.recv().await {
            if let Err(e) = dbus::emit(emitter_iface.signal_emitter(), o).await {
                log::warn!("cannot emit signal: {e}");
            }
        }
    });

    let (in_tx, mut in_rx) = mpsc::unbounded_channel();
    let manager = device::Manager::new(in_tx);
    tokio::spawn(manager.run());

    let mut sighup = signal(SignalKind::hangup()).map_err(|e| e.to_string())?;
    let mut sigterm = signal(SignalKind::terminate()).map_err(|e| e.to_string())?;
    let mut sigint = signal(SignalKind::interrupt()).map_err(|e| e.to_string())?;

    loop {
        let deadline = shared.next_deadline();
        let sleep_until = tokio::time::Instant::from_std(
            shared.epoch() + Duration::from_millis(deadline.unwrap_or(0)),
        );
        tokio::select! {
            input = in_rx.recv() => match input {
                Some(i) => shared.feed(i),
                None => break,
            },
            _ = tokio::time::sleep_until(sleep_until), if deadline.is_some() => shared.tick(),
            _ = sighup.recv() => {
                let before = shared.enabled();
                let _ = shared.reload();
                if shared.enabled() != before {
                    if let Err(e) = iface.get().await.enabled_changed(iface.signal_emitter()).await {
                        log::warn!("cannot emit PropertiesChanged: {e}");
                    }
                }
            }
            _ = sigterm.recv() => break,
            _ = sigint.recv() => break,
        }
    }

    // Do not leave the UI with a stuck overlay: end an active drag first.
    let was_dragging = shared.state().0;
    shared.set_enabled(false);
    if was_dragging {
        // The emitter never sees its channel close (the interface in the
        // object server holds a sender), so give it a moment to flush.
        let _ = tokio::time::timeout(Duration::from_millis(200), emitter).await;
    }
    log::info!("exiting");
    Ok(())
}

//! Session bus service, see `docs/DBUS.md`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::mpsc::UnboundedSender;
use zbus::object_server::SignalEmitter;
use zbus::{fdo, interface};

use crate::chord::{Input, Machine, Output};
use crate::config::{self, Config, Loaded};

pub const BUS_NAME: &str = "io.github.beratkaanakcan.Nashi.Input";
pub const OBJECT_PATH: &str = "/io/github/beratkaanakcan/Nashi/Input";
#[cfg(test)]
pub const INTERFACE: &str = "io.github.beratkaanakcan.Nashi.Input1";

/// State shared by the event loop and the D-Bus interface. Outputs are pushed
/// to `out` while the machine lock is held, so signal order is preserved.
pub struct Shared {
    machine: Mutex<Machine>,
    out: UnboundedSender<Output>,
    config_path: Option<PathBuf>,
    epoch: Instant,
}

fn log_output(o: Output) {
    match o {
        Output::DragStarted => log::debug!("drag started"),
        Output::ChordStarted(m) => log::debug!("chord started: {}", m.as_str()),
        Output::ChordReleased => log::debug!("chord released"),
        Output::DragEnded => log::debug!("drag ended"),
    }
}

impl Shared {
    pub fn new(cfg: Config, config_path: Option<PathBuf>, out: UnboundedSender<Output>) -> Self {
        Shared {
            machine: Mutex::new(Machine::new(cfg.settings, cfg.enabled)),
            out,
            config_path,
            epoch: Instant::now(),
        }
    }

    pub fn epoch(&self) -> Instant {
        self.epoch
    }

    pub fn now_ms(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn with_machine(&self, f: impl FnOnce(&mut Machine, u64) -> Vec<Output>) {
        let now = self.now_ms();
        let mut m = self.machine.lock().unwrap();
        for o in f(&mut m, now) {
            log_output(o);
            let _ = self.out.send(o);
        }
    }

    pub fn feed(&self, input: Input) {
        self.with_machine(|m, now| m.handle(now, input));
    }

    pub fn tick(&self) {
        self.with_machine(|m, now| m.tick(now));
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.machine.lock().unwrap().next_deadline()
    }

    pub fn state(&self) -> (bool, &'static str) {
        self.machine.lock().unwrap().state()
    }

    pub fn enabled(&self) -> bool {
        self.machine.lock().unwrap().enabled()
    }

    pub fn set_enabled(&self, enabled: bool) {
        if self.enabled() != enabled {
            log::info!("{}", if enabled { "enabled" } else { "disabled" });
        }
        self.with_machine(|m, _| m.set_enabled(enabled));
    }

    /// Re-read the config file. Missing file → defaults; invalid file →
    /// warning, previous settings kept. Returns `Err` (with the reason) when
    /// the file was invalid.
    pub fn reload(&self) -> Result<(), String> {
        let cfg = match &self.config_path {
            None => Config::default(),
            Some(p) => match config::load(p) {
                Loaded::Ok(c) => c,
                Loaded::Missing => Config::default(),
                Loaded::Invalid(e) => {
                    log::warn!(
                        "invalid config {}: {e}; keeping previous settings",
                        p.display()
                    );
                    return Err(e);
                }
            },
        };
        log::info!("config reloaded");
        self.with_machine(|m, now| {
            let mut out = m.apply_settings(now, cfg.settings);
            out.extend(m.set_enabled(cfg.enabled));
            out
        });
        Ok(())
    }
}

pub struct InputIface {
    pub shared: Arc<Shared>,
}

#[interface(name = "io.github.beratkaanakcan.Nashi.Input1")]
impl InputIface {
    /// `(dragging, mode)`; mode is empty when no chord is active.
    fn get_state(&self) -> (bool, String) {
        let (d, m) = self.shared.state();
        (d, m.to_owned())
    }

    /// Re-read the config file. Fails (org.freedesktop.DBus.Error.Failed)
    /// when the file is invalid; the previous settings stay active then.
    async fn reload(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> fdo::Result<()> {
        let before = self.shared.enabled();
        let res = self.shared.reload();
        if self.shared.enabled() != before {
            self.enabled_changed(&emitter).await?;
        }
        res.map_err(|e| fdo::Error::Failed(format!("invalid config: {e}")))
    }

    /// Runtime switch; not written back to the config file. `Reload()` resets
    /// it to the config value.
    #[zbus(property)]
    fn enabled(&self) -> bool {
        self.shared.enabled()
    }

    #[zbus(property)]
    fn set_enabled(&mut self, enabled: bool) {
        self.shared.set_enabled(enabled);
    }

    #[zbus(property)]
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    #[zbus(signal)]
    pub async fn drag_started(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn chord_started(emitter: &SignalEmitter<'_>, mode: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn chord_released(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn drag_ended(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

pub async fn emit(emitter: &SignalEmitter<'_>, o: Output) -> zbus::Result<()> {
    match o {
        Output::DragStarted => InputIface::drag_started(emitter).await,
        Output::ChordStarted(m) => InputIface::chord_started(emitter, m.as_str()).await,
        Output::ChordReleased => InputIface::chord_released(emitter).await,
        Output::DragEnded => InputIface::drag_ended(emitter).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::Mode;

    #[test]
    fn interface_name_matches_contract() {
        use zbus::object_server::Interface;
        assert_eq!(InputIface::name().as_str(), INTERFACE);
        assert_eq!(BUS_NAME, "io.github.beratkaanakcan.Nashi.Input");
        assert_eq!(OBJECT_PATH, "/io/github/beratkaanakcan/Nashi/Input");
    }

    #[test]
    fn reload_invalid_keeps_previous_and_missing_gives_defaults() {
        let dir = std::env::temp_dir().join(format!("nashi-inputd-dbus-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("inputd.toml");
        std::fs::write(&path, "enabled = false\n[chords]\nshift = \"tools\"\n").unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let shared = Shared::new(Config::default(), Some(path.clone()), tx);
        shared.reload().unwrap();
        assert!(!shared.enabled());
        assert_eq!(
            shared.machine.lock().unwrap().settings().shift,
            Some(Mode::Tools)
        );
        std::fs::write(&path, "enabled = \"maybe\"").unwrap();
        assert!(shared.reload().is_err());
        assert!(!shared.enabled());
        std::fs::remove_file(&path).unwrap();
        shared.reload().unwrap();
        assert!(shared.enabled());
        assert_eq!(
            shared.machine.lock().unwrap().settings().shift,
            Some(Mode::Convert)
        );
        assert!(rx.try_recv().is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

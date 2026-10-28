//! `nashi-cli`: the engine front-end used by the Nashi UI.
//! Contract: `docs/CLI-PROTOCOL.md`. With `--json`, stdout carries only JSON;
//! diagnostics go to stderr.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;

use clap::{Parser, Subcommand};
use nashi_core::catalog;
use nashi_core::deps::{self, Deps};
use nashi_core::detect;
use nashi_core::events::{Event, Sink};
use nashi_core::job::{self, JobKind};
use nashi_core::options::Options;
use nashi_core::process;

#[derive(Parser)]
#[command(name = "nashi-cli", version, about = "Nashi conversion engine")]
struct Cli {
    /// Machine-readable output (JSON / JSON lines on stdout).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Full catalog (formats, targets, tools, actions, availability).
    Catalog,
    /// Wheel items for exactly these files.
    Wheel {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Per-file category, extension, size, media facts, PDF pages.
    Inspect {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// External tool availability, versions and Arch package names.
    Deps,
    /// Convert each file to TARGET.
    Convert {
        target: String,
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long)]
        options: Option<String>,
    },
    /// Run a tool.
    Tool {
        tool_id: String,
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long)]
        options: Option<String>,
    },
}

fn print_json<T: serde::Serialize>(v: &T) {
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, v);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

struct JsonSink;

impl Sink for JsonSink {
    fn emit(&self, e: &Event) {
        print_json(e);
    }
}

struct HumanSink {
    last: Mutex<Option<(Option<usize>, i64)>>,
}

impl Sink for HumanSink {
    fn emit(&self, e: &Event) {
        match e {
            Event::Start { total, .. } => eprintln!("Processing {total} file(s)…"),
            Event::Progress {
                input,
                index,
                fraction,
                message,
            } => {
                let pct = fraction.map(|f| (f * 100.0).round() as i64).unwrap_or(-1);
                let mut last = self.last.lock().unwrap();
                let key = (*index, pct / 10);
                if *last != Some(key) {
                    *last = Some(key);
                    let name = input.as_deref().unwrap_or("all inputs");
                    if pct >= 0 {
                        eprintln!("  {message} {name}: {pct}%");
                    } else {
                        eprintln!("  {message} {name}…");
                    }
                }
            }
            Event::Output { path, message, .. } => {
                println!("{path}");
                if let Some(m) = message {
                    println!("{m}");
                }
            }
            Event::Error {
                input,
                code,
                message,
                ..
            } => {
                let code = serde_json::to_value(code)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();
                eprintln!(
                    "error [{code}] {}: {message}",
                    input.as_deref().unwrap_or("job")
                );
            }
            Event::Done {
                outputs,
                failed,
                cancelled,
            } => {
                if *cancelled {
                    eprintln!("Cancelled.");
                } else {
                    eprintln!("Done: {} output(s), {failed} failed.", outputs.len());
                }
            }
        }
    }
}

fn wheel_files(files: &[PathBuf]) -> Vec<(String, String, catalog::Category)> {
    files
        .iter()
        .map(|f| {
            let abs = std::path::absolute(f).unwrap_or_else(|_| f.clone());
            let d = detect::detect(&abs);
            (abs.to_string_lossy().into_owned(), d.ext, d.category)
        })
        .collect()
}

fn run_job(
    json: bool,
    kind: JobKind,
    files: Vec<PathBuf>,
    out_dir: Option<PathBuf>,
    options: Option<String>,
) -> ExitCode {
    if let Err(e) = job::validate(&kind, &files) {
        eprintln!("nashi-cli: {e}");
        return ExitCode::from(2);
    }
    let opts = match Options::parse(options.as_deref()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("nashi-cli: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = process::install_signal_handlers() {
        eprintln!("nashi-cli: cannot install signal handlers: {e}");
    }
    let deps = Deps::global();
    let summary = if json {
        job::run(&kind, &files, out_dir.as_deref(), &opts, deps, &JsonSink)
    } else {
        let sink = HumanSink {
            last: Mutex::new(None),
        };
        job::run(&kind, &files, out_dir.as_deref(), &opts, deps, &sink)
    };
    ExitCode::from(summary.exit_code() as u8)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match cli.cmd {
        Cmd::Catalog => {
            let c = catalog::catalog(Deps::global());
            if json {
                print_json(&c);
            } else {
                println!("Formats:");
                for f in &c.formats {
                    println!(
                        "  {:<13} {:<13} → {}",
                        f.ext,
                        f.category.as_str(),
                        f.targets.join(", ")
                    );
                }
                println!("Tools:");
                for t in &c.tools {
                    let avail = if t.available {
                        String::new()
                    } else {
                        format!("  (missing: {})", t.missing.join(", "))
                    };
                    println!("  {:<17} {}{avail}", t.id, t.title.en);
                }
                println!("Actions:");
                for a in c.actions {
                    println!("  {:<17} {}", a.id, a.title.en);
                }
            }
        }
        Cmd::Wheel { files } => {
            let w = catalog::wheel(&wheel_files(&files), Deps::global());
            if json {
                print_json(&w);
            } else {
                for f in &w.files {
                    println!("{} ({}, {})", f.path, f.ext, f.category.as_str());
                }
                let mark = |a: bool| if a { "" } else { " (unavailable)" };
                println!(
                    "Convert: {}",
                    w.convert
                        .iter()
                        .map(|t| format!("{}{}", t.id, mark(t.available)))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                println!(
                    "Tools:   {}",
                    w.tools
                        .iter()
                        .map(|t| format!("{}{}", t.id, mark(t.available)))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                println!(
                    "Actions: {}",
                    w.actions
                        .iter()
                        .map(|a| a.id)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        Cmd::Inspect { files } => {
            let deps = Deps::global();
            let list: Vec<detect::Inspect> = files
                .iter()
                .map(|f| {
                    detect::inspect(deps, &std::path::absolute(f).unwrap_or_else(|_| f.clone()))
                })
                .collect();
            if json {
                print_json(&serde_json::json!({ "files": list }));
            } else {
                for i in &list {
                    let cat = i.category.map(|c| c.as_str()).unwrap_or("unknown");
                    let mut extra = Vec::new();
                    if let Some(s) = i.size {
                        extra.push(format!("{s} bytes"));
                    }
                    if let Some(d) = i.duration {
                        extra.push(format!("{d:.2}s"));
                    }
                    if let (Some(w), Some(h)) = (i.width, i.height) {
                        extra.push(format!("{w}x{h}"));
                    }
                    if let Some(p) = i.pages {
                        extra.push(format!("{p} pages"));
                    }
                    println!("{}: {} ({cat}) {}", i.path, i.ext, extra.join(", "));
                }
            }
        }
        Cmd::Deps => {
            let r = deps::report(Deps::global());
            if json {
                print_json(&serde_json::json!({ "tools": r }));
            } else {
                for d in &r {
                    let state = if d.available { "ok     " } else { "missing" };
                    println!(
                        "  {state} {:<13} {:<12} pacman -S {}{}",
                        d.id,
                        d.version.as_deref().unwrap_or("-"),
                        d.package,
                        if d.optional { "  (optional)" } else { "" }
                    );
                }
            }
        }
        Cmd::Convert {
            target,
            files,
            out_dir,
            options,
        } => {
            return run_job(
                json,
                JobKind::Convert(target.to_ascii_lowercase()),
                files,
                out_dir,
                options,
            )
        }
        Cmd::Tool {
            tool_id,
            files,
            out_dir,
            options,
        } => return run_job(json, JobKind::Tool(tool_id), files, out_dir, options),
    }
    ExitCode::SUCCESS
}

//! Shared helpers for CLI integration tests: run the binary, validate the
//! event stream, generate fixtures with external tools at runtime.

#![allow(dead_code)]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const BIN: &str = env!("CARGO_BIN_EXE_nashi-cli");

pub fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
        .unwrap_or(false)
}

/// Return early (skip) when any tool is missing.
#[macro_export]
macro_rules! need {
    ($($t:expr),+) => {
        $(
            if !$crate::common::which($t) {
                eprintln!("skipped: {} missing", $t);
                return;
            }
        )+
    };
}

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub events: Vec<Value>,
}

impl Run {
    pub fn outputs(&self) -> Vec<PathBuf> {
        self.events
            .iter()
            .filter(|e| e["event"] == "output")
            .map(|e| PathBuf::from(e["path"].as_str().unwrap()))
            .collect()
    }
    pub fn errors(&self) -> Vec<&Value> {
        self.events
            .iter()
            .filter(|e| e["event"] == "error")
            .collect()
    }
    pub fn error_codes(&self) -> Vec<String> {
        self.errors()
            .iter()
            .map(|e| e["code"].as_str().unwrap().to_string())
            .collect()
    }
    pub fn done(&self) -> &Value {
        self.events.last().expect("no events")
    }
    /// The single output; panics with diagnostics otherwise.
    pub fn one(&self) -> PathBuf {
        let o = self.outputs();
        assert_eq!(
            o.len(),
            1,
            "expected one output\nstdout:\n{}\nstderr:\n{}",
            self.stdout,
            self.stderr
        );
        assert!(o[0].exists(), "output missing: {}", o[0].display());
        o[0].clone()
    }
    pub fn ok(&self) -> &Self {
        assert_eq!(
            self.code, 0,
            "exit code\nstdout:\n{}\nstderr:\n{}",
            self.stdout, self.stderr
        );
        self
    }
}

pub fn run_env(args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut c = Command::new(BIN);
    c.args(args).stdin(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().expect("run nashi-cli");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let mut events = Vec::new();
    if args.contains(&"--json") && (args[0] == "convert" || args[0] == "tool") {
        for line in stdout.lines() {
            let v: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("non-JSON line {line:?}: {e}"));
            check_event(&v);
            events.push(v);
        }
        if !events.is_empty() {
            check_stream(&events);
        }
    }
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout,
        stderr,
        events,
    }
}

pub fn run(args: &[&str]) -> Run {
    run_env(args, &[])
}

fn is_null_or<F: Fn(&Value) -> bool>(v: &Value, f: F) -> bool {
    v.is_null() || f(v)
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

/// Validate one event against the protocol.
pub fn check_event(v: &Value) {
    let ev = v["event"].as_str().expect("event field");
    match ev {
        "start" => {
            assert_eq!(keys(v), ["event", "job", "total"]);
            assert!(v["job"].is_string());
            assert!(v["total"].is_u64());
        }
        "progress" => {
            assert_eq!(keys(v), ["event", "fraction", "index", "input", "message"]);
            assert!(is_null_or(&v["fraction"], |f| f
                .as_f64()
                .is_some_and(|x| (0.0..=1.0).contains(&x))));
            assert!(is_null_or(&v["index"], Value::is_u64));
            assert!(is_null_or(&v["input"], Value::is_string));
            assert!(v["message"].is_string());
        }
        "output" => {
            let k = keys(v);
            assert!(
                k == ["event", "index", "input", "path"]
                    || k == ["event", "index", "input", "message", "path"],
                "{k:?}"
            );
            assert!(v["path"].as_str().is_some_and(|p| p.starts_with('/')));
            assert!(is_null_or(&v["index"], Value::is_u64));
        }
        "error" => {
            assert_eq!(keys(v), ["code", "event", "index", "input", "message"]);
            let codes = [
                "unsupported_route",
                "missing_dependency",
                "tool_failed",
                "invalid_input",
                "invalid_options",
                "io",
                "cancelled",
            ];
            assert!(codes.contains(&v["code"].as_str().unwrap()), "{v}");
            assert!(v["message"].is_string());
        }
        "done" => {
            assert_eq!(keys(v), ["cancelled", "event", "failed", "outputs"]);
            assert!(v["outputs"]
                .as_array()
                .unwrap()
                .iter()
                .all(Value::is_string));
            assert!(v["failed"].is_u64());
            assert!(v["cancelled"].is_boolean());
        }
        other => panic!("unknown event {other}"),
    }
}

fn check_stream(events: &[Value]) {
    assert_eq!(events[0]["event"], "start", "first event must be start");
    assert_eq!(
        events.last().unwrap()["event"],
        "done",
        "last event must be done"
    );
    assert_eq!(events.iter().filter(|e| e["event"] == "start").count(), 1);
    assert_eq!(events.iter().filter(|e| e["event"] == "done").count(), 1);
}

pub fn tool(prog: &str, args: &[&str]) {
    // C.UTF-8: ImageMagick misbehaves under tr_TR (dotless i in coder names).
    let st = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .env("LC_ALL", "C.UTF-8")
        .output()
        .expect(prog);
    assert!(
        st.status.success(),
        "{prog} {args:?} failed: {}",
        String::from_utf8_lossy(&st.stderr)
    );
}

pub fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

// ---------------------------------------------------------------- fixtures

pub fn png(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    let gen = dir.join(".gen.png");
    tool(
        "magick",
        &[
            "-size",
            "64x48",
            "xc:red",
            "-fill",
            "blue",
            "-draw",
            "rectangle 0,0 31,47",
            "-set",
            "comment",
            "secret-comment",
            s(&gen),
        ],
    );
    std::fs::rename(&gen, &p).unwrap();
    p
}

pub fn jpg(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    let gen = dir.join(".gen.jpg");
    tool(
        "magick",
        &[
            "-size",
            "64x48",
            "xc:green",
            "-fill",
            "yellow",
            "-draw",
            "rectangle 0,0 31,47",
            "-set",
            "comment",
            "secret-comment",
            s(&gen),
        ],
    );
    std::fs::rename(&gen, &p).unwrap();
    p
}

pub fn svg(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(
        &p,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><rect width="40" height="30" fill="#c33"/></svg>"##,
    )
    .unwrap();
    p
}

pub fn wav(dir: &Path, name: &str, secs: f64) -> PathBuf {
    let p = dir.join(name);
    let src = format!("sine=frequency=440:duration={secs}");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &src,
            "-metadata",
            "title=secret",
            s(&p),
        ],
    );
    p
}

pub fn mp4(dir: &Path, name: &str, secs: f64, size: &str) -> PathBuf {
    let p = dir.join(name);
    let v = format!("testsrc=size={size}:rate=25:duration={secs}");
    let a = format!("sine=frequency=330:duration={secs}");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &v,
            "-f",
            "lavfi",
            "-i",
            &a,
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "25",
            "-pix_fmt",
            "yuv444p",
            "-c:a",
            "aac",
            "-shortest",
            "-metadata",
            "title=secret",
            s(&p),
        ],
    );
    p
}

/// Hand-written PDF with `pages` pages of text and an Info Title.
pub fn pdf(dir: &Path, name: &str, pages: usize) -> PathBuf {
    let mut objs: Vec<String> = Vec::new();
    let page_ids: Vec<usize> = (0..pages).map(|i| 4 + i * 2).collect();
    objs.push("<< /Type /Catalog /Pages 2 0 R >>".into()); // 1
    objs.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {pages} >>",
        page_ids
            .iter()
            .map(|i| format!("{i} 0 R"))
            .collect::<Vec<_>>()
            .join(" ")
    )); // 2
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into()); // 3
    for i in 0..pages {
        let content = format!("BT /F1 24 Tf 72 700 Td (Page {} hello) Tj ET", i + 1);
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            5 + i * 2
        ));
        objs.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    objs.push("<< /Title (Secret Title) /Author (Secret Author) >>".into());
    let info = objs.len();
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    let p = dir.join(name);
    std::fs::write(&p, out).unwrap();
    p
}

pub fn text(dir: &Path, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p
}

pub fn hash(p: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::fs::read(p).unwrap().hash(&mut h);
    h.finish()
}

/// No Nashi temp files anywhere under `dir`.
pub fn assert_no_temp(dir: &Path) {
    for e in walk(dir) {
        let n = e.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!n.contains("nashi-tmp"), "leftover temp {}", e.display());
    }
}

pub fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        v.push(p.clone());
        if p.is_dir() {
            v.extend(walk(&p));
        }
    }
    v
}

/// First bytes of a file.
pub fn head(p: &Path) -> Vec<u8> {
    let d = std::fs::read(p).unwrap();
    d[..d.len().min(64)].to_vec()
}

pub fn ffprobe_json(p: &Path) -> Value {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(p)
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap_or(Value::Null)
}

pub fn convert(target: &str, inputs: &[&Path]) -> Run {
    let mut a = vec!["convert", target];
    a.extend(inputs.iter().map(|p| s(p)));
    a.push("--json");
    run(&a)
}

pub fn tool_run(id: &str, inputs: &[&Path], options: Option<&str>) -> Run {
    tool_run_env(id, inputs, options, &[])
}

pub fn tool_run_env(
    id: &str,
    inputs: &[&Path],
    options: Option<&str>,
    env: &[(&str, &str)],
) -> Run {
    let mut a = vec!["tool", id];
    a.extend(inputs.iter().map(|p| s(p)));
    if let Some(o) = options {
        a.push("--options");
        a.push(o);
    }
    a.push("--json");
    run_env(&a, env)
}

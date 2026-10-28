//! Protocol-level tests: JSON shapes, exit codes, naming, safety, cancellation.

mod common;
use common::*;
use serde_json::Value;
use std::path::Path;

#[test]
fn catalog_shape() {
    let r = run(&["catalog", "--json"]);
    assert_eq!(r.code, 0);
    let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    let formats = v["formats"].as_array().unwrap();
    assert_eq!(formats.len(), 45);
    for f in formats {
        for k in [
            "ext",
            "category",
            "title",
            "targets",
            "single_tools",
            "multiple_tools",
        ] {
            assert!(!f[k].is_null(), "format missing {k}: {f}");
        }
        assert_eq!(
            f["title"].as_str().unwrap(),
            f["ext"].as_str().unwrap().to_uppercase()
        );
    }
    let tools = v["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 27);
    for t in tools {
        assert!(t["title"]["en"].is_string() && t["title"]["tr"].is_string());
        assert!(
            t["needs_editor"].is_boolean()
                && t["multi_input"].is_boolean()
                && t["available"].is_boolean()
        );
        assert!(t["missing"].is_array());
    }
    let ids: Vec<&str> = v["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["copy", "copyPath", "terminal", "openFolder", "reveal"]
    );
    // Human mode works too and writes something.
    let h = run(&["catalog"]);
    assert_eq!(h.code, 0);
    assert!(h.stdout.contains("Formats:"));
}

#[test]
fn deps_shape() {
    let r = run(&["deps", "--json"]);
    assert_eq!(r.code, 0);
    let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    let tools = v["tools"].as_array().unwrap();
    let ids: Vec<&str> = tools.iter().map(|t| t["id"].as_str().unwrap()).collect();
    for id in [
        "ffmpeg",
        "ffprobe",
        "magick",
        "soffice",
        "exiftool",
        "qpdf",
        "pdftoppm",
        "pdftotext",
        "gs",
        "7z",
        "zbarimg",
        "pandoc",
        "rsvg-convert",
        "yt-dlp",
    ] {
        assert!(ids.contains(&id), "deps lacks {id}");
    }
    for t in tools {
        assert!(t["available"].is_boolean());
        assert!(t["package"].is_string());
        assert!(t["optional"].is_boolean());
        assert!(t["used_for"].is_array());
        if t["available"] == false {
            assert!(t["path"].is_null() && t["version"].is_null());
        }
    }
    let so = tools.iter().find(|t| t["id"] == "soffice").unwrap();
    assert_eq!(so["package"], "libreoffice-fresh");
    let disabled = run_env(&["deps", "--json"], &[("NASHI_DISABLE_TOOLS", "ffmpeg")]);
    let v: Value = serde_json::from_str(disabled.stdout.trim()).unwrap();
    let ff = v["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "ffmpeg")
        .unwrap()
        .clone();
    assert_eq!(ff["available"], false);
}

#[test]
fn wheel_shape() {
    let d = tempfile::tempdir().unwrap();
    let a = text(d.path(), "a.mp4", "");
    let r = run(&["wheel", s(&a), "--json"]);
    assert_eq!(r.code, 0);
    let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(v["files"][0]["category"], "video");
    assert_eq!(v["files"][0]["path"], s(&a));
    let conv: Vec<&str> = v["convert"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(conv, ["mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"]);
    for t in v["tools"].as_array().unwrap() {
        assert!(t["title"]["en"].is_string());
        assert!(t["needs_editor"].is_boolean());
        if t["available"] == false {
            assert!(!t["missing"].as_array().unwrap().is_empty());
        }
    }
    // Missing tools are reported per item.
    let r = run_env(
        &["wheel", s(&a), "--json"],
        &[("NASHI_DISABLE_TOOLS", "ffmpeg")],
    );
    let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert!(v["convert"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["available"] == false && t["missing"][0] == "ffmpeg"));
    // Mixed categories have no common target.
    let b = text(d.path(), "b.pdf", "");
    let r = run(&["wheel", s(&a), s(&b), "--json"]);
    let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert!(v["convert"].as_array().unwrap().is_empty());
}

#[test]
fn inspect_shape() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "v.mp4", 1.0, "160x120");
    let p = pdf(d.path(), "p.pdf", 3);
    let i = png(d.path(), "i.png");
    let r = run(&[
        "inspect",
        s(&v),
        s(&p),
        s(&i),
        "/nonexistent/x.flac",
        "--json",
    ]);
    assert_eq!(r.code, 0);
    let j: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    let f = j["files"].as_array().unwrap();
    for x in f {
        assert_eq!(
            {
                let mut k: Vec<&String> = x.as_object().unwrap().keys().collect();
                k.sort();
                k
            },
            ["category", "duration", "ext", "height", "pages", "path", "size", "width"]
        );
    }
    assert_eq!(f[0]["category"], "video");
    assert_eq!(f[0]["width"], 160);
    assert!((f[0]["duration"].as_f64().unwrap() - 1.0).abs() < 0.2);
    assert_eq!(f[1]["pages"], 3);
    assert_eq!(f[2]["width"], 64);
    assert_eq!(f[3]["category"], "audio");
    assert!(f[3]["size"].is_null());
}

#[test]
fn usage_errors_exit_2_without_stdout() {
    let d = tempfile::tempdir().unwrap();
    let a = text(d.path(), "a.txt", "hi");
    for args in [
        vec!["convert", "xyz", s(&a), "--json"],
        vec!["tool", "noSuchTool", s(&a), "--json"],
        vec!["convert", "pdf", s(&a), "--options", "{bad json", "--json"],
        vec![
            "convert",
            "pdf",
            s(&a),
            "--options",
            r#"{"quality": 900}"#,
            "--json",
        ],
        vec!["convert", "--json"],
        vec!["frobnicate"],
    ] {
        let r = run(&args);
        assert_eq!(r.code, 2, "{args:?}");
        assert!(
            r.stdout.is_empty(),
            "{args:?} printed to stdout: {}",
            r.stdout
        );
        assert!(!r.stderr.is_empty());
    }
}

#[test]
fn missing_input_and_unsupported_route() {
    let d = tempfile::tempdir().unwrap();
    let r = run(&["convert", "srt", "/nonexistent/a.vtt", "--json"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.error_codes(), ["invalid_input"]);
    assert_eq!(r.done()["failed"], 1);
    // Explicitly unsupported route (Pages cannot be written).
    let doc = text(d.path(), "a.md", "# x");
    let r = convert("pages", &[&doc]);
    assert_eq!(r.code, 1);
    assert_eq!(r.error_codes(), ["unsupported_route"]);
    assert!(r.errors()[0]["message"].as_str().unwrap().contains("Pages"));
    // pdf -> xlsx
    let p = pdf(d.path(), "p.pdf", 1);
    let r = convert("xlsx", &[&p]);
    assert_eq!(r.error_codes(), ["unsupported_route"]);
    // A target not valid for this source.
    let r = convert("mp3", &[&doc]);
    assert_eq!(r.error_codes(), ["unsupported_route"]);
    // Tool that does not apply.
    let r = tool_run("muteVideo", &[&doc], None);
    assert_eq!(r.error_codes(), ["unsupported_route"]);
    assert_no_temp(d.path());
}

#[test]
fn missing_dependency_exit_3() {
    let d = tempfile::tempdir().unwrap();
    let a = text(d.path(), "a.wav", "RIFF");
    let b = text(d.path(), "b.wav", "RIFF");
    let r = run_env(
        &["convert", "mp3", s(&a), s(&b), "--json"],
        &[("NASHI_DISABLE_TOOLS", "ffmpeg,ffprobe")],
    );
    assert_eq!(r.code, 3);
    assert_eq!(
        r.error_codes(),
        ["missing_dependency", "missing_dependency"]
    );
    assert!(r.errors()[0]["message"]
        .as_str()
        .unwrap()
        .contains("ffmpeg"));
    // Mixed: one missing dep + one other failure -> exit 1.
    let c = text(d.path(), "c.vtt", "WEBVTT\n\n00:01.000 --> 00:02.000\nhi\n");
    let r = run_env(
        &["convert", "srt", s(&c), "/nonexistent.vtt", "--json"],
        &[],
    );
    assert_eq!(r.code, 1);
}

#[test]
fn unique_naming_no_overwrite_input_untouched() {
    let d = tempfile::tempdir().unwrap();
    let a = text(
        d.path(),
        "talk.vtt",
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n",
    );
    let existing = text(d.path(), "talk.srt", "do not touch");
    let before = hash(&a);
    let r1 = convert("srt", &[&a]);
    r1.ok();
    assert_eq!(r1.one(), d.path().join("talk (1).srt"));
    let r2 = convert("srt", &[&a]);
    assert_eq!(r2.one(), d.path().join("talk (2).srt"));
    assert_eq!(std::fs::read_to_string(&existing).unwrap(), "do not touch");
    assert_eq!(hash(&a), before);
    assert_no_temp(d.path());
    // --out-dir is created and used.
    let out = d.path().join("nested/out");
    let r = run(&["convert", "srt", s(&a), "--out-dir", s(&out), "--json"]);
    assert_eq!(r.one(), out.join("talk.srt"));
}

#[test]
fn dash_file_names_are_safe() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = png(d.path(), "-rf.png");
    let r = convert("jpg", &[&p]);
    r.ok();
    assert_eq!(r.one(), d.path().join("-rf.jpg"));
    // Relative path starting with '-' given after the target.
    let r = run_env(&["convert", "bmp", "--json", "--", s(&p)], &[]);
    r.ok();
}

#[test]
fn event_indices_and_human_mode() {
    let d = tempfile::tempdir().unwrap();
    let a = text(d.path(), "a.srt", "1\n00:00:01,000 --> 00:00:02,000\nA\n");
    let b = text(d.path(), "b.srt", "1\n00:00:01,000 --> 00:00:02,000\nB\n");
    let r = convert("vtt", &[&a, &b]);
    r.ok();
    assert_eq!(r.events[0]["total"], 2);
    let outs: Vec<&Value> = r.events.iter().filter(|e| e["event"] == "output").collect();
    assert_eq!(outs[0]["index"], 0);
    assert_eq!(outs[1]["index"], 1);
    assert_eq!(outs[0]["input"], s(&a));
    assert_eq!(r.done()["outputs"].as_array().unwrap().len(), 2);
    assert_eq!(r.done()["cancelled"], false);
    // Human mode prints output paths on stdout.
    let h = run(&["convert", "txt", s(&a)]);
    assert_eq!(h.code, 0);
    assert!(h.stdout.trim().ends_with("a.txt"));
}

#[test]
fn cancellation_kills_children_and_cleans_up() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "long.mp4", 30.0, "1280x720");
    let mut child = std::process::Command::new(BIN)
        .args(["convert", "webm", s(&v), "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    use std::io::{BufRead, BufReader};
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut lines = Vec::new();
    // Wait for an encoding progress event (ffmpeg is running).
    loop {
        let mut l = String::new();
        assert!(
            reader.read_line(&mut l).unwrap() > 0,
            "stream ended early: {lines:?}"
        );
        let v: Value = serde_json::from_str(l.trim()).unwrap();
        check_event(&v);
        lines.push(v.clone());
        if v["event"] == "progress" && v["fraction"].as_f64().is_some_and(|f| f > 0.0) {
            break;
        }
    }
    // The temp file exists while encoding.
    assert!(walk(d.path())
        .iter()
        .any(|p| p.to_string_lossy().ends_with(".nashi-tmp")));
    unsafe {
        libc_kill(child.id() as i32, 15);
    }
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut reader, &mut rest).unwrap();
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(130));
    let last: Value = serde_json::from_str(rest.lines().last().unwrap()).unwrap();
    check_event(&last);
    assert_eq!(last["event"], "done");
    assert_eq!(last["cancelled"], true);
    assert_no_temp(d.path());
    assert!(!d.path().join("long.webm").exists());
    // No ffmpeg left behind from our process group.
    std::thread::sleep(std::time::Duration::from_millis(300));
    let ps = std::process::Command::new("pgrep")
        .args(["-f", s(&v)])
        .output();
    if let Ok(ps) = ps {
        assert!(
            String::from_utf8_lossy(&ps.stdout).trim().is_empty(),
            "ffmpeg still running"
        );
    }
}

extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

#[test]
fn json_mode_never_prints_non_json() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = png(d.path(), "x.png");
    let r = convert("webp", &[&p, Path::new("/nonexistent/y.png")]);
    assert_eq!(r.code, 1);
    assert_eq!(r.outputs().len(), 1);
    assert_eq!(r.error_codes(), ["invalid_input"]);
}

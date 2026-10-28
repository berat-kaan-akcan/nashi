//! Tools. Each group is skipped (not failed) when its external tool is missing.

mod common;
use common::*;
use std::path::Path;
use std::process::Command;

fn exif_has(p: &Path, needle: &str) -> bool {
    let out = Command::new("exiftool")
        .args(["-a", "-G1"])
        .arg(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).contains(needle)
}

fn pdfinfo(p: &Path) -> String {
    let out = Command::new("pdfinfo").arg(p).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn pages_of(p: &Path) -> u32 {
    pdfinfo(p)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:").map(|v| v.trim().parse().unwrap()))
        .unwrap()
}

#[test]
fn image_tools() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = png(d.path(), "pic.png");
    let j = jpg(d.path(), "photo.jpg");
    let before = (hash(&p), hash(&j));
    let r = tool_run("compress", &[&j], Some(r#"{"quality":40}"#));
    r.ok();
    assert_eq!(r.one(), d.path().join("photo-compressed.jpg"));
    tool_run("compress", &[&p], None).ok();
    let r = tool_run(
        "cropImage",
        &[&p],
        Some(r#"{"rect":{"x":0,"y":0,"w":10,"h":8}}"#),
    );
    r.ok();
    let id = Command::new("magick")
        .args(["identify", "-format", "%w %h"])
        .arg(r.one())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&id.stdout), "10 8");
    let r = tool_run(
        "editImage",
        &[&p],
        Some(r#"{"rotate":90,"flip":"horizontal"}"#),
    );
    r.ok();
    tool_run("frameImage", &[&p], None).ok();
    tool_run(
        "redactImage",
        &[&p],
        Some(r#"{"rects":[{"x":1,"y":1,"w":5,"h":5}]}"#),
    )
    .ok();
    // Editor-only tools without options fail cleanly.
    assert_eq!(
        tool_run("cropImage", &[&p], None).error_codes(),
        ["invalid_options"]
    );
    assert_eq!(
        tool_run("editImage", &[&p], None).error_codes(),
        ["invalid_options"]
    );
    assert_eq!(
        tool_run("annotateImage", &[&p], None).error_codes(),
        ["invalid_options"]
    );
    assert_eq!((hash(&p), hash(&j)), before);
    assert_no_temp(d.path());
}

#[test]
fn image_multi_tools() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let a = png(d.path(), "a.png");
    let b = jpg(d.path(), "b.jpg");
    let c = svg(d.path(), "c.svg");
    let r = tool_run("createPDF", &[&a, &b, &c], None);
    r.ok();
    let out = r.one();
    assert_eq!(out, d.path().join("images.pdf"));
    let o = r.events.iter().find(|e| e["event"] == "output").unwrap();
    assert!(o["index"].is_null() && o["input"].is_null());
    if which("pdfinfo") {
        assert_eq!(pages_of(&out), 3);
    }
    let r = tool_run("createCollage", &[&a, &b], None);
    r.ok();
    assert_eq!(r.one(), d.path().join("collage.png"));
    // Second run never overwrites.
    assert_eq!(
        tool_run("createCollage", &[&a, &b], None).one(),
        d.path().join("collage (1).png")
    );
    // Wrong input type.
    let t = text(d.path(), "x.txt", "x");
    assert_eq!(
        tool_run("createPDF", &[&a, &t], None).error_codes(),
        ["invalid_input"]
    );
}

#[test]
fn remove_metadata_image_exiftool() {
    need!("magick", "exiftool");
    let d = tempfile::tempdir().unwrap();
    let j = jpg(d.path(), "photo.jpg");
    assert!(exif_has(&j, "secret-comment"));
    let r = tool_run("removeMetadata", &[&j], None);
    r.ok();
    let out = r.one();
    assert_eq!(out, d.path().join("photo-clean.jpg"));
    assert!(!exif_has(&out, "secret-comment"));
    assert!(exif_has(&j, "secret-comment"), "input must be untouched");
}

#[test]
fn remove_metadata_image_magick_fallback() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = png(d.path(), "pic.png");
    let r = tool_run_env(
        "removeMetadata",
        &[&p],
        None,
        &[("NASHI_DISABLE_TOOLS", "exiftool")],
    );
    r.ok();
    let out = r.one();
    let id = Command::new("magick")
        .args(["identify", "-format", "%c"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&id.stdout).contains("secret-comment"));
}

#[test]
fn remove_metadata_pdf() {
    need!("pdfinfo");
    let d = tempfile::tempdir().unwrap();
    let p = pdf(d.path(), "doc.pdf", 2);
    assert!(pdfinfo(&p).contains("Secret Title"));
    let mut variants: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
    if which("exiftool") && which("qpdf") {
        variants.push(("exiftool+qpdf", vec![]));
    } else {
        eprintln!("skipped: exiftool/qpdf variant");
    }
    if which("gs") {
        variants.push(("gs", vec![("NASHI_DISABLE_TOOLS", "exiftool")]));
    }
    for (name, env) in variants {
        let r = tool_run_env("removeMetadata", &[&p], None, &env);
        r.ok();
        let info = pdfinfo(&r.one());
        assert!(
            !info.contains("Secret Title") && !info.contains("Secret Author"),
            "{name}: {info}"
        );
        // The old metadata must not be recoverable from the bytes either.
        let raw = std::fs::read(r.one()).unwrap();
        assert!(
            !raw.windows(12).any(|w| w == b"Secret Title"),
            "{name}: still in file"
        );
    }
}

#[test]
fn remove_metadata_media() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "clip.mp4", 1.0, "64x64");
    let w = wav(d.path(), "tone.wav", 0.5);
    for p in [&v, &w] {
        let r = tool_run("removeMetadata", &[p], None);
        r.ok();
        let j = ffprobe_json(&r.one());
        assert!(j["format"]["tags"]["title"].is_null(), "{j}");
    }
}

#[test]
fn audio_tools() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let w = wav(d.path(), "tone.wav", 2.0);
    let r = convert("mp3", &[&w]);
    let mp3 = r.one();
    for (id, opts, expect) in [
        ("compress", None, "tone-compressed.mp3"),
        ("normalizeAudio", None, "tone-normalized.mp3"),
        (
            "audioChannels",
            Some(r#"{"channels":"stereo"}"#),
            "tone-stereo.mp3",
        ),
        (
            "trimAudio",
            Some(r#"{"start":0.5,"end":1.5}"#),
            "tone-trimmed.mp3",
        ),
        (
            "redactAudio",
            Some(r#"{"ranges":[[0.2,0.4]]}"#),
            "tone-bleeped.mp3",
        ),
        ("audioToVideo", None, "tone-visualizer.mp4"),
    ] {
        let r = tool_run(id, &[&mp3], opts);
        r.ok();
        assert_eq!(r.one(), d.path().join(expect), "{id}");
    }
    let j = ffprobe_json(&d.path().join("tone-trimmed.mp3"));
    let dur: f64 = j["format"]["duration"].as_str().unwrap().parse().unwrap();
    assert!((dur - 1.0).abs() < 0.15, "trim duration {dur}");
    let j = ffprobe_json(&d.path().join("tone-stereo.mp3"));
    assert_eq!(j["streams"][0]["channels"], 2);
    // Multiple files: per-file outputs with indices.
    let r = tool_run("normalizeAudio", &[&w, &mp3], None);
    r.ok();
    assert_eq!(r.outputs().len(), 2);
    assert_eq!(
        tool_run("trimAudio", &[&w], None).error_codes(),
        ["invalid_options"]
    );
    assert_no_temp(d.path());
}

#[test]
fn video_tools() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "clip.mp4", 3.0, "160x120");
    for (id, opts, expect) in [
        ("compress", None, "clip-compressed.mp4"),
        ("muteVideo", None, "clip-muted.mp4"),
        ("trimVideo", Some(r#"{"start":1.0}"#), "clip-trimmed.mp4"),
        ("changeVideoSpeed", Some(r#"{"speed":2.0}"#), "clip-2x.mp4"),
        (
            "cropVideo",
            Some(r#"{"rect":{"x":0,"y":0,"w":80,"h":60}}"#),
            "clip-cropped.mp4",
        ),
        (
            "redactVideo",
            Some(r#"{"rects":[{"x":0,"y":0,"w":20,"h":20,"t0":0.5,"t1":1.0}]}"#),
            "clip-redacted.mp4",
        ),
        ("videoSnapshots", Some(r#"{"fps":1}"#), "clip-snapshots"),
        ("splitVideo", Some(r#"{"segment_seconds":1}"#), "clip-parts"),
    ] {
        let r = tool_run(id, &[&v], opts);
        r.ok();
        assert_eq!(r.one(), d.path().join(expect), "{id}");
    }
    let muted = ffprobe_json(&d.path().join("clip-muted.mp4"));
    assert!(muted["streams"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["codec_type"] != "audio"));
    let fast = ffprobe_json(&d.path().join("clip-2x.mp4"));
    let dur: f64 = fast["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((dur - 1.5).abs() < 0.3, "speed duration {dur}");
    let cropped = ffprobe_json(&d.path().join("clip-cropped.mp4"));
    assert_eq!(cropped["streams"][0]["width"], 80);
    assert!(
        std::fs::read_dir(d.path().join("clip-snapshots"))
            .unwrap()
            .count()
            >= 2
    );
    assert!(
        std::fs::read_dir(d.path().join("clip-parts"))
            .unwrap()
            .count()
            >= 2
    );
    assert_eq!(
        tool_run("changeVideoSpeed", &[&v], None).error_codes(),
        ["invalid_options"]
    );
    assert_no_temp(d.path());
}

#[test]
fn join_videos_copy_and_reencode() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let a = mp4(d.path(), "a.mp4", 1.0, "160x120");
    let b = mp4(d.path(), "b.mp4", 1.0, "160x120");
    let c = mp4(d.path(), "c.mp4", 1.0, "200x100");
    let r = tool_run("joinVideos", &[&a, &b], None);
    r.ok();
    assert_eq!(r.one(), d.path().join("joined.mp4"));
    let j = ffprobe_json(&r.one());
    let dur: f64 = j["format"]["duration"].as_str().unwrap().parse().unwrap();
    assert!((dur - 2.0).abs() < 0.3, "{dur}");
    let r = tool_run("joinVideos", &[&a, &c], None);
    r.ok();
    assert_eq!(r.one(), d.path().join("joined (1).mp4"));
    let j = ffprobe_json(&r.one());
    assert_eq!(j["streams"][0]["width"], 160);
    assert_eq!(
        tool_run("joinVideos", &[&a], None).error_codes(),
        ["invalid_input"]
    );
}

fn pdf_tools_with(env: &[(&str, &str)]) {
    let d = tempfile::tempdir().unwrap();
    let a = pdf(d.path(), "a.pdf", 2);
    let b = pdf(d.path(), "b.pdf", 3);
    let r = tool_run_env("mergePDF", &[&a, &b], None, env);
    r.ok();
    assert_eq!(r.one(), d.path().join("merged.pdf"));
    assert_eq!(pages_of(&r.one()), 5);
    let r = tool_run_env("splitPDF", &[&b], None, env);
    r.ok();
    let dir = r.one();
    assert_eq!(dir, d.path().join("b-pages"));
    for n in 1..=3 {
        assert_eq!(pages_of(&dir.join(format!("b-{n}.pdf"))), 1);
    }
    let r = tool_run_env("splitPDF", &[&b], Some(r#"{"pages":"2-3"}"#), env);
    r.ok();
    assert_eq!(std::fs::read_dir(r.one()).unwrap().count(), 2);
    assert_eq!(
        tool_run_env("mergePDF", &[&a], None, env).error_codes(),
        ["invalid_input"]
    );
    assert_no_temp(d.path());
}

#[test]
fn pdf_tools_qpdf() {
    need!("qpdf", "pdfinfo");
    pdf_tools_with(&[]);
    // organizePDF reorders pages.
    let d = tempfile::tempdir().unwrap();
    let b = pdf(d.path(), "b.pdf", 3);
    let r = tool_run("organizePDF", &[&b], Some(r#"{"pages":"3,1"}"#));
    r.ok();
    assert_eq!(pages_of(&r.one()), 2);
    if which("pdftotext") {
        let out = Command::new("pdftotext")
            .arg(r.one())
            .arg("-")
            .output()
            .unwrap();
        let t = String::from_utf8_lossy(&out.stdout);
        assert!(t.find("Page 3").unwrap() < t.find("Page 1").unwrap());
    }
    assert_eq!(
        tool_run("organizePDF", &[&b], None).error_codes(),
        ["invalid_options"]
    );
    assert_eq!(
        tool_run("organizePDF", &[&b], Some(r#"{"pages":"9"}"#)).error_codes(),
        ["invalid_options"]
    );
}

#[test]
fn pdf_tools_poppler_fallback() {
    need!("pdfunite", "pdfseparate", "pdfinfo");
    pdf_tools_with(&[("NASHI_DISABLE_TOOLS", "qpdf")]);
}

#[test]
fn pdf_tools_gs_fallback() {
    need!("gs", "pdfinfo");
    pdf_tools_with(&[("NASHI_DISABLE_TOOLS", "qpdf,pdfunite,pdfseparate")]);
}

#[test]
fn pdf_compress() {
    need!("gs");
    let d = tempfile::tempdir().unwrap();
    let p = pdf(d.path(), "doc.pdf", 2);
    let r = tool_run("compress", &[&p], Some(r#"{"pdf_preset":"screen"}"#));
    r.ok();
    assert_eq!(r.one(), d.path().join("doc-compressed.pdf"));
    assert!(head(&r.one()).starts_with(b"%PDF"));
}

#[test]
fn read_qr_code() {
    need!("zbarimg", "magick");
    let d = tempfile::tempdir().unwrap();
    let blank = png(d.path(), "blank.png");
    let r = tool_run("readQRCode", &[&blank], None);
    assert_eq!(r.error_codes(), ["invalid_input"]);
    if !which("qrencode") {
        eprintln!("skipped: qrencode missing (positive QR test)");
        return;
    }
    let q = d.path().join("code.png");
    tool("qrencode", &["-o", s(&q), "-s", "6", "hello nashi"]);
    let r = tool_run("readQRCode", &[&q], None);
    r.ok();
    let out = r.one();
    assert_eq!(out, d.path().join("code-qr.txt"));
    assert_eq!(std::fs::read_to_string(&out).unwrap().trim(), "hello nashi");
    let ev = r.events.iter().find(|e| e["event"] == "output").unwrap();
    assert_eq!(ev["message"], "hello nashi");
}

#[test]
fn hostile_file_names() {
    // Names with `)`, `'`, `:`, `%`, `\` and a leading `-` must be treated as
    // plain file names by every tool (no PostScript/filter/format injection).
    let d = tempfile::tempdir().unwrap();
    let name = |ext: &str| format!("-x) (r'q:%d\\z.{ext}");
    let p = pdf(d.path(), &name("pdf"), 2);
    let r = run(&["inspect", s(&p), "--json"]);
    let v: serde_json::Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(v["files"][0]["pages"], 2);
    if which("gs") {
        let r = tool_run("compress", &[&p], None);
        r.ok();
        assert_eq!(r.one(), d.path().join("-x) (r'q:%d\\z-compressed.pdf"));
    }
    if which("qpdf") || which("pdfseparate") {
        let r = tool_run("splitPDF", &[&p], None);
        r.ok();
        assert!(r.one().join("-x) (r'q:%d\\z-2.pdf").exists());
    }
    if which("magick") {
        let i = png(d.path(), &name("png"));
        let r = convert("jpg", &[&i]);
        r.ok();
        assert_eq!(r.one(), d.path().join(name("jpg")));
        tool_run("createPDF", &[&i], None).ok();
    }
    if which("ffmpeg") && which("ffprobe") {
        let a = mp4(d.path(), &name("mp4"), 1.0, "64x64");
        let b = mp4(d.path(), "plain.mp4", 1.0, "64x64");
        convert("mp3", &[&a]).ok();
        tool_run("joinVideos", &[&a, &b], None).ok();
        let r = tool_run("videoSnapshots", &[&a], None);
        r.ok();
        assert!(std::fs::read_dir(r.one()).unwrap().count() >= 1);
    }
    let sub = text(
        d.path(),
        &name("srt"),
        "1\n00:00:01,000 --> 00:00:02,000\nx\n",
    );
    convert("vtt", &[&sub]).ok();
    // Nothing unexpected was created outside the temp dir's known outputs.
    assert_no_temp(d.path());
}

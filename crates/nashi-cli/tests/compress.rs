//! Compress must reach the size or dimensions it was asked for. Skipped (not
//! failed) when an external tool is missing.

mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn size(p: &Path) -> u64 {
    std::fs::metadata(p).unwrap().len()
}

fn dims(p: &Path) -> (u32, u32) {
    let out = Command::new("magick")
        .args(["identify", "-format", "%w %h"])
        .arg(p)
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (w, h) = text.trim().split_once(' ').unwrap();
    (w.parse().unwrap(), h.parse().unwrap())
}

/// A photo-like image that does not compress away to nothing.
fn noisy(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
    let p = dir.join(name);
    tool(
        "magick",
        &[
            "-size",
            &format!("{w}x{h}"),
            "plasma:fractal",
            "-attenuate",
            "0.4",
            "+noise",
            "Gaussian",
            s(&p),
        ],
    );
    p
}

fn noisy_video(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=320x240:r=25:d=3,noise=alls=40:allf=t",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=3",
            "-c:v",
            "libx264",
            "-crf",
            "16",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-shortest",
            s(&p),
        ],
    );
    p
}

fn tone(dir: &Path, name: &str, extra: &[&str]) -> PathBuf {
    let p = dir.join(name);
    let mut args = vec!["-v", "error", "-y", "-f", "lavfi", "-i", "sine=f=440:d=10"];
    args.extend_from_slice(extra);
    args.push(s(&p));
    tool("ffmpeg", &args);
    p
}

fn message_of(r: &Run) -> Option<String> {
    r.events
        .iter()
        .find(|e| e["event"] == "output")
        .and_then(|e| e["message"].as_str().map(String::from))
}

#[test]
fn image_reaches_a_target_size_without_changing_shape() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = noisy(d.path(), "photo.jpg", 800, 600);
    let before = size(&p);
    let target = before / 4;
    let r = tool_run(
        "compress",
        &[&p],
        Some(&format!(r#"{{"target_bytes":{target}}}"#)),
    );
    r.ok();
    assert!(size(&r.one()) <= target, "{} > {target}", size(&r.one()));
    let (w, h) = dims(&r.one());
    assert_eq!(w * 3, h * 4, "shape changed: {w}x{h}");
    assert_eq!(size(&p), before, "input must be untouched");
}

#[test]
fn image_percentage_is_of_the_file_a_symlink_points_to() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = noisy(d.path(), "photo.jpg", 800, 600);
    let link = d.path().join("link.jpg");
    std::os::unix::fs::symlink(&p, &link).unwrap();
    let r = tool_run("compress", &[&link], Some(r#"{"percentage":85}"#));
    r.ok();
    let out = size(&r.one());
    assert!(out <= size(&p) * 15 / 100, "{out} of {}", size(&p));
    assert!(out > 1024, "shrunk far beyond the request: {out}");
}

#[test]
fn lossless_image_reaches_its_target_by_shrinking() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = noisy(d.path(), "shot.png", 600, 300);
    let target = size(&p) / 3;
    let r = tool_run(
        "compress",
        &[&p],
        Some(&format!(r#"{{"target_bytes":{target}}}"#)),
    );
    r.ok();
    assert!(size(&r.one()) <= target);
    let (w, h) = dims(&r.one());
    assert!(w < 600, "not shrunk: {w}");
    assert!((i64::from(w) - 2 * i64::from(h)).abs() <= 2, "{w}x{h}");
}

#[test]
fn unreachable_target_fails_and_leaves_nothing_behind() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = noisy(d.path(), "photo.jpg", 200, 200);
    let r = tool_run("compress", &[&p], Some(r#"{"target_bytes":10}"#));
    assert_eq!(r.error_codes(), ["invalid_options"]);
    assert_eq!(r.outputs().len(), 0);
    assert_eq!(walk(d.path()).len(), 1, "{:?}", walk(d.path()));
}

#[test]
fn file_already_under_the_target_is_kept_as_it_is() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = noisy(d.path(), "photo.jpg", 200, 200);
    let target = size(&p) + 1000;
    let r = tool_run(
        "compress",
        &[&p],
        Some(&format!(r#"{{"target_bytes":{target}}}"#)),
    );
    r.ok();
    assert_eq!(hash(&r.one()), hash(&p));
    assert_ne!(r.one(), p);
    assert!(message_of(&r).is_some());
}

#[test]
fn image_size_presets_scale_each_file_from_its_own_size() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let wide = noisy(d.path(), "wide.png", 400, 200);
    let tall = noisy(d.path(), "tall.png", 100, 300);
    let small = noisy(d.path(), "small.png", 50, 40);
    for (options, want) in [
        (r#"{"max_edge":200}"#, [(200, 100), (67, 200), (50, 40)]),
        (r#"{"image_scale":0.5}"#, [(200, 100), (50, 150), (25, 20)]),
    ] {
        let r = tool_run("compress", &[&wide, &tall, &small], Some(options));
        r.ok();
        let got: Vec<(u32, u32)> = r.outputs().iter().map(|p| dims(p)).collect();
        assert_eq!(got, want, "{options}");
    }
}

#[test]
fn bad_compress_options_are_refused() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = png(d.path(), "pic.png");
    for options in [
        r#"{"target_bytes":0}"#,
        r#"{"percentage":0}"#,
        r#"{"percentage":100}"#,
        r#"{"max_edge":0}"#,
        r#"{"image_scale":0}"#,
        r#"{"image_scale":1.5}"#,
    ] {
        let r = tool_run("compress", &[&p], Some(options));
        assert_eq!(r.error_codes(), ["invalid_options"], "{options}");
    }
}

#[test]
fn video_reaches_a_target_size_and_keeps_its_audio() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let p = noisy_video(d.path(), "clip.mp4");
    let target = size(&p) / 3;
    let r = tool_run(
        "compress",
        &[&p],
        Some(&format!(r#"{{"target_bytes":{target}}}"#)),
    );
    r.ok();
    assert!(size(&r.one()) <= target, "{} > {target}", size(&r.one()));
    let j = ffprobe_json(&r.one());
    let kinds: Vec<&str> = j["streams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["codec_type"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"audio") && kinds.contains(&"video"),
        "{kinds:?}"
    );
    assert_no_temp(d.path());
}

#[test]
fn video_max_height_only_shrinks() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let p = noisy_video(d.path(), "clip.mp4");
    for (limit, want) in [(120, (160, 120)), (1080, (320, 240))] {
        let r = tool_run(
            "compress",
            &[&p],
            Some(&format!(r#"{{"max_height":{limit}}}"#)),
        );
        r.ok();
        let j = ffprobe_json(&r.one());
        let v = j["streams"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["codec_type"] == "video")
            .unwrap()
            .clone();
        assert_eq!(
            (v["width"].as_u64().unwrap(), v["height"].as_u64().unwrap()),
            want
        );
    }
}

#[test]
fn audio_reaches_a_target_size() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let mp3 = tone(d.path(), "tone.mp3", &["-b:a", "320k"]);
    let target = size(&mp3) / 3;
    let r = tool_run(
        "compress",
        &[&mp3],
        Some(&format!(r#"{{"target_bytes":{target}}}"#)),
    );
    r.ok();
    assert!(size(&r.one()) <= target);
    assert_eq!(r.one().extension().unwrap(), "mp3");

    // Lossless containers have no bitrate to lower, so a target gives M4A.
    let wav = tone(d.path(), "tone.wav", &[]);
    let r = tool_run("compress", &[&wav], Some(r#"{"percentage":90}"#));
    r.ok();
    assert!(size(&r.one()) <= size(&wav) / 10);
    assert_eq!(r.one().extension().unwrap(), "m4a");
}

#[test]
fn audio_bitrate_option_sets_the_bitrate() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let mp3 = tone(d.path(), "tone.mp3", &["-b:a", "320k"]);
    let r = tool_run("compress", &[&mp3], Some(r#"{"audio_kbps":64}"#));
    r.ok();
    let kbps = size(&r.one()) * 8 / 10 / 1000;
    assert!((55..=75).contains(&kbps), "{kbps} kbps");
}

#[test]
fn pdf_reaches_a_target_size_or_says_it_cannot() {
    need!("gs", "magick");
    let d = tempfile::tempdir().unwrap();
    let photo = noisy(d.path(), "scan.png", 1600, 2000);
    let p = d.path().join("scan.pdf");
    tool("magick", &[s(&photo), "-quality", "95", s(&p)]);
    let target = size(&p) / 4;
    let r = tool_run(
        "compress",
        &[&p],
        Some(&format!(r#"{{"target_bytes":{target}}}"#)),
    );
    r.ok();
    assert!(size(&r.one()) <= target, "{} > {target}", size(&r.one()));

    let r = tool_run("compress", &[&p], Some(r#"{"target_bytes":50}"#));
    assert_eq!(r.error_codes(), ["invalid_options"]);
}

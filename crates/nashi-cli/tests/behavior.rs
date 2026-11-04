//! Tools must do what they promise, not merely produce a file of the right
//! size. Each test here fails if the matching operation silently becomes a
//! no-op. Skipped (not failed) when an external tool is missing.

mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn magick_out(args: &[&str]) -> String {
    let out = Command::new("magick")
        .args(args)
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "magick {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn pixel(p: &Path, x: u32, y: u32) -> String {
    magick_out(&[
        s(p),
        "-alpha",
        "off",
        "-crop",
        &format!("1x1+{x}+{y}"),
        "-format",
        "%[hex:u.p{0,0}]",
        "INFO:",
    ])
}

/// Normalised RMS difference (0..1) between the same region of two images.
fn region_rmse(a: &Path, b: &Path, region: &str) -> f64 {
    let out = Command::new("magick")
        .args(["(", s(a), "-crop", region, "+repage", ")"])
        .args(["(", s(b), "-crop", region, "+repage", ")"])
        .args(["-metric", "RMSE", "-compare", "-format", "%[distortion]"])
        .arg("INFO:")
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    let txt = String::from_utf8_lossy(&out.stdout);
    txt.trim()
        .parse()
        .unwrap_or_else(|_| panic!("unexpected compare output: {txt:?}"))
}

/// One still frame at `t` seconds, as a PNG next to the video.
fn frame_at(video: &Path, t: f64, name: &str) -> PathBuf {
    let out = video.with_file_name(name);
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-ss",
            &format!("{t:.3}"),
            "-i",
            s(video),
            "-frames:v",
            "1",
            s(&out),
        ],
    );
    out
}

fn is_black(hex: &str) -> bool {
    channels(hex).iter().all(|c| *c < 0x30)
}

fn is_white(hex: &str) -> bool {
    channels(hex).iter().all(|c| *c > 0xD0)
}

fn channels(hex: &str) -> Vec<u32> {
    (0..3)
        .map(|i| u32::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

/// ffmpeg's report for a filter that logs to stderr, on a slice of a file.
fn ffmpeg_report(p: &Path, start: f64, dur: f64, filter: &str) -> String {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-ss", &format!("{start:.3}")])
        .args(["-t", &format!("{dur:.3}"), "-i", s(p), "-af", filter])
        .args(["-f", "null", "-"])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn max_volume_db(p: &Path, start: f64, dur: f64) -> f64 {
    let report = ffmpeg_report(p, start, dur, "volumedetect");
    let (_, rest) = report
        .split_once("max_volume:")
        .unwrap_or_else(|| panic!("no volumedetect output:\n{report}"));
    rest.split_whitespace().next().unwrap().parse().unwrap()
}

fn integrated_lufs(p: &Path) -> f64 {
    let report = ffmpeg_report(p, 0.0, 3600.0, "ebur128");
    let summary = report.rsplit("Summary:").next().unwrap();
    let (_, rest) = summary
        .split_once("I:")
        .unwrap_or_else(|| panic!("no ebur128 summary:\n{report}"));
    rest.split_whitespace().next().unwrap().parse().unwrap()
}

/// 40x20: the left 10 columns are blue, everything else red.
fn marked_image(dir: &Path) -> PathBuf {
    let p = dir.join("marked.png");
    tool(
        "magick",
        &[
            "-size",
            "40x20",
            "xc:red",
            "-fill",
            "blue",
            "-draw",
            "rectangle 0,0 9,19",
            s(&p),
        ],
    );
    p
}

#[test]
fn edit_image_rotate_moves_pixels() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = marked_image(d.path());
    let r = tool_run("editImage", &[&p], Some(r#"{"rotate":90}"#));
    r.ok();
    let out = r.one();
    assert_eq!(magick_out(&[s(&out), "-format", "%w %h", "INFO:"]), "20 40");
    // 90° clockwise: the blue left edge becomes the top edge.
    assert_eq!(pixel(&out, 10, 2), "0000FF");
    assert_eq!(pixel(&out, 10, 37), "FF0000");
}

#[test]
fn edit_image_flip_mirrors_pixels() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = marked_image(d.path());
    let r = tool_run("editImage", &[&p], Some(r#"{"flip":"horizontal"}"#));
    r.ok();
    let out = r.one();
    assert_eq!(pixel(&out, 2, 10), "FF0000");
    assert_eq!(pixel(&out, 37, 10), "0000FF");
}

#[test]
fn compress_image_honours_quality() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let j = d.path().join("detail.jpg");
    tool(
        "magick",
        &["-size", "400x300", "-seed", "1", "plasma:fractal", s(&j)],
    );
    let size = |q: &str| {
        let r = tool_run("compress", &[&j], Some(&format!(r#"{{"quality":{q}}}"#)));
        r.ok();
        std::fs::metadata(r.one()).unwrap().len()
    };
    let (low, high) = (size("20"), size("95"));
    assert!(low * 2 < high, "quality ignored: q20={low} q95={high}");
}

#[test]
fn redact_image_pixelate_destroys_detail_only_inside_the_rect() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("detail.png");
    tool(
        "magick",
        &["-size", "120x90", "-seed", "1", "plasma:fractal", s(&src)],
    );
    let r = tool_run(
        "redactImage",
        &[&src],
        Some(r#"{"rects":[{"x":40,"y":20,"w":60,"h":48,"style":"pixelate"}]}"#),
    );
    r.ok();
    let out = r.one();
    let colors = |p: &Path| -> u32 {
        magick_out(&[
            s(p),
            "-crop",
            "60x48+40+20",
            "+repage",
            "-format",
            "%k",
            "INFO:",
        ])
        .parse()
        .unwrap()
    };
    // A 12x12 mosaic can hold at most 144 colours; the plasma region has far more.
    assert!(colors(&src) > 500, "fixture too flat: {}", colors(&src));
    assert!(colors(&out) <= 144, "not pixelated: {}", colors(&out));
    assert_eq!(pixel(&out, 5, 5), pixel(&src, 5, 5), "outside was changed");
}

fn white_video(dir: &Path) -> PathBuf {
    let p = dir.join("white.mp4");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=white:size=160x120:rate=25:duration=2",
            "-pix_fmt",
            "yuv444p",
            s(&p),
        ],
    );
    p
}

#[test]
fn redact_video_black_box_covers_only_its_rect_and_time_window() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    let v = white_video(d.path());
    let r = tool_run(
        "redactVideo",
        &[&v],
        Some(r#"{"rects":[{"x":0,"y":0,"w":40,"h":40,"t0":0.5,"t1":1.0,"style":"black"}]}"#),
    );
    r.ok();
    let out = r.one();
    let inside = frame_at(&out, 0.75, "inside.png");
    assert!(is_black(&pixel(&inside, 10, 10)), "rect not blacked out");
    assert!(
        is_white(&pixel(&inside, 100, 100)),
        "outside the rect changed"
    );
    for (t, name) in [(0.2, "before.png"), (1.5, "after.png")] {
        let f = frame_at(&out, t, name);
        assert!(is_white(&pixel(&f, 10, 10)), "blacked out at t={t}");
    }
}

#[test]
fn redact_video_blur_and_pixelate_change_only_the_rect() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    let v = d.path().join("detail.mp4");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x240:rate=25:duration=1",
            "-pix_fmt",
            "yuv444p",
            "-c:v",
            "libx264",
            "-qp",
            "0",
            s(&v),
        ],
    );
    let source = frame_at(&v, 0.5, "source.png");
    for style in ["blur", "pixelate"] {
        let opts = format!(r#"{{"rects":[{{"x":80,"y":60,"w":160,"h":120,"style":"{style}"}}]}}"#);
        let r = tool_run("redactVideo", &[&v], Some(&opts));
        r.ok();
        let frame = frame_at(&r.one(), 0.5, &format!("{style}.png"));
        let inside = region_rmse(&source, &frame, "160x120+80+60");
        let outside = region_rmse(&source, &frame, "60x60+5+5");
        assert!(
            inside > outside * 3.0,
            "{style}: inside={inside} outside={outside}"
        );
    }
}

#[test]
fn redact_audio_silences_only_the_range() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let w = wav(d.path(), "tone.wav", 3.0);
    let r = tool_run("redactAudio", &[&w], Some(r#"{"ranges":[[1.0,2.0]]}"#));
    r.ok();
    let out = r.one();
    assert!(max_volume_db(&out, 1.2, 0.6) < -60.0, "range not silenced");
    assert!(
        max_volume_db(&out, 0.2, 0.6) > -30.0,
        "audio before was lost"
    );
    assert!(
        max_volume_db(&out, 2.3, 0.6) > -30.0,
        "audio after was lost"
    );
}

#[test]
fn normalize_audio_reaches_target_loudness() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let q = d.path().join("quiet.wav");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=4,volume=0.03",
            s(&q),
        ],
    );
    assert!(integrated_lufs(&q) < -40.0, "fixture is not quiet");
    let r = tool_run("normalizeAudio", &[&q], None);
    r.ok();
    let lufs = integrated_lufs(&r.one());
    assert!((lufs + 16.0).abs() < 2.0, "loudness {lufs} LUFS");
}

/// A PDF of only images has nothing to extract: report it, never an empty file.
#[test]
fn pdf_without_text_is_invalid_input_not_an_empty_file() {
    need!("magick", "pdftotext");
    let d = tempfile::tempdir().unwrap();
    let scan = d.path().join("scan.pdf");
    tool("magick", &["-size", "64x64", "xc:red", s(&scan)]);
    for target in ["txt", "md", "epub"] {
        let r = convert(target, &[&scan]);
        assert_eq!(r.code, 1, "{target}: {}", r.stdout);
        assert_eq!(r.error_codes(), ["invalid_input"], "{target}");
        assert!(r.outputs().is_empty(), "{target}");
        assert!(
            !d.path().join(format!("scan.{target}")).exists(),
            "{target} left a file behind"
        );
    }
    assert_no_temp(d.path());
}

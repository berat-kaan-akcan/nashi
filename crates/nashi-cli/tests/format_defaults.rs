//! Per-format conversion defaults chosen in Settings reach the output file.
//! Skipped (not failed) when an external tool is missing.

mod common;
use common::*;
use std::path::Path;
use std::process::Command;

fn stream_value(p: &Path, stream: &str, entry: &str) -> String {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", stream, "-show_entries"])
        .arg(entry)
        .args(["-of", "csv=p=0"])
        .arg(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .trim_end_matches(',')
        .to_string()
}

fn converted(input: &Path, target: &str, options: &str) -> Run {
    run(&["convert", "--json", target, "--options", options, s(input)])
}

#[test]
fn audio_bitrate_is_used_for_lossy_targets_and_ignored_for_lossless() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let w = wav(d.path(), "a.wav", 2.0);
    for target in ["mp3", "m4a"] {
        let r = converted(&w, target, r#"{"audio_kbps":96}"#);
        r.ok();
        let rate: f64 = stream_value(&r.one(), "a:0", "stream=bit_rate")
            .parse()
            .unwrap();
        assert!((rate / 1000.0 - 96.0).abs() < 25.0, "{target}: {rate}");
    }
    let r = converted(&w, "flac", r#"{"audio_kbps":96}"#);
    r.ok();
    assert_eq!(stream_value(&r.one(), "a:0", "stream=codec_name"), "flac");
    let bad = converted(&w, "mp3", r#"{"audio_kbps":4}"#);
    assert_eq!(bad.error_codes(), ["invalid_options"]);
}

#[test]
fn video_height_and_quality_apply_to_a_conversion() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "v.mp4", 1.0, "640x480");
    let small = converted(&v, "mkv", r#"{"max_height":240}"#);
    small.ok();
    assert_eq!(
        stream_value(&small.one(), "v:0", "stream=width,height"),
        "320,240"
    );
    // A height above the source's does not enlarge it.
    let same = converted(&v, "mkv", r#"{"max_height":1080}"#);
    same.ok();
    assert_eq!(
        stream_value(&same.one(), "v:0", "stream=width,height"),
        "640,480"
    );
    for bad in [r#"{"max_height":1}"#, r#"{"max_height":16385}"#] {
        assert_eq!(
            converted(&v, "mkv", bad).error_codes(),
            ["invalid_options"],
            "{bad}"
        );
    }
}

#[test]
fn video_quality_changes_the_size_in_every_container() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "v.mp4", 1.0, "640x480");
    let size = |r: &Run| std::fs::metadata(r.one()).unwrap().len();
    for target in ["mkv", "mov", "webm", "avi", "wmv"] {
        let rough = converted(&v, target, r#"{"crf":45}"#);
        rough.ok();
        let fine = converted(&v, target, r#"{"crf":15}"#);
        fine.ok();
        assert!(size(&rough) < size(&fine), "{target}");
    }
}

#[test]
fn audio_bitrate_reaches_the_audio_of_a_video() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = d.path().join("loud.mp4");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=d=2:s=64x64",
            "-f",
            "lavfi",
            "-i",
            "anoisesrc=d=2:c=pink",
            "-shortest",
            s(&v),
        ],
    );
    let r = converted(&v, "mkv", r#"{"audio_kbps":64}"#);
    r.ok();
    let low = std::fs::metadata(r.one()).unwrap().len();
    let r = converted(&v, "mkv", r#"{"audio_kbps":320}"#);
    r.ok();
    assert!(low < std::fs::metadata(r.one()).unwrap().len());
}

#[test]
fn image_quality_changes_the_size() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("n.png");
    tool("magick", &["-size", "200x200", "plasma:", s(&p)]);
    let size = |r: &Run| std::fs::metadata(r.one()).unwrap().len();
    for target in ["jpg", "webp"] {
        let low = converted(&p, target, r#"{"quality":45}"#);
        low.ok();
        let high = converted(&p, target, r#"{"quality":95}"#);
        high.ok();
        assert!(size(&low) < size(&high), "{target}");
    }
}

#[test]
fn gif_rate_and_width_apply() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "v.mp4", 1.0, "640x480");
    let r = converted(&v, "gif", r#"{"gif_fps":5,"gif_max_width":100}"#);
    r.ok();
    assert_eq!(
        stream_value(&r.one(), "v:0", "stream=width,height"),
        "100,75"
    );
    assert_eq!(stream_value(&r.one(), "v:0", "stream=r_frame_rate"), "5/1");
    for bad in [
        r#"{"gif_fps":0}"#,
        r#"{"gif_fps":61}"#,
        r#"{"gif_max_width":1}"#,
    ] {
        assert_eq!(
            converted(&v, "gif", bad).error_codes(),
            ["invalid_options"],
            "{bad}"
        );
    }
}

#[test]
fn gif_compression_presets_differ_and_the_smallest_is_smaller() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    // Smooth colours that move: where palette and dithering matter.
    let v = d.path().join("v.mp4");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "gradients=s=160x90:r=12:d=1:speed=0.05,drawbox=x='t*60':y=20:w=30:h=30:color=red:t=fill",
            "-pix_fmt",
            "yuv420p",
            s(&v),
        ],
    );
    let gif = |options: &str| {
        let r = converted(&v, "gif", options);
        r.ok();
        let bytes = std::fs::read(r.one()).unwrap();
        std::fs::remove_file(r.one()).unwrap();
        bytes
    };
    let default = gif("{}");
    let balanced = gif(r#"{"gif_compression":"balanced"}"#);
    let fast = gif(r#"{"gif_compression":"fast"}"#);
    let smallest = gif(r#"{"gif_compression":"maximum"}"#);
    assert_eq!(default, balanced, "balanced is the engine's default");
    assert_ne!(fast, balanced);
    assert!(
        smallest.len() < balanced.len(),
        "maximum {} >= balanced {}",
        smallest.len(),
        balanced.len()
    );
    for out in [&fast, &smallest] {
        assert!(out.starts_with(b"GIF89a"));
    }
    assert_eq!(
        converted(&v, "gif", r#"{"gif_compression":"huge"}"#).error_codes(),
        ["invalid_options"]
    );
}

#[test]
fn metadata_can_be_left_out_of_a_conversion() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    let w = d.path().join("tagged.wav");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=d=1",
            "-metadata",
            "title=Secret",
            s(&w),
        ],
    );
    let tag = |p: &Path| stream_value(p, "a:0", "format_tags=title");
    let kept = converted(&w, "flac", "{}");
    kept.ok();
    assert_eq!(tag(&kept.one()), "Secret");
    let dropped = converted(&w, "m4a", r#"{"strip_metadata":true}"#);
    dropped.ok();
    assert_eq!(tag(&dropped.one()), "");

    let p = d.path().join("c.png");
    tool(
        "magick",
        &["-size", "8x8", "xc:red", "-set", "comment", "Secret", s(&p)],
    );
    let comment = |f: &Path| {
        let out = Command::new("magick")
            .args(["identify", "-format", "%c", s(f)])
            .env("LC_ALL", "C.UTF-8")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    assert_eq!(comment(&p), "Secret");
    let before = hash(&p);
    let clean = converted(&p, "jpg", r#"{"strip_metadata":true}"#);
    clean.ok();
    assert_eq!(comment(&clean.one()), "");
    assert_eq!(hash(&p), before);
}

#[test]
fn stripping_also_removes_album_art_and_ogg_tags() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    let art = png(d.path(), "art.png");
    let w = wav(d.path(), "a.wav", 1.0);
    let mp3 = d.path().join("cover.mp3");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-i",
            s(&w),
            "-i",
            s(&art),
            "-map",
            "0:a",
            "-map",
            "1:v",
            "-c:v",
            "copy",
            "-disposition:v:0",
            "attached_pic",
            s(&mp3),
        ],
    );
    let kept = converted(&mp3, "m4a", "{}");
    kept.ok();
    assert_ne!(stream_value(&kept.one(), "v", "stream=codec_name"), "");
    let dropped = converted(&mp3, "m4a", r#"{"strip_metadata":true}"#);
    dropped.ok();
    assert_eq!(stream_value(&dropped.one(), "v", "stream=codec_name"), "");

    let ogg = d.path().join("tagged.ogg");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-i",
            s(&w),
            "-metadata",
            "title=Secret",
            "-c:a",
            "libvorbis",
            s(&ogg),
        ],
    );
    let clean = converted(&ogg, "mp3", r#"{"strip_metadata":true}"#);
    clean.ok();
    assert_eq!(stream_value(&clean.one(), "a:0", "format_tags=title"), "");
}

#[test]
fn stripping_a_video_removes_its_tags_and_chapters() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let plain = mp4(d.path(), "p.mp4", 1.0, "64x64");
    let notes = d.path().join("chapters.txt");
    std::fs::write(
        &notes,
        ";FFMETADATA1\ntitle=Secret\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=500\ntitle=Part\n",
    )
    .unwrap();
    let v = d.path().join("tagged.mkv");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-i",
            s(&plain),
            "-f",
            "ffmetadata",
            "-i",
            s(&notes),
            "-map_metadata",
            "1",
            "-map_chapters",
            "1",
            "-c",
            "copy",
            s(&v),
        ],
    );
    let chapters = |p: &Path| {
        let o = Command::new("ffprobe")
            .args(["-v", "error", "-show_chapters", "-of", "csv=p=0"])
            .arg(p)
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let kept = converted(&v, "mp4", "{}");
    kept.ok();
    assert_ne!(chapters(&kept.one()), "");
    let clean = converted(&v, "mov", r#"{"strip_metadata":true}"#);
    clean.ok();
    assert_eq!(chapters(&clean.one()), "");
    assert_eq!(stream_value(&clean.one(), "v:0", "format_tags=title"), "");
}

fn sample_format(p: &Path) -> String {
    stream_value(p, "a:0", "stream=codec_name")
}

#[test]
fn pcm_targets_keep_the_depth_of_the_source_unless_told_otherwise() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let deep = d.path().join("deep.flac");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=d=1",
            "-c:a",
            "flac",
            "-sample_fmt",
            "s32",
            "-bits_per_raw_sample",
            "24",
            s(&deep),
        ],
    );
    let w = wav(d.path(), "plain.wav", 1.0);
    let flac16 = converted(&w, "flac", "{}");
    flac16.ok();

    let kept = converted(&deep, "wav", "{}");
    kept.ok();
    assert_eq!(sample_format(&kept.one()), "pcm_s24le");
    let kept = converted(&deep, "aiff", "{}");
    kept.ok();
    assert_eq!(sample_format(&kept.one()), "pcm_s24be");
    // 16-bit sources stay 16-bit.
    let same = converted(&flac16.one(), "aiff", "{}");
    same.ok();
    assert_eq!(sample_format(&same.one()), "pcm_s16be");

    for (depth, codec) in [
        ("16", "pcm_s16le"),
        ("24", "pcm_s24le"),
        ("32", "pcm_s32le"),
        ("float32", "pcm_f32le"),
    ] {
        let r = converted(&deep, "wav", &format!(r#"{{"sample_depth":"{depth}"}}"#));
        r.ok();
        assert_eq!(sample_format(&r.one()), codec, "{depth}");
    }
    let bad = converted(&deep, "wav", r#"{"sample_depth":"8"}"#);
    assert_eq!(bad.error_codes(), ["invalid_options"]);
}

#[test]
fn flac_compression_changes_the_size_only() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let noisy = d.path().join("noise.wav");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "anoisesrc=d=3:c=pink:a=0.3",
            s(&noisy),
        ],
    );
    let size = |r: &Run| std::fs::metadata(r.one()).unwrap().len();
    let fast = converted(&noisy, "flac", r#"{"audio_compression":"fast"}"#);
    fast.ok();
    let max = converted(&noisy, "flac", r#"{"audio_compression":"maximum"}"#);
    max.ok();
    assert!(size(&max) < size(&fast), "{} {}", size(&max), size(&fast));
    let bad = converted(&noisy, "flac", r#"{"audio_compression":"huge"}"#);
    assert_eq!(bad.error_codes(), ["invalid_options"]);
}

#[test]
fn tools_keep_the_depth_of_a_pcm_file() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let deep = d.path().join("deep.wav");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=d=3",
            "-c:a",
            "pcm_s24le",
            s(&deep),
        ],
    );
    let trimmed = tool_run("trimAudio", &[&deep], Some(r#"{"start":0.5,"end":2}"#));
    trimmed.ok();
    assert_eq!(sample_format(&trimmed.one()), "pcm_s24le");
    let normal = tool_run("normalizeAudio", &[&deep], None);
    normal.ok();
    assert_eq!(sample_format(&normal.one()), "pcm_s24le");
}

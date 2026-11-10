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

/// A lossless video that is black except for one white column at `x`.
fn white_column_video(dir: &Path, x: u32) -> PathBuf {
    let v = dir.join("column.mkv");
    let graph = format!(
        "color=black:s=320x240:r=25:d=1,drawbox=x={x}:y=0:w=1:h=240:color=white:t=fill,format=yuv444p"
    );
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &graph,
            "-c:v",
            "libx264",
            "-qp",
            "0",
            s(&v),
        ],
    );
    v
}

#[test]
fn redact_video_blur_and_pixelate_cover_the_last_column_at_an_odd_x() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    // The selection spans columns 101..=119, and 119 holds the only bright pixels.
    let v = white_column_video(d.path(), 119);
    for style in ["blur", "pixelate"] {
        let opts = format!(r#"{{"rects":[{{"x":101,"y":41,"w":19,"h":99,"style":"{style}"}}]}}"#);
        let r = tool_run("redactVideo", &[&v], Some(&opts));
        r.ok();
        let frame = frame_at(&r.one(), 0.5, &format!("{style}.png"));
        let inside = channels(&pixel(&frame, 119, 90));
        assert!(
            inside[0] < 0x80,
            "{style}: selected column visible: {inside:?}"
        );
        assert!(
            is_white(&pixel(&frame, 119, 10)),
            "{style}: the column above the selection must stay"
        );
    }
}

#[test]
fn redact_video_blur_replaces_pixels_that_are_transparent() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    // Fully transparent pixels still store colour, which a yuv420p export reveals.
    let v = d.path().join("alpha.mkv");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=black:s=320x240:r=25:d=1,drawbox=x=150:y=0:w=1:h=240:color=white:t=fill,format=rgba,colorchannelmixer=aa=0",
            "-c:v",
            "ffv1",
            "-pix_fmt",
            "bgra",
            s(&v),
        ],
    );
    let r = tool_run(
        "redactVideo",
        &[&v],
        Some(r#"{"rects":[{"x":100,"y":40,"w":100,"h":100,"style":"blur"}]}"#),
    );
    r.ok();
    let frame = frame_at(&r.one(), 0.5, "frame.png");
    let inside = channels(&pixel(&frame, 150, 90));
    assert!(inside[0] < 0x80, "column survived the blur: {inside:?}");
}

/// A PDF with a commented note, an inline highlight and a form field.
fn annotated_pdf(dir: &Path) -> PathBuf {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] >> >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << >> /Annots [4 0 R 5 0 R << /Type /Annot /Subtype /Highlight /Rect [0 30 20 40] /QuadPoints [0 40 20 40 0 30 20 30] /T (Inline Reviewer) /M (D:20200101000000) >>] >>",
        "<< /Type /Annot /Subtype /Text /Rect [0 0 20 20] /T (Note Reviewer) /M (D:20200101000000) /CreationDate (D:20200101000000) /Contents (keep this comment) >>",
        "<< /Type /Annot /Subtype /Widget /FT /Tx /Rect [50 0 90 20] /T (field_name) >>",
    ];
    let mut out = b"%PDF-1.5\n".to_vec();
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
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    let p = dir.join("notes.pdf");
    std::fs::write(&p, out).unwrap();
    p
}

/// The PDF rewritten with uncompressed objects, so its strings can be searched.
fn pdf_plain_text(p: &Path) -> String {
    let out = Command::new("qpdf")
        .args(["--qdf", "--object-streams=disable"])
        .arg(p)
        .arg("-")
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn remove_metadata_pdf_clears_annotation_authors_and_dates() {
    need!("qpdf", "exiftool");
    let d = tempfile::tempdir().unwrap();
    let p = annotated_pdf(d.path());
    let before = hash(&p);
    let r = tool_run("removeMetadata", &[&p], None);
    r.ok();
    let text = pdf_plain_text(&r.one());
    for gone in ["Note Reviewer", "Inline Reviewer", "D:2020"] {
        assert!(!text.contains(gone), "{gone} survived:\n{text}");
    }
    assert!(
        text.contains("keep this comment"),
        "comment text lost:\n{text}"
    );
    assert!(text.contains("field_name"), "form field name lost:\n{text}");
    assert_eq!(hash(&p), before, "input must be untouched");
}

#[test]
fn remove_metadata_pdf_warns_when_authors_cannot_be_removed() {
    need!("exiftool");
    let d = tempfile::tempdir().unwrap();
    let p = annotated_pdf(d.path());
    let r = tool_run_env(
        "removeMetadata",
        &[&p],
        None,
        &[("NASHI_DISABLE_TOOLS", "qpdf")],
    );
    r.ok();
    let warned = r.events.iter().any(|e| {
        e["event"] == "output" && e["message"].as_str().is_some_and(|m| m.contains("qpdf"))
    });
    assert!(warned, "no warning: {:?}", r.events);
}

#[test]
fn outputs_of_files_at_the_name_limit_are_shortened_not_refused() {
    need!("magick", "ffmpeg", "ffprobe", "pdftoppm", "qpdf");
    let d = tempfile::tempdir().unwrap();
    // 250 bytes of ASCII and 248 bytes of two-byte letters, plus ".png" etc.
    for stem in ["n".repeat(250), "ş".repeat(124)] {
        let image = d.path().join(format!("{stem}.png"));
        std::fs::copy(png(d.path(), "pic.png"), &image).unwrap();
        let video = d.path().join(format!("{stem}.mp4"));
        std::fs::copy(mp4(d.path(), "clip.mp4", 2.0, "64x64"), &video).unwrap();
        let doc = d.path().join(format!("{stem}.pdf"));
        std::fs::copy(pdf(d.path(), "doc.pdf", 2), &doc).unwrap();

        let compressed = tool_run("compress", &[&image], None);
        compressed.ok();
        let name = compressed.one();
        let name = name.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.ends_with("-compressed.png"), "{name}");
        assert!(name.starts_with(&stem[..100]), "{name}");

        convert("jpg", &[&image]).ok();
        tool_run("splitPDF", &[&doc], None).ok();
        convert("png", &[&doc]).ok();
        tool_run("splitVideo", &[&video], Some(r#"{"segment_seconds":1}"#)).ok();
        tool_run("videoSnapshots", &[&video], Some(r#"{"fps":1}"#)).ok();
        // A second run must take a new name rather than clash with the first.
        let again = tool_run("compress", &[&image], None);
        again.ok();
        assert_ne!(again.one(), compressed.one());
        assert_no_temp(d.path());
    }
}

/// `(channels, sample rate)` of the first audio stream.
fn audio_shape(p: &Path) -> (u64, String) {
    let j = ffprobe_json(p);
    let stream = j["streams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["codec_type"] == "audio")
        .unwrap()
        .clone();
    (
        stream["channels"].as_u64().unwrap(),
        stream["sample_rate"].as_str().unwrap().to_string(),
    )
}

#[test]
fn surround_and_hi_res_audio_converts_to_opus_and_wma() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let make = |name: &str, layout: &str, rate: &str| {
        let p = d.path().join(name);
        let src = format!("anullsrc=r={rate}:cl={layout}");
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
                "-t",
                "1",
                "-c:a",
                "flac",
                s(&p),
            ],
        );
        p
    };
    let side = make("side.flac", "5.1(side)", "48000");
    let hi = make("hi.flac", "stereo", "96000");

    // Opus keeps all six channels; WMA holds at most two at up to 48 kHz.
    let opus = convert("opus", &[&side]);
    opus.ok();
    assert_eq!(audio_shape(&opus.one()).0, 6);
    let wma = convert("wma", &[&side]);
    wma.ok();
    assert_eq!(audio_shape(&wma.one()).0, 2);
    let hi_wma = convert("wma", &[&hi]);
    hi_wma.ok();
    assert_eq!(audio_shape(&hi_wma.one()), (2, "48000".to_string()));

    let clip = d.path().join("surround.mp4");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=d=1:s=64x64",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=48000:cl=5.1(side)",
            "-t",
            "1",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            s(&clip),
        ],
    );
    let wmv = convert("wmv", &[&clip]);
    wmv.ok();
    assert_eq!(audio_shape(&wmv.one()).0, 2);
}

#[test]
fn subtitle_cues_are_written_in_start_time_order() {
    let d = tempfile::tempdir().unwrap();
    let p = text(
        d.path(),
        "late.srt",
        "1\n00:00:05,000 --> 00:00:06,000\nSecond\n\n2\n00:00:01,000 --> 00:00:02,000\nFirst\n",
    );
    let r = convert("vtt", &[&p]);
    r.ok();
    assert_eq!(
        std::fs::read_to_string(r.one()).unwrap(),
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nFirst\n\n00:00:05.000 --> 00:00:06.000\nSecond\n\n"
    );
}

#[test]
fn subtitles_without_blank_lines_keep_cues_separate() {
    let d = tempfile::tempdir().unwrap();
    let srt = text(
        d.path(),
        "packed.srt",
        "1\n00:00:01,000 --> 00:00:02,000\nA\n2\n00:00:03,000 --> 00:00:04,000\nB\n",
    );
    let r = convert("vtt", &[&srt]);
    r.ok();
    assert_eq!(
        std::fs::read_to_string(r.one()).unwrap(),
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nA\n\n00:00:03.000 --> 00:00:04.000\nB\n\n"
    );
    // WebVTT has no cue numbers, so a line of digits there is the cue's own text.
    let vtt = text(
        d.path(),
        "packed.vtt",
        "WEBVTT\n\n00:01.000 --> 00:02.000\nYear\n2024\n00:03.000 --> 00:04.000\nNext\n",
    );
    let r = convert("srt", &[&vtt]);
    r.ok();
    assert_eq!(
        std::fs::read_to_string(r.one()).unwrap(),
        "1\n00:00:01,000 --> 00:00:02,000\nYear\n2024\n\n2\n00:00:03,000 --> 00:00:04,000\nNext\n\n"
    );
}

#[test]
fn subtitle_position_tags_are_dropped() {
    let d = tempfile::tempdir().unwrap();
    // VLC, mpv and Aegisub write {\an8} to move a caption up; WebVTT players show it as text.
    let srt = text(
        d.path(),
        "top.srt",
        "1\n00:00:01,000 --> 00:00:02,000\n{\\an8}Top line\n\n2\n00:00:03,000 --> 00:00:04,000\n{\\an8}\nAlone\n\n3\n00:00:05,000 --> 00:00:06,000\nLast\n{\\an2} {\\an8}\n\n",
    );
    let r = convert("vtt", &[&srt]);
    r.ok();
    assert_eq!(
        std::fs::read_to_string(r.one()).unwrap(),
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nTop line\n\n00:00:03.000 --> 00:00:04.000\nAlone\n\n00:00:05.000 --> 00:00:06.000\nLast\n\n"
    );
    let r = convert("txt", &[&srt]);
    r.ok();
    assert_eq!(
        std::fs::read_to_string(r.one()).unwrap(),
        "Top line\nAlone\nLast\n"
    );
}

fn format_tag(p: &Path, tag: &str) -> Option<String> {
    ffprobe_json(p)["format"]["tags"][tag]
        .as_str()
        .map(String::from)
}

/// A three-second MP3 with a 64×64 JPEG cover.
fn song_with_cover(dir: &Path) -> PathBuf {
    // Made in two steps: with a one-frame picture input, -frames:v would end the audio too.
    let plain = dir.join("plain.mp3");
    let cover = dir.join("cover.jpg");
    let song = dir.join("song.mp3");
    let ff = |args: &[&str]| tool("ffmpeg", &[&["-v", "error", "-y"], args].concat());
    ff(&[
        "-f",
        "lavfi",
        "-i",
        "sine=d=3",
        "-c:a",
        "libmp3lame",
        s(&plain),
    ]);
    ff(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=64x64",
        "-frames:v",
        "1",
        s(&cover),
    ]);
    ff(&[
        "-i",
        s(&plain),
        "-i",
        s(&cover),
        "-map",
        "0:a",
        "-map",
        "1:v",
        "-c",
        "copy",
        "-disposition:v",
        "attached_pic",
        "-id3v2_version",
        "3",
        s(&song),
    ]);
    std::fs::remove_file(plain).unwrap();
    std::fs::remove_file(cover).unwrap();
    song
}

fn has_album_art(p: &Path) -> bool {
    ffprobe_json(p)["streams"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["disposition"]["attached_pic"] == 1)
}

#[test]
fn audio_conversion_keeps_album_art() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let song = song_with_cover(d.path());
    for target in ["m4a", "flac"] {
        let r = convert(target, &[&song]);
        r.ok();
        assert!(has_album_art(&r.one()), "{target}: no album art");
    }
    // A format with nowhere to store art still converts.
    convert("wav", &[&song]).ok();
}

#[test]
fn audio_tools_keep_album_art() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let song = song_with_cover(d.path());
    for (id, options) in [
        ("normalizeAudio", None),
        ("audioChannels", Some(r#"{"channels":"mono"}"#)),
        ("trimAudio", Some(r#"{"start":0.5,"end":2}"#)),
        ("trimAudio", Some(r#"{"start":0.5,"trim_silence":true}"#)),
        ("redactAudio", Some(r#"{"ranges":[[0.5,1]]}"#)),
        (
            "redactAudio",
            Some(r#"{"ranges":[[0.5,1]],"audio_mode":"bleep"}"#),
        ),
    ] {
        let r = tool_run(id, &[&song], options);
        r.ok();
        let out = r.one();
        assert!(has_album_art(&out), "{id} {options:?}: no album art");
        std::fs::remove_file(out).unwrap();
    }
    // Removing metadata still removes the cover.
    let r = tool_run(
        "normalizeAudio",
        &[&song],
        Some(r#"{"strip_metadata":true}"#),
    );
    r.ok();
    assert!(!has_album_art(&r.one()), "strip_metadata kept the cover");
}

#[test]
fn converting_ogg_keeps_its_tags() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let ogg = d.path().join("tune.ogg");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=d=1",
            "-c:a",
            "libvorbis",
            "-metadata",
            "title=Ogg Title",
            s(&ogg),
        ],
    );
    let r = convert("mp3", &[&ogg]);
    r.ok();
    assert_eq!(format_tag(&r.one(), "title").as_deref(), Some("Ogg Title"));
}

#[test]
fn converting_mov_to_mp4_keeps_apple_location() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let mov = d.path().join("phone.mov");
    let key = "com.apple.quicktime.location.ISO6709";
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=d=1:s=64x64",
            "-c:v",
            "libx264",
            "-metadata",
            &format!("{key}=+41.0+029.0/"),
            "-movflags",
            "use_metadata_tags",
            s(&mov),
        ],
    );
    let r = convert("mp4", &[&mov]);
    r.ok();
    assert_eq!(format_tag(&r.one(), key).as_deref(), Some("+41.0+029.0/"));
}

/// 720×480 stored with 32:27 pixels, which displays as 853×480 (16:9).
fn anamorphic_clip(dir: &Path) -> PathBuf {
    let p = dir.join("dv.mp4");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=d=1:s=720x480",
            "-vf",
            "setsar=32/27",
            "-c:v",
            "libx264",
            s(&p),
        ],
    );
    p
}

fn video_shape(p: &Path) -> (u64, u64, String) {
    let j = ffprobe_json(p);
    let v = j["streams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["codec_type"] == "video")
        .unwrap()
        .clone();
    (
        v["width"].as_u64().unwrap(),
        v["height"].as_u64().unwrap(),
        v["sample_aspect_ratio"]
            .as_str()
            .unwrap_or("1:1")
            .to_string(),
    )
}

#[test]
fn gif_from_non_square_pixels_has_its_displayed_shape() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let r = convert("gif", &[&anamorphic_clip(d.path())]);
    r.ok();
    let (w, h, _) = video_shape(&r.one());
    assert_eq!(w, 640);
    assert!((359..=361).contains(&h), "{w}x{h}");
}

#[test]
fn join_sizes_its_canvas_from_the_first_clips_displayed_width() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let first = anamorphic_clip(d.path());
    let second = mp4(d.path(), "square.mp4", 1.0, "320x240");
    let r = tool_run("joinVideos", &[&first, &second], None);
    r.ok();
    let (w, h, sar) = video_shape(&r.one());
    assert_eq!((w, h), (852, 480));
    assert_eq!(sar, "1:1");
}

#[test]
fn snapshots_of_non_square_pixels_have_their_displayed_shape() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    let r = tool_run(
        "videoSnapshots",
        &[&anamorphic_clip(d.path())],
        Some(r#"{"fps":1}"#),
    );
    r.ok();
    let first = walk(&r.one())
        .into_iter()
        .find(|p| p.is_file())
        .expect("a frame");
    let size = magick_out(&["identify", "-format", "%w %h", s(&first)]);
    assert_eq!(size, "853 480");
}

#[test]
fn inspect_measures_an_mp3_without_a_length_header() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("quiet-end.mp3");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:d=10:r=44100",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=mono",
            "-filter_complex",
            "[1:a]atrim=0:50[b];[0:a][b]concat=n=2:v=0:a=1",
            "-c:a",
            "libmp3lame",
            "-q:a",
            "2",
            "-write_xing",
            "0",
            "-id3v2_version",
            "0",
            s(&p),
        ],
    );
    let r = run(&["inspect", "--json", s(&p)]);
    let j: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let duration = j["files"][0]["duration"].as_f64().unwrap();
    assert!((duration - 60.0).abs() < 0.2, "duration {duration}");
}

#[test]
fn pdf_compression_keeps_the_original_when_it_cannot_shrink() {
    need!("gs");
    let d = tempfile::tempdir().unwrap();
    let p = pdf(d.path(), "small.pdf", 1);
    let r = tool_run("compress", &[&p], None);
    r.ok();
    assert_eq!(hash(&r.one()), hash(&p));
    assert_ne!(r.one(), p, "a new file, not the input");
    let told = r
        .events
        .iter()
        .any(|e| e["event"] == "output" && e["message"].is_string());
    assert!(told, "no message: {:?}", r.events);
}

#[test]
fn converting_a_rotated_photo_bakes_in_its_orientation() {
    need!("magick", "exiftool");
    let d = tempfile::tempdir().unwrap();
    let j = jpg(d.path(), "phone.jpg");
    let (w, h) = {
        let size = magick_out(&["identify", "-format", "%w %h", s(&j)]);
        let (a, b) = size.split_once(' ').unwrap();
        (a.to_string(), b.to_string())
    };
    tool(
        "exiftool",
        &["-q", "-overwrite_original", "-Orientation#=6", s(&j)],
    );
    for target in ["png", "webp"] {
        let r = convert(target, &[&j]);
        r.ok();
        let size = magick_out(&["identify", "-format", "%w %h %[orientation]", s(&r.one())]);
        assert!(
            size == format!("{h} {w} TopLeft") || size == format!("{h} {w} Undefined"),
            "{target}: {size}"
        );
    }
}

#[test]
fn a_failed_stdout_write_is_reported_in_the_exit_code() {
    let d = tempfile::tempdir().unwrap();
    let p = text(d.path(), "a.srt", "1\n00:00:01,000 --> 00:00:02,000\nA\n");
    let full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let human = Command::new(env!("CARGO_BIN_EXE_nashi-cli"))
        .args(["convert", "vtt", s(&p)])
        .stdout(full.try_clone().unwrap())
        .output()
        .unwrap();
    assert_eq!(human.status.code(), Some(1), "{human:?}");
    let json = Command::new(env!("CARGO_BIN_EXE_nashi-cli"))
        .args(["deps", "--json"])
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(json.status.code(), Some(1), "{json:?}");
}

fn seconds(p: &Path) -> f64 {
    ffprobe_json(p)["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn trim_audio_removes_silence_inside_the_selection_only() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    // 2 s silence, 2 s tone, 2 s silence, 2 s tone.
    let p = d.path().join("gaps.wav");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:r=44100",
            "-af",
            "volume='if(mod(floor(t/2),2),1,0)':eval=frame",
            "-t",
            "8",
            s(&p),
        ],
    );
    // Selecting 0..6 holds silence, tone, silence: only the 2 s of tone stay.
    let r = tool_run(
        "trimAudio",
        &[&p],
        Some(r#"{"start":0,"end":6,"trim_silence":true}"#),
    );
    r.ok();
    let kept = seconds(&r.one());
    assert!((kept - 2.0).abs() < 0.3, "kept {kept} s");
    // Without the option the selection is kept whole.
    let r = tool_run("trimAudio", &[&p], Some(r#"{"start":0,"end":6}"#));
    r.ok();
    let whole = seconds(&r.one());
    assert!((whole - 6.0).abs() < 0.1, "kept {whole} s");
}

#[test]
fn redact_audio_can_bleep_instead_of_muting() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("silent.wav");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=stereo",
            "-t",
            "3",
            s(&p),
        ],
    );
    let r = tool_run(
        "redactAudio",
        &[&p],
        Some(r#"{"ranges":[[1.0,2.0]],"audio_mode":"bleep"}"#),
    );
    r.ok();
    let inside = max_volume_db(&r.one(), 1.2, 0.5);
    assert!((-20.0..-10.0).contains(&inside), "bleep level {inside} dB");
    let outside = max_volume_db(&r.one(), 2.2, 0.5);
    assert!(outside < -80.0, "outside level {outside} dB");

    let bad = tool_run(
        "redactAudio",
        &[&p],
        Some(r#"{"ranges":[[1.0,2.0]],"audio_mode":"whistle"}"#),
    );
    assert_eq!(bad.error_codes(), ["invalid_options"]);
}

#[test]
fn pdf_to_image_uses_the_requested_resolution() {
    need!("pdftoppm", "magick", "qpdf");
    let d = tempfile::tempdir().unwrap();
    // A single 612 pt (8.5 in) wide page.
    let p = pdf(d.path(), "page.pdf", 1);
    for (options, want) in [
        (None, "1275"),
        (Some(r#"{"dpi":100}"#), "850"),
        (Some(r#"{"dpi":300}"#), "2550"),
    ] {
        let mut args = vec!["convert", "--json", "png", s(&p)];
        if let Some(o) = options {
            args.extend(["--options", o]);
        }
        let r = run(&args);
        r.ok();
        let width = magick_out(&["identify", "-format", "%w", s(&r.one())]);
        assert_eq!(width, want, "{options:?}");
    }
    let r = run(&[
        "convert",
        "--json",
        "png",
        s(&p),
        "--options",
        r#"{"dpi":5000}"#,
    ]);
    assert_eq!(r.error_codes(), ["invalid_options"]);
}

//! Fixed-size PDF parts, equal page widths in a merge, and a chosen loudness.
//! Skipped (not failed) when an external tool is missing.

mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;

/// (width, height) of each page, in points.
fn page_sizes(p: &Path) -> Vec<(f64, f64)> {
    let o = Command::new("pdfinfo")
        .args(["-f", "1", "-l", "999"])
        .arg(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| l.starts_with("Page ") && l.contains(" size:"))
        .map(|l| {
            let mut v = l.split(':').nth(1).unwrap().split_whitespace();
            let w = v.next().unwrap().parse().unwrap();
            v.next();
            (w, v.next().unwrap().parse().unwrap())
        })
        .collect()
}

/// Page sizes as shown, with each page's rotation applied.
fn page_shapes(p: &Path) -> Vec<(f64, f64)> {
    let o = Command::new("pdfinfo")
        .args(["-f", "1", "-l", "999"])
        .arg(p)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    let turned: Vec<bool> = text
        .lines()
        .filter(|l| l.starts_with("Page ") && l.contains(" rot:"))
        .map(|l| {
            let deg: i32 = l.split(':').nth(1).unwrap().trim().parse().unwrap();
            deg.rem_euclid(180) == 90
        })
        .collect();
    page_sizes(p)
        .into_iter()
        .zip(turned)
        .map(|((w, h), t)| if t { (h, w) } else { (w, h) })
        .collect()
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn sized_pdf(dir: &Path, name: &str, size: &str) -> PathBuf {
    let p = dir.join(name);
    tool(
        "magick",
        &[
            "-size",
            size,
            "xc:white",
            "-units",
            "PixelsPerInch",
            "-density",
            "72",
            s(&p),
        ],
    );
    p
}

#[test]
fn split_writes_parts_of_the_chosen_size() {
    need!("qpdf", "pdfinfo");
    let d = tempfile::tempdir().unwrap();
    let f = pdf(d.path(), "b.pdf", 5);
    let before = hash(&f);
    let r = tool_run("splitPDF", &[&f], Some(r#"{"pages_per_file":2}"#));
    r.ok();
    let dir = r.one();
    assert_eq!(files_in(&dir), ["b-1-2.pdf", "b-3-4.pdf", "b-5.pdf"]);
    assert_eq!(page_sizes(&dir.join("b-1-2.pdf")).len(), 2);
    assert_eq!(page_sizes(&dir.join("b-5.pdf")).len(), 1);
    assert_eq!(hash(&f), before);
}

#[test]
fn split_applies_the_part_size_to_the_chosen_pages() {
    need!("qpdf", "pdfinfo");
    let d = tempfile::tempdir().unwrap();
    let f = pdf(d.path(), "b.pdf", 5);
    let r = tool_run(
        "splitPDF",
        &[&f],
        Some(r#"{"pages":"2-5","pages_per_file":3}"#),
    );
    r.ok();
    let dir = r.one();
    assert_eq!(files_in(&dir), ["b-2-4.pdf", "b-5.pdf"]);
    assert_eq!(page_sizes(&dir.join("b-2-4.pdf")).len(), 3);
}

#[test]
fn split_stays_one_page_each_by_default() {
    need!("qpdf");
    let d = tempfile::tempdir().unwrap();
    let f = pdf(d.path(), "b.pdf", 2);
    let r = tool_run("splitPDF", &[&f], None);
    r.ok();
    assert_eq!(files_in(&r.one()), ["b-1.pdf", "b-2.pdf"]);
}

#[test]
fn split_rejects_a_part_size_of_zero() {
    need!("qpdf");
    let d = tempfile::tempdir().unwrap();
    let f = pdf(d.path(), "b.pdf", 2);
    let r = tool_run("splitPDF", &[&f], Some(r#"{"pages_per_file":0}"#));
    assert_eq!(r.error_codes(), ["invalid_options"]);
}

#[test]
fn split_parts_work_without_qpdf() {
    need!("pdfseparate", "pdfunite", "pdfinfo");
    let d = tempfile::tempdir().unwrap();
    let f = pdf(d.path(), "b.pdf", 3);
    let r = tool_run_env(
        "splitPDF",
        &[&f],
        Some(r#"{"pages_per_file":2}"#),
        &[("NASHI_DISABLE_TOOLS", "qpdf")],
    );
    r.ok();
    let dir = r.one();
    assert_eq!(files_in(&dir), ["b-1-2.pdf", "b-3.pdf"]);
    assert_eq!(page_sizes(&dir.join("b-1-2.pdf")).len(), 2);
}

#[test]
fn merge_scales_pages_to_the_narrowest_and_keeps_their_shape() {
    need!("magick", "pdfinfo", "gs");
    let d = tempfile::tempdir().unwrap();
    let wide = sized_pdf(d.path(), "wide.pdf", "600x300");
    let small = sized_pdf(d.path(), "small.pdf", "200x400");
    let (a, b) = (hash(&wide), hash(&small));
    let r = tool_run(
        "mergePDF",
        &[&wide, &small],
        Some(r#"{"uniform_width":true}"#),
    );
    r.ok();
    let sizes = page_sizes(&r.one());
    assert_eq!(sizes.len(), 2);
    assert!(
        (sizes[0].0 - 200.0).abs() < 1.0 && (sizes[0].1 - 100.0).abs() < 1.0,
        "{sizes:?}"
    );
    assert!(
        (sizes[1].0 - 200.0).abs() < 1.0 && (sizes[1].1 - 400.0).abs() < 1.0,
        "{sizes:?}"
    );
    assert_eq!((hash(&wide), hash(&small)), (a, b));
}

#[test]
fn uniform_width_scales_the_content_and_does_not_crop_it() {
    need!("magick", "pdfinfo", "gs", "pdftoppm");
    let d = tempfile::tempdir().unwrap();
    let small = sized_pdf(d.path(), "small.pdf", "200x400");
    let wide = d.path().join("wide.pdf");
    // Only the right edge of the wide page is red.
    tool(
        "magick",
        &[
            "-size",
            "600x300",
            "xc:white",
            "-fill",
            "red",
            "-draw",
            "rectangle 500,0 600,300",
            "-units",
            "PixelsPerInch",
            "-density",
            "72",
            s(&wide),
        ],
    );
    let r = tool_run(
        "mergePDF",
        &[&wide, &small],
        Some(r#"{"uniform_width":true}"#),
    );
    r.ok();
    let shot = d.path().join("shot");
    tool(
        "pdftoppm",
        &[
            "-png",
            "-r",
            "72",
            "-singlefile",
            "-f",
            "1",
            "-l",
            "1",
            s(&r.one()),
            s(&shot),
        ],
    );
    let pixel = Command::new("magick")
        .arg(d.path().join("shot.png"))
        .args(["-crop", "1x1+190+50", "+repage", "txt:-"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&pixel.stdout).into_owned();
    let rgb: Vec<u32> = text
        .lines()
        .nth(1)
        .and_then(|l| l.split(['(', ')']).nth(1))
        .unwrap_or_else(|| panic!("no pixel in {text:?}"))
        .split(',')
        .map(|v| v.trim().parse().unwrap())
        .collect();
    assert!(rgb[0] > 200 && rgb[1] < 50 && rgb[2] < 50, "{rgb:?}");
}

#[test]
fn uniform_width_goes_by_how_a_rotated_page_looks() {
    need!("magick", "pdfinfo", "gs", "qpdf");
    let d = tempfile::tempdir().unwrap();
    let narrow = sized_pdf(d.path(), "narrow.pdf", "300x300");
    let flat = sized_pdf(d.path(), "flat.pdf", "800x400");
    let turned = d.path().join("turned.pdf");
    tool("qpdf", &[s(&flat), "--rotate=+90", s(&turned)]);
    // Shown as 400x800, so it becomes 300x600.
    let r = tool_run(
        "mergePDF",
        &[&narrow, &turned],
        Some(r#"{"uniform_width":true}"#),
    );
    r.ok();
    let shown: Vec<(f64, f64)> = page_shapes(&r.one());
    assert!(
        (shown[1].0 - 300.0).abs() < 1.0 && (shown[1].1 - 600.0).abs() < 1.0,
        "{shown:?}"
    );
}

#[test]
fn organize_can_make_pages_the_same_width() {
    need!("magick", "pdfinfo", "gs", "qpdf");
    let d = tempfile::tempdir().unwrap();
    let wide = sized_pdf(d.path(), "wide.pdf", "600x300");
    let small = sized_pdf(d.path(), "small.pdf", "200x400");
    let both = d.path().join("both.pdf");
    tool(
        "qpdf",
        &["--empty", "--pages", s(&wide), s(&small), "--", s(&both)],
    );
    let r = tool_run(
        "organizePDF",
        &[&both],
        Some(r#"{"pages":"2,1","uniform_width":true}"#),
    );
    r.ok();
    let sizes = page_sizes(&r.one());
    assert!(
        (sizes[0].0 - 200.0).abs() < 1.0 && (sizes[0].1 - 400.0).abs() < 1.0,
        "{sizes:?}"
    );
    assert!(
        (sizes[1].0 - 200.0).abs() < 1.0 && (sizes[1].1 - 100.0).abs() < 1.0,
        "{sizes:?}"
    );
}

#[test]
fn uniform_width_needs_ghostscript_and_says_so() {
    need!("magick", "pdfinfo");
    let d = tempfile::tempdir().unwrap();
    let wide = sized_pdf(d.path(), "wide.pdf", "600x300");
    let small = sized_pdf(d.path(), "small.pdf", "200x400");
    let r = tool_run_env(
        "mergePDF",
        &[&wide, &small],
        Some(r#"{"uniform_width":true}"#),
        &[("NASHI_DISABLE_TOOLS", "gs")],
    );
    assert_eq!(r.error_codes(), ["missing_dependency"]);
    assert!(r.outputs().is_empty());
}

#[test]
fn merge_keeps_page_sizes_unless_asked() {
    need!("magick", "pdfinfo");
    let d = tempfile::tempdir().unwrap();
    let wide = sized_pdf(d.path(), "wide.pdf", "600x300");
    let small = sized_pdf(d.path(), "small.pdf", "200x400");
    let r = tool_run("mergePDF", &[&wide, &small], None);
    r.ok();
    let sizes = page_sizes(&r.one());
    assert!(
        (sizes[0].0 - 600.0).abs() < 1.0 && (sizes[1].0 - 200.0).abs() < 1.0,
        "{sizes:?}"
    );
}

fn integrated_lufs(p: &Path) -> f64 {
    let o = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(p)
        .args(["-af", "ebur128=framelog=quiet", "-f", "null", "-"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stderr).into_owned();
    let line = text
        .lines()
        .rfind(|l| l.trim_start().starts_with("I:"))
        .unwrap();
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[test]
fn normalize_reaches_the_chosen_loudness() {
    need!("ffmpeg");
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("speech.wav");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "anoisesrc=d=12:c=pink:a=0.03",
            s(&src),
        ],
    );
    for target in [-23.0, -14.0] {
        let r = tool_run(
            "normalizeAudio",
            &[&src],
            Some(&format!(r#"{{"target_lufs":{target}}}"#)),
        );
        r.ok();
        let got = integrated_lufs(&r.one());
        assert!((got - target).abs() < 1.5, "target {target}, got {got}");
    }
}

#[test]
fn normalize_rejects_values_out_of_range() {
    need!("ffmpeg");
    let d = tempfile::tempdir().unwrap();
    let src = wav(d.path(), "a.wav", 1.0);
    for bad in [
        r#"{"target_lufs":-71}"#,
        r#"{"target_lufs":-4}"#,
        r#"{"loudness_range":0.5}"#,
        r#"{"loudness_range":51}"#,
        r#"{"true_peak":-10}"#,
        r#"{"true_peak":1}"#,
    ] {
        let r = tool_run("normalizeAudio", &[&src], Some(bad));
        assert_eq!(r.error_codes(), ["invalid_options"], "{bad}");
    }
}

fn loudness_range(p: &Path) -> f64 {
    let o = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(p)
        .args(["-af", "ebur128=framelog=quiet", "-f", "null", "-"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stderr).into_owned();
    let line = text
        .lines()
        .rfind(|l| l.trim_start().starts_with("LRA:"))
        .unwrap();
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[test]
fn normalize_keeps_quiet_and_loud_parts_apart() {
    need!("ffmpeg");
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("dynamic.wav");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "anoisesrc=d=6:c=pink:a=0.02",
            "-f",
            "lavfi",
            "-i",
            "anoisesrc=d=6:c=pink:a=0.5",
            "-filter_complex",
            "[0][1]concat=n=2:v=0:a=1",
            s(&src),
        ],
    );
    let before = loudness_range(&src);
    let r = tool_run("normalizeAudio", &[&src], None);
    r.ok();
    let out = r.one();
    assert!(
        (loudness_range(&out) - before).abs() < 1.5,
        "{before} -> {}",
        loudness_range(&out)
    );
    assert!(
        (integrated_lufs(&out) + 16.0).abs() < 1.5,
        "{}",
        integrated_lufs(&out)
    );
}

#[test]
fn normalize_handles_silence() {
    need!("ffmpeg");
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("silent.wav");
    tool(
        "ffmpeg",
        &["-v", "error", "-f", "lavfi", "-i", "anullsrc=d=2", s(&src)],
    );
    let r = tool_run("normalizeAudio", &[&src], None);
    r.ok();
    r.one();
}

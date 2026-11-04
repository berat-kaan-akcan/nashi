//! Edit Image adjustments must change the picture the way their names promise.
//! Skipped (not failed) when an external tool is missing.

mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fx(p: &Path, expr: &str) -> f64 {
    let out = Command::new("magick")
        .args([s(p), "-format", &format!("%[fx:{expr}]"), "info:"])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    text.trim()
        .parse()
        .unwrap_or_else(|_| panic!("{expr}: {text:?}"))
}

/// A mid-toned, colourful gradient with detail.
fn photo(dir: &Path) -> PathBuf {
    let p = dir.join("photo.png");
    tool(
        "magick",
        &[
            "-size",
            "128x128",
            "gradient:#a06040-#4080a0",
            "(",
            "-size",
            "128x128",
            "pattern:checkerboard",
            ")",
            "-define",
            "compose:args=20",
            "-compose",
            "Blend",
            "-composite",
            "-depth",
            "8",
            s(&p),
        ],
    );
    p
}

fn adjusted(input: &Path, adjust: &str) -> PathBuf {
    let r = tool_run(
        "editImage",
        &[input],
        Some(&format!(r#"{{"adjust":{adjust}}}"#)),
    );
    r.ok();
    let out = r.one();
    let kept = input.with_file_name(format!("out-{}.png", hash(&out)));
    std::fs::rename(&out, &kept).unwrap();
    kept
}

#[test]
fn brightness_contrast_and_exposure_move_the_tones() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    let (mean, spread) = (fx(&p, "mean"), fx(&p, "standard_deviation"));

    let brighter = adjusted(&p, r#"{"brightness":40}"#);
    assert!(fx(&brighter, "mean") > mean + 0.05);
    let darker = adjusted(&p, r#"{"brightness":-40}"#);
    assert!(fx(&darker, "mean") < mean - 0.05);
    let contrasty = adjusted(&p, r#"{"contrast":60}"#);
    assert!(fx(&contrasty, "standard_deviation") > spread * 1.2);
    let flat = adjusted(&p, r#"{"contrast":-60}"#);
    assert!(fx(&flat, "standard_deviation") < spread * 0.8);
    let stop_up = adjusted(&p, r#"{"exposure":1}"#);
    let ratio = fx(&stop_up, "mean") / mean;
    assert!((1.6..=2.1).contains(&ratio), "one stop gave {ratio}");
    let gamma = adjusted(&p, r#"{"gamma":2}"#);
    assert!(fx(&gamma, "mean") > mean + 0.05);
}

#[test]
fn colour_controls_change_colour() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    let gap = |p: &Path| (fx(p, "mean.r") - fx(p, "mean.b")).abs();
    let warm = adjusted(&p, r#"{"temperature":60}"#);
    assert!(fx(&warm, "mean.r") > fx(&p, "mean.r"));
    assert!(fx(&warm, "mean.b") < fx(&p, "mean.b"));
    let cool = adjusted(&p, r#"{"temperature":-60}"#);
    assert!(fx(&cool, "mean.b") > fx(&p, "mean.b"));

    let corner = |p: &Path| fx(p, "abs(p{0,0}.r-p{0,0}.b)");
    assert!(corner(&adjusted(&p, r#"{"saturation":-100}"#)) < 0.01);
    assert!(corner(&adjusted(&p, r#"{"saturation":60}"#)) > corner(&p));
    assert!(gap(&adjusted(&p, r#"{"hue":50}"#)) != gap(&p));

    assert!(corner(&adjusted(&p, r#"{"filter":"grayscale"}"#)) < 0.01);
    let sepia = adjusted(&p, r#"{"filter":"sepia"}"#);
    assert!(fx(&sepia, "mean.r") > fx(&sepia, "mean.b") + 0.02);
}

#[test]
fn detail_controls_change_detail() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    let detail = |p: &Path| {
        let edges = p.with_extension("edges.png");
        tool(
            "magick",
            &[
                s(p),
                "-colorspace",
                "Gray",
                "-define",
                "convolve:scale=1",
                "-morphology",
                "Convolve",
                "Laplacian:0",
                s(&edges),
            ],
        );
        fx(&edges, "mean")
    };
    let base = detail(&p);
    assert!(detail(&adjusted(&p, r#"{"sharpen":80}"#)) > base);
    assert!(detail(&adjusted(&p, r#"{"blur":40}"#)) < base);
    let flat = d.path().join("flat.png");
    tool("magick", &["-size", "64x64", "xc:#808080", s(&flat)]);
    let grainy = adjusted(&flat, r#"{"grain":60}"#);
    assert!(fx(&grainy, "standard_deviation") > fx(&flat, "standard_deviation") + 0.01);
}

#[test]
fn adjustments_keep_transparent_areas_transparent() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("cutout.png");
    tool(
        "magick",
        &[
            "-size",
            "64x64",
            "xc:none",
            "-fill",
            "#a06040",
            "-draw",
            "rectangle 16,16 47,47",
            s(&p),
        ],
    );
    for adjust in [
        r#"{"brightness":40,"contrast":40}"#,
        r#"{"exposure":1}"#,
        r#"{"gamma":2}"#,
        r#"{"temperature":50}"#,
        r#"{"saturation":50,"hue":20}"#,
        r#"{"sharpen":50}"#,
        r#"{"grain":50}"#,
        r#"{"filter":"sepia"}"#,
    ] {
        let out = adjusted(&p, adjust);
        assert!(
            fx(&out, "p{1,1}.a") < 0.01,
            "{adjust}: corner became visible"
        );
        assert!(fx(&out, "p{32,32}.a") > 0.99, "{adjust}: centre faded");
    }
}

#[test]
fn neutral_or_invalid_adjustments_are_refused() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    for options in [
        r#"{"adjust":{}}"#,
        r#"{"adjust":{"brightness":0,"exposure":0,"gamma":1}}"#,
        r#"{"adjust":{"brightness":500}}"#,
        r#"{"adjust":{"exposure":9}}"#,
        r#"{"adjust":{"gamma":0}}"#,
        r#"{"adjust":{"blur":-1}}"#,
        r#"{"adjust":{"filter":"polaroid"}}"#,
    ] {
        let r = tool_run("editImage", &[&p], Some(options));
        assert_eq!(r.error_codes(), ["invalid_options"], "{options}");
    }
}

#[test]
fn adjustments_combine_with_rotation_and_resize() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    let r = tool_run(
        "editImage",
        &[&p],
        Some(r#"{"rotate":90,"resize":{"w":64},"adjust":{"brightness":30}}"#),
    );
    r.ok();
    assert_eq!(fx(&r.one(), "w"), 64.0);
    assert!(fx(&r.one(), "mean") > fx(&p, "mean"));
}

#[test]
fn a_preview_can_show_the_adjustments_without_touching_the_input() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    let before = hash(&p);
    let render = |extra: &[&str]| -> PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        let mut args = vec![
            "preview",
            "--json",
            s(&p),
            "--kind",
            "frame",
            "--out-dir",
            s(&dir),
        ];
        args.extend_from_slice(extra);
        let r = run(&args);
        assert_eq!(r.code, 0, "{}", r.stdout);
        let j: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
        PathBuf::from(j["files"][0].as_str().unwrap())
    };
    let plain = render(&[]);
    let lit = render(&["--adjust", r#"{"brightness":40}"#]);
    assert!(fx(&lit, "mean") > fx(&plain, "mean") + 0.05);
    assert_eq!(hash(&p), before);

    let dir = d.path().join("bad");
    let r = run(&[
        "preview",
        "--json",
        s(&p),
        "--kind",
        "frame",
        "--out-dir",
        s(&dir),
        "--adjust",
        r#"{"exposure":9}"#,
    ]);
    assert_eq!(r.code, 1, "{}", r.stdout);
}

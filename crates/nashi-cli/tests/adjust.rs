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

fn ramp(dir: &Path) -> PathBuf {
    let p = dir.join("ramp.png");
    tool(
        "magick",
        &[
            "-size",
            "256x64",
            "-define",
            "gradient:angle=90",
            "gradient:black-white",
            "-colorspace",
            "sRGB",
            "-type",
            "TrueColor",
            s(&p),
        ],
    );
    p
}

fn green_at(p: &Path, x: u32, y: u32) -> f64 {
    fx(p, &format!("p{{{x},{y}}}.g"))
}

#[test]
fn tonal_ranges_move_only_their_own_tones() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = ramp(d.path());
    let (dark, mid, bright) = ((40, 32), (128, 32), (216, 32));

    let shadows = adjusted(&p, r#"{"shadows":80}"#);
    assert!(green_at(&shadows, dark.0, dark.1) > green_at(&p, dark.0, dark.1) + 0.05);
    assert!(
        (green_at(&shadows, bright.0, bright.1) - green_at(&p, bright.0, bright.1)).abs() < 0.03
    );

    let highlights = adjusted(&p, r#"{"highlights":-80}"#);
    assert!(green_at(&highlights, bright.0, bright.1) < green_at(&p, bright.0, bright.1) - 0.05);
    assert!((green_at(&highlights, dark.0, dark.1) - green_at(&p, dark.0, dark.1)).abs() < 0.03);

    let blacks = adjusted(&p, r#"{"blacks":100}"#);
    assert!(green_at(&blacks, 2, 32) > 0.15);
    let whites = adjusted(&p, r#"{"whites":-100}"#);
    assert!(green_at(&whites, 253, 32) < 0.85);
    assert!((green_at(&whites, mid.0, mid.1) - green_at(&p, mid.0, mid.1)).abs() < 0.03);
}

#[test]
fn fade_lifts_the_blacks_and_lowers_the_whites() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = ramp(d.path());
    let faded = adjusted(&p, r#"{"fade":100}"#);
    assert!(green_at(&faded, 1, 32) > 0.1);
    assert!(green_at(&faded, 254, 32) < 0.85);
}

#[test]
fn tint_moves_green_against_magenta() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    let magenta = adjusted(&p, r#"{"tint":60}"#);
    assert!(fx(&magenta, "mean.g") < fx(&p, "mean.g") - 0.02);
    assert!((fx(&magenta, "mean.r") - fx(&p, "mean.r")).abs() < 0.01);
    let green = adjusted(&p, r#"{"tint":-60}"#);
    assert!(fx(&green, "mean.g") > fx(&p, "mean.g") + 0.02);
}

#[test]
fn vibrance_favours_muted_colours() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("swatches.png");
    tool(
        "magick",
        &[
            "-size",
            "20x20",
            "xc:#907070",
            "xc:#ff0000",
            "xc:#808080",
            "+append",
            "-type",
            "TrueColor",
            s(&p),
        ],
    );
    let v = adjusted(&p, r#"{"vibrance":100}"#);
    let spread =
        |f: &Path, x: u32| fx(f, &format!("p{{{x},10}}.r")) - fx(f, &format!("p{{{x},10}}.b"));
    assert!(
        spread(&v, 10) > spread(&p, 10) + 0.05,
        "muted colour got stronger"
    );
    assert!((spread(&v, 30) - 1.0).abs() < 0.02, "pure colour stays");
    assert!(spread(&v, 50).abs() < 0.02, "gray stays gray");
    let less = adjusted(&p, r#"{"vibrance":-100}"#);
    assert!(spread(&less, 10) < spread(&p, 10) - 0.03);
}

#[test]
fn dehaze_clarity_and_texture_change_local_contrast() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    let spread = fx(&p, "standard_deviation");
    for strong in [r#"{"dehaze":80}"#, r#"{"clarity":80}"#, r#"{"texture":80}"#] {
        assert!(
            fx(&adjusted(&p, strong), "standard_deviation") > spread + 0.002,
            "{strong}"
        );
    }
    for soft in [r#"{"dehaze":-80}"#, r#"{"clarity":-80}"#] {
        assert!(
            fx(&adjusted(&p, soft), "standard_deviation") < spread - 0.002,
            "{soft}"
        );
    }
}

#[test]
fn noise_reduction_and_negative_texture_smooth_noise() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("noisy.png");
    tool(
        "magick",
        &[
            "-size",
            "128x128",
            "xc:#808080",
            "-attenuate",
            "0.5",
            "+noise",
            "Gaussian",
            "-type",
            "TrueColor",
            s(&p),
        ],
    );
    let noise = fx(&p, "standard_deviation");
    for smooth in [r#"{"noise_reduction":100}"#, r#"{"texture":-100}"#] {
        assert!(
            fx(&adjusted(&p, smooth), "standard_deviation") < noise * 0.9,
            "{smooth}"
        );
    }
}

#[test]
fn vignette_changes_the_corners_and_keeps_the_centre_and_transparency() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("gray.png");
    tool(
        "magick",
        &[
            "-size",
            "128x128",
            "xc:#808080",
            "(",
            "-size",
            "128x128",
            "xc:none",
            "-fill",
            "white",
            "-draw",
            "rectangle 0,0 127,63",
            ")",
            "-compose",
            "CopyOpacity",
            "-composite",
            "-type",
            "TrueColorAlpha",
            s(&p),
        ],
    );
    let dark = adjusted(&p, r#"{"vignette":100}"#);
    assert!(green_at(&dark, 1, 1) < 0.3);
    assert!(green_at(&dark, 64, 62) > 0.45);
    let light = adjusted(&p, r#"{"vignette":-100}"#);
    assert!(green_at(&light, 1, 1) > 0.7);
    for f in [&dark, &light] {
        assert!(fx(f, "p{5,120}.a") < 0.01, "transparent stays transparent");
        assert!(fx(f, "p{5,5}.a") > 0.99, "opaque stays opaque");
    }
}

#[test]
fn a_vignette_keeps_every_page_of_a_document() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("pages.tiff");
    tool(
        "magick",
        &[
            "-size",
            "40x40",
            "xc:#808080",
            "xc:#808080",
            "xc:#808080",
            s(&p),
        ],
    );
    let r = tool_run("editImage", &[&p], Some(r#"{"adjust":{"vignette":100}}"#));
    r.ok();
    let out = r.one();
    let frames = Command::new("magick")
        .args(["identify", "-format", "%n\n", s(&out)])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&frames.stdout).lines().next(),
        Some("3")
    );
}

#[test]
fn new_adjustments_refuse_values_out_of_range() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = photo(d.path());
    for bad in [
        r#"{"tint":101}"#,
        r#"{"vibrance":-101}"#,
        r#"{"highlights":101}"#,
        r#"{"shadows":101}"#,
        r#"{"whites":101}"#,
        r#"{"blacks":101}"#,
        r#"{"fade":-1}"#,
        r#"{"clarity":101}"#,
        r#"{"texture":101}"#,
        r#"{"dehaze":101}"#,
        r#"{"noise_reduction":-1}"#,
        r#"{"vignette":101}"#,
    ] {
        let r = tool_run("editImage", &[&p], Some(&format!(r#"{{"adjust":{bad}}}"#)));
        assert_eq!(r.error_codes(), ["invalid_options"], "{bad}");
    }
}

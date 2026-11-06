//! `nashi-cli suggest`: places to redact, found by reading the text with OCR.
//! Skipped (not failed) when tesseract, ImageMagick or a sans font is missing.

mod common;
use common::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn font() -> Option<String> {
    let o = Command::new("fc-match")
        .args(["-f", "%{file}", "sans-serif:lang=en"])
        .output()
        .ok()?;
    let f = String::from_utf8_lossy(&o.stdout).into_owned();
    Path::new(&f).is_file().then_some(f)
}

fn ocr_ready() -> bool {
    have(&["tesseract", "magick"], "suggest") && {
        let l = Command::new("tesseract")
            .arg("--list-langs")
            .output()
            .unwrap();
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&l.stdout),
            String::from_utf8_lossy(&l.stderr)
        );
        let ok = all.lines().any(|l| l.trim() == "eng");
        if !ok {
            skipped("suggest (tesseract eng data missing)");
        }
        ok
    }
}

/// A white picture with the given lines of text, one every 70 pixels.
fn text_png(dir: &Path, lines: &[&str]) -> Option<PathBuf> {
    let Some(f) = font() else {
        skipped("suggest (no sans font)");
        return None;
    };
    let out = dir.join("text.png");
    let mut args = vec![
        "-size".to_string(),
        "1000x400".into(),
        "xc:white".into(),
        "-font".into(),
        f,
        "-fill".into(),
        "black".into(),
        "-pointsize".into(),
        "32".into(),
    ];
    for (i, l) in lines.iter().enumerate() {
        args.extend([
            "-annotate".into(),
            format!("+30+{}", 60 + i * 70),
            (*l).into(),
        ]);
    }
    args.push(s(&out).into());
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let o = Command::new("magick")
        .env("LC_ALL", "C")
        .args(&a)
        .output()
        .unwrap();
    assert!(o.status.success());
    Some(out)
}

fn suggest(args: &[&str]) -> (i32, Value) {
    let mut a = vec!["suggest"];
    a.extend(args);
    a.push("--json");
    let r = run(&a);
    let v = serde_json::from_str(r.stdout.lines().next().unwrap_or("null")).unwrap_or(Value::Null);
    (r.code, v)
}

fn kinds(v: &Value) -> Vec<String> {
    v["rects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["kind"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn marks_email_phone_and_iban_but_not_ordinary_text() {
    if !ocr_ready() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let Some(img) = text_png(
        d.path(),
        &[
            "Mail ayse.yilmaz@mail.com",
            "Tel +90 532 123 45 67",
            "IBAN GB82 WEST 1234 5698 7654 32",
            "Just an ordinary line of words",
        ],
    ) else {
        return;
    };
    let (code, v) = suggest(&[s(&img)]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        (v["width"].as_u64(), v["height"].as_u64()),
        (Some(1000), Some(400))
    );
    let mut k = kinds(&v);
    k.sort();
    assert_eq!(k, ["email", "iban", "phone"], "{v}");
    // Each box sits on its own line: lines are 70 px apart, the 4th has none.
    let rects = v["rects"].as_array().unwrap();
    let mut ys: Vec<u64> = rects
        .iter()
        .map(|r| r["y"].as_u64().unwrap() / 70)
        .collect();
    ys.sort();
    assert_eq!(ys, [0, 1, 2], "{v}");
    for r in rects {
        assert!(r["x"].as_u64().unwrap() + r["w"].as_u64().unwrap() <= 1000);
        assert!(r["w"].as_u64().unwrap() > 50);
    }
    // The text itself never leaves the engine.
    assert!(!v.to_string().contains("ayse"));
}

#[test]
fn kinds_narrow_the_search_and_unknown_kinds_are_refused() {
    if !ocr_ready() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let Some(img) = text_png(
        d.path(),
        &["Mail ayse.yilmaz@mail.com", "Tel +90 532 123 45 67"],
    ) else {
        return;
    };
    let (code, v) = suggest(&[s(&img), "--kinds", "email"]);
    assert_eq!(code, 0);
    assert_eq!(kinds(&v), ["email"]);
    let (code, v) = suggest(&[s(&img), "--kinds", "everything"]);
    assert_eq!(code, 1);
    assert_eq!(v["error"]["code"], "invalid_options");
}

#[test]
fn a_picture_without_text_has_no_suggestions_and_other_files_are_refused() {
    if !ocr_ready() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let blank = d.path().join("blank.png");
    tool("magick", &["-size", "300x200", "xc:white", s(&blank)]);
    let (code, v) = suggest(&[s(&blank)]);
    assert_eq!(code, 0);
    assert!(kinds(&v).is_empty());
    let doc = text(d.path(), "a.txt", "hello");
    let (code, v) = suggest(&[s(&doc)]);
    assert_eq!(code, 1);
    assert_eq!(v["error"]["code"], "invalid_input");
}

#[test]
fn reads_a_video_frame_and_leaves_the_input_alone() {
    if !ocr_ready() || !have(&["ffmpeg"], "suggest video") {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let Some(img) = text_png(d.path(), &["Mail ayse.yilmaz@mail.com"]) else {
        return;
    };
    let video = d.path().join("v.mp4");
    tool(
        "ffmpeg",
        &[
            "-loglevel",
            "error",
            "-loop",
            "1",
            "-i",
            s(&img),
            "-t",
            "1",
            "-r",
            "5",
            "-pix_fmt",
            "yuv420p",
            "-y",
            s(&video),
        ],
    );
    let before = hash(&video);
    let (code, v) = suggest(&[s(&video), "--at", "0.4"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(kinds(&v), ["email"], "{v}");
    assert_eq!(hash(&video), before);
    assert_no_temp(d.path());
}

fn opencv_ready() -> bool {
    if !have(&["python3", "magick"], "suggest faces") {
        return false;
    }
    let ok = Command::new("python3")
        .args(["-I", "-c", "import cv2"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        skipped("suggest faces (python OpenCV missing)");
    }
    ok
}

/// A flat, drawn face: skin-coloured oval, two eyes, a nose and a mouth.
fn face_png(dir: &Path) -> PathBuf {
    let out = dir.join("face.png");
    tool(
        "magick",
        &[
            "-size",
            "500x500",
            "xc:#8899aa",
            "-fill",
            "#e0b090",
            "-draw",
            "ellipse 250,260 120,160 0,360",
            "-fill",
            "white",
            "-draw",
            "ellipse 205,220 28,16 0,360",
            "-draw",
            "ellipse 295,220 28,16 0,360",
            "-fill",
            "#303030",
            "-draw",
            "circle 205,222 205,232",
            "-draw",
            "circle 295,222 295,232",
            "-stroke",
            "#704030",
            "-strokewidth",
            "6",
            "-draw",
            "line 250,235 240,290",
            "-draw",
            "line 240,290 262,292",
            "-stroke",
            "#a03030",
            "-strokewidth",
            "9",
            "-draw",
            "line 205,340 295,340",
            "-blur",
            "0x2",
            s(&out),
        ],
    );
    out
}

#[test]
fn finds_a_face_and_marks_it_around_the_middle_of_the_picture() {
    if !opencv_ready() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let img = face_png(d.path());
    let (code, v) = suggest(&[s(&img), "--kinds", "face"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(kinds(&v), ["face"], "{v}");
    let r = &v["rects"][0];
    let (x, y, w, h) = (
        r["x"].as_u64().unwrap(),
        r["y"].as_u64().unwrap(),
        r["w"].as_u64().unwrap(),
        r["h"].as_u64().unwrap(),
    );
    // The oval is at 250,260; the box has to cover its middle and stay in the picture.
    assert!(x < 250 && 250 < x + w && y < 260 && 260 < y + h, "{v}");
    assert!(x + w <= 500 && y + h <= 500, "{v}");
}

#[test]
fn no_face_in_text_and_faces_are_part_of_the_default_search() {
    if !opencv_ready() || !ocr_ready() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let Some(text) = text_png(d.path(), &["Nothing personal here"]) else {
        return;
    };
    let (code, v) = suggest(&[s(&text), "--kinds", "face"]);
    assert_eq!(code, 0);
    assert!(kinds(&v).is_empty(), "{v}");
    let (code, v) = suggest(&[s(&face_png(d.path()))]);
    assert_eq!(code, 0);
    assert!(kinds(&v).contains(&"face".to_string()), "{v}");
}

#[test]
fn asking_for_faces_without_opencv_says_what_is_missing() {
    if !have(&["magick"], "suggest faces") {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let img = d.path().join("a.png");
    tool("magick", &["-size", "50x50", "xc:white", s(&img)]);
    let r = run_env(
        &["suggest", s(&img), "--kinds", "face", "--json"],
        &[("NASHI_DISABLE_TOOLS", "opencv")],
    );
    assert_eq!(r.code, 3, "{}", r.stdout);
    let v: Value = serde_json::from_str(r.stdout.lines().next().unwrap()).unwrap();
    assert_eq!(v["error"]["code"], "missing_dependency");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("python-opencv"));
}

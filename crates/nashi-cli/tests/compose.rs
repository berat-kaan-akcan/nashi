//! Frame Image and Create Collage lay pictures out where they were asked to go.
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

fn size(p: &Path) -> (f64, f64) {
    (fx(p, "w"), fx(p, "h"))
}

/// (red, green, blue) of one pixel, each 0..1.
fn at(p: &Path, x: u32, y: u32) -> (f64, f64, f64) {
    let c = |ch: &str| fx(p, &format!("p{{{x},{y}}}.{ch}"));
    (c("r"), c("g"), c("b"))
}

fn is_red(c: (f64, f64, f64)) -> bool {
    c.0 > 0.9 && c.1 < 0.1 && c.2 < 0.1
}
fn is_blue(c: (f64, f64, f64)) -> bool {
    c.0 < 0.1 && c.1 < 0.1 && c.2 > 0.9
}
fn is_white(c: (f64, f64, f64)) -> bool {
    c.0 > 0.95 && c.1 > 0.95 && c.2 > 0.95
}

fn solid(dir: &Path, name: &str, color: &str, w: u32, h: u32) -> PathBuf {
    let p = dir.join(name);
    tool(
        "magick",
        &["-size", &format!("{w}x{h}"), &format!("xc:{color}"), s(&p)],
    );
    p
}

fn framed(input: &Path, frame: &str) -> PathBuf {
    let r = tool_run(
        "frameImage",
        &[input],
        Some(&format!(r#"{{"frame":{frame}}}"#)),
    );
    r.ok();
    let out = r.one();
    let kept = input.with_file_name(format!("framed-{}.png", hash(&out)));
    std::fs::rename(&out, &kept).unwrap();
    kept
}

#[test]
fn padding_and_colour_surround_the_image() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = solid(d.path(), "red.png", "red", 100, 60);
    let out = framed(&p, r##"{"padding":20,"color":"#0000ff"}"##);
    assert_eq!(size(&out), (140.0, 100.0));
    assert!(is_blue(at(&out, 2, 2)));
    assert!(is_red(at(&out, 20, 20)), "image starts at the padding");
    assert!(is_red(at(&out, 119, 79)), "image ends at the padding");
    assert!(is_blue(at(&out, 120, 80)));
}

#[test]
fn gradient_runs_between_its_colours_along_the_angle() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = solid(d.path(), "red.png", "red", 40, 40);
    let across = framed(
        &p,
        r##"{"padding":60,"backdrop":"gradient","color":"#000000","color2":"#ffffff","angle":90}"##,
    );
    let (left, right) = (at(&across, 2, 80), at(&across, 157, 80));
    assert!(left.0 < 0.1 && right.0 > 0.9, "{left:?} {right:?}");
    let (top, bottom) = (at(&across, 80, 2), at(&across, 80, 157));
    assert!((top.0 - bottom.0).abs() < 0.05, "{top:?} {bottom:?}");
}

#[test]
fn an_image_backdrop_fills_the_canvas() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = solid(d.path(), "red.png", "red", 40, 40);
    let back = solid(d.path(), "back.png", "#0000ff", 10, 30);
    let frame = format!(
        r#"{{"padding":20,"backdrop":"image","background_image":"{}","background_blur":10}}"#,
        back.display()
    );
    let out = framed(&p, &frame);
    assert_eq!(size(&out), (80.0, 80.0));
    assert!(is_blue(at(&out, 1, 1)));
    assert!(is_red(at(&out, 40, 40)));
}

#[test]
fn rounded_corners_show_the_backdrop() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = solid(d.path(), "red.png", "red", 100, 100);
    let out = framed(
        &p,
        r##"{"padding":10,"color":"#ffffff","corner_radius":30}"##,
    );
    assert!(is_white(at(&out, 11, 11)), "corner cut");
    assert!(is_red(at(&out, 60, 60)), "middle kept");
    assert!(is_red(at(&out, 60, 11)), "edge kept");
}

#[test]
fn a_shadow_falls_around_the_image_only() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = solid(d.path(), "red.png", "red", 60, 60);
    let out = framed(
        &p,
        r##"{"padding":40,"color":"#ffffff","shadow":10,"shadow_opacity":80}"##,
    );
    let near = at(&out, 70, 104);
    assert!(near.0 < 0.8, "no shadow below the image: {near:?}");
    assert!(is_white(at(&out, 1, 1)), "shadow reached the far corner");
    assert!(is_red(at(&out, 70, 70)), "shadow drawn over the image");
}

#[test]
fn aspect_scale_and_position_place_the_image() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = solid(d.path(), "red.png", "red", 100, 60);
    let square = framed(&p, r##"{"padding":20,"aspect":1,"color":"#ffffff"}"##);
    assert_eq!(size(&square), (140.0, 140.0));
    assert!(is_red(at(&square, 70, 70)));
    assert!(is_white(at(&square, 70, 10)));

    let corner = framed(
        &p,
        r##"{"padding":20,"color":"#ffffff","image_scale":0.5,"position":{"x":0,"y":0}}"##,
    );
    assert_eq!(size(&corner), (140.0, 100.0));
    assert!(is_red(at(&corner, 1, 1)));
    assert!(is_red(at(&corner, 49, 29)));
    assert!(is_white(at(&corner, 50, 30)));
}

#[test]
fn frame_works_for_jpeg_and_keeps_the_old_border_option() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let j = jpg(d.path(), "photo.jpg");
    let r = tool_run(
        "frameImage",
        &[&j],
        Some(r#"{"frame":{"padding":10,"corner_radius":8}}"#),
    );
    r.ok();
    assert_eq!(size(&r.one()), (84.0, 68.0));
    let p = solid(d.path(), "red.png", "red", 20, 20);
    let r = tool_run(
        "frameImage",
        &[&p],
        Some(r##"{"border":{"px":5,"color":"#0000ff"}}"##),
    );
    r.ok();
    assert_eq!(size(&r.one()), (30.0, 30.0));
}

#[test]
fn bad_frames_are_refused() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = solid(d.path(), "red.png", "red", 20, 20);
    for frame in [
        r#"{"backdrop":"plaid"}"#,
        r#"{"color":"url(x)"}"#,
        r#"{"backdrop":"image"}"#,
        r#"{"backdrop":"image","background_image":"/no/such/file.png"}"#,
        r#"{"aspect":0}"#,
        r#"{"image_scale":0}"#,
        r#"{"image_scale":2}"#,
        r#"{"position":{"x":2,"y":0}}"#,
        r#"{"padding":999999}"#,
    ] {
        let r = tool_run(
            "frameImage",
            &[&p],
            Some(&format!(r#"{{"frame":{frame}}}"#)),
        );
        assert_eq!(r.error_codes(), ["invalid_options"], "{frame}");
    }
}

fn collage(inputs: &[&Path], options: &str) -> PathBuf {
    let r = tool_run(
        "createCollage",
        inputs,
        Some(&format!(r#"{{"collage":{options}}}"#)),
    );
    r.ok();
    r.one()
}

#[test]
fn row_and_column_share_the_width() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let a = solid(d.path(), "a.png", "red", 50, 50);
    let b = solid(d.path(), "b.png", "blue", 50, 50);
    let row = collage(
        &[&a, &b],
        r##"{"layout":"row","width":410,"spacing":10,"color":"#ffffff"}"##,
    );
    assert_eq!(size(&row), (410.0, 200.0));
    assert!(is_red(at(&row, 100, 100)));
    assert!(is_white(at(&row, 205, 100)));
    assert!(is_blue(at(&row, 300, 100)));

    let column = collage(&[&a, &b], r#"{"layout":"column","width":200,"aspect":2}"#);
    assert_eq!(size(&column), (200.0, 200.0));
    assert!(is_red(at(&column, 100, 50)));
    assert!(is_blue(at(&column, 100, 150)));
}

#[test]
fn grid_wraps_and_order_can_be_changed() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let a = solid(d.path(), "a.png", "red", 50, 50);
    let b = solid(d.path(), "b.png", "blue", 50, 50);
    let c = solid(d.path(), "c.png", "lime", 50, 50);
    let grid = collage(
        &[&a, &b, &c],
        r#"{"layout":"grid","columns":2,"width":200}"#,
    );
    assert_eq!(size(&grid), (200.0, 200.0));
    assert!(is_red(at(&grid, 50, 50)));
    assert!(is_blue(at(&grid, 150, 50)));
    let swapped = collage(&[&a, &b], r#"{"layout":"row","width":200,"order":[1,0]}"#);
    assert!(is_blue(at(&swapped, 50, 50)));
    assert!(is_red(at(&swapped, 150, 50)));
}

#[test]
fn featured_gives_the_first_image_the_large_cell() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let a = solid(d.path(), "a.png", "red", 50, 50);
    let b = solid(d.path(), "b.png", "blue", 50, 50);
    let c = solid(d.path(), "c.png", "lime", 50, 50);
    let out = collage(&[&a, &b, &c], r#"{"layout":"featured","width":300}"#);
    assert!(is_red(at(&out, 100, 100)), "large cell on the left");
    assert!(is_blue(at(&out, 250, 20)), "first small cell");
    let h = fx(&out, "h") as u32;
    let bottom = at(&out, 250, h - 20);
    assert!(
        bottom.1 > 0.9 && bottom.0 < 0.1,
        "second small cell {bottom:?}"
    );
}

#[test]
fn fit_shows_the_whole_image_and_fill_crops_it() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let wide = solid(d.path(), "wide.png", "red", 200, 50);
    let filled = collage(
        &[&wide],
        r##"{"layout":"row","width":200,"color":"#ffffff"}"##,
    );
    assert!(is_red(at(&filled, 100, 5)));
    let fitted = collage(
        &[&wide],
        r##"{"layout":"row","width":200,"fit":"fit","color":"#ffffff"}"##,
    );
    assert_eq!(size(&fitted), (200.0, 200.0));
    assert!(is_white(at(&fitted, 100, 5)));
    assert!(is_red(at(&fitted, 100, 100)));
}

#[test]
fn collage_without_options_still_works() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let a = solid(d.path(), "a.png", "red", 50, 50);
    let b = solid(d.path(), "b.png", "blue", 50, 50);
    tool_run("createCollage", &[&a, &b], None).ok();
}

#[test]
fn bad_collages_are_refused() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let a = solid(d.path(), "a.png", "red", 50, 50);
    let b = solid(d.path(), "b.png", "blue", 50, 50);
    for options in [
        r#"{"layout":"spiral"}"#,
        r#"{"layout":"grid","columns":0}"#,
        r#"{"width":0}"#,
        r#"{"width":90000}"#,
        r#"{"aspect":0}"#,
        r#"{"fit":"stretch"}"#,
        r#"{"color":"url(x)"}"#,
        r#"{"order":[0,0]}"#,
        r#"{"order":[0]}"#,
        r#"{"order":[0,5]}"#,
        r#"{"width":10,"padding":50}"#,
    ] {
        let r = tool_run(
            "createCollage",
            &[&a, &b],
            Some(&format!(r#"{{"collage":{options}}}"#)),
        );
        assert_eq!(r.error_codes(), ["invalid_options"], "{options}");
    }
}

#[test]
fn a_third_colour_passes_through_the_middle_of_the_gradient() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = solid(d.path(), "white.png", "white", 10, 10);
    let out = framed(
        &p,
        r##"{"padding":100,"backdrop":"gradient","color":"#ff0000","color2":"#00ff00","color3":"#0000ff","angle":90}"##,
    );
    let (left, top_mid, right) = (at(&out, 1, 105), at(&out, 105, 2), at(&out, 208, 105));
    assert!(left.0 > 0.9 && left.1 < 0.1, "{left:?}");
    assert!(top_mid.1 > 0.9 && top_mid.0 < 0.1, "{top_mid:?}");
    assert!(right.2 > 0.9 && right.1 < 0.1, "{right:?}");
    let bad = tool_run(
        "frameImage",
        &[&p],
        Some(r##"{"frame":{"backdrop":"gradient","color3":"#zz"}}"##),
    );
    assert_eq!(bad.error_codes(), ["invalid_options"]);
}

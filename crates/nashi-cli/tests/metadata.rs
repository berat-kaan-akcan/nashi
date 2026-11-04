//! Reading and editing metadata. Skipped (not failed) when an external tool is
//! missing.

mod common;
use common::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fields(p: &Path) -> Value {
    let r = run(&["metadata", "--json", s(p)]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    serde_json::from_str(&r.stdout).unwrap()
}

fn field(j: &Value, key: &str) -> Option<Value> {
    j["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == key)
        .cloned()
}

fn exif_tag(p: &Path, tag: &str) -> String {
    let out = Command::new("exiftool")
        .args(["-b", &format!("-{tag}")])
        .arg(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

fn format_tag(p: &Path, tag: &str) -> Option<String> {
    ffprobe_json(p)["format"]["tags"][tag]
        .as_str()
        .map(String::from)
}

fn tagged_mp3(dir: &Path) -> PathBuf {
    let p = dir.join("song.mp3");
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
            "-metadata",
            "title=Old",
            "-metadata",
            "artist=Someone",
            s(&p),
        ],
    );
    p
}

#[test]
fn image_fields_are_listed_and_only_real_tags_can_be_edited() {
    need!("exiftool", "magick");
    let d = tempfile::tempdir().unwrap();
    let j = jpg(d.path(), "photo.jpg");
    tool(
        "exiftool",
        &["-q", "-overwrite_original", "-XMP-dc:Title=My Title", s(&j)],
    );
    let list = fields(&j);
    let title = field(&list, "XMP-dc:Title").expect("title listed");
    assert_eq!(title["value"], "My Title");
    assert_eq!(title["editable"], true);
    // Facts about the file itself are not metadata that travels with it.
    assert!(field(&list, "System:FileSize").is_none());
    assert!(field(&list, "File:FileType").is_none());
}

#[test]
fn image_edit_writes_a_new_file_and_leaves_the_input_alone() {
    need!("exiftool", "magick");
    let d = tempfile::tempdir().unwrap();
    let j = jpg(d.path(), "photo.jpg");
    tool(
        "exiftool",
        &[
            "-q",
            "-overwrite_original",
            "-XMP-dc:Title=Old",
            "-IFD0:Artist=Someone",
            s(&j),
        ],
    );
    let before = hash(&j);
    let r = tool_run(
        "removeMetadata",
        &[&j],
        Some(r#"{"metadata_set":{"XMP-dc:Title":"-New\nLine"},"metadata_remove":["IFD0:Artist"]}"#),
    );
    r.ok();
    let out = r.one();
    assert_eq!(out, d.path().join("photo-metadata.jpg"));
    assert_eq!(exif_tag(&out, "XMP-dc:Title"), "-New\nLine");
    assert_eq!(exif_tag(&out, "IFD0:Artist"), "");
    assert_eq!(hash(&j), before, "input must be untouched");
    assert_no_temp(d.path());
}

#[test]
fn removing_everything_still_works_without_options() {
    need!("exiftool", "magick");
    let d = tempfile::tempdir().unwrap();
    let j = jpg(d.path(), "photo.jpg");
    tool(
        "exiftool",
        &["-q", "-overwrite_original", "-IFD0:Artist=Someone", s(&j)],
    );
    let r = tool_run("removeMetadata", &[&j], None);
    r.ok();
    assert_eq!(exif_tag(&r.one(), "IFD0:Artist"), "");
    assert!(r.one().ends_with("photo-clean.jpg"), "{:?}", r.one());
}

#[test]
fn unwritable_and_hostile_keys_are_refused() {
    need!("exiftool", "magick");
    let d = tempfile::tempdir().unwrap();
    let j = jpg(d.path(), "photo.jpg");
    for options in [
        r#"{"metadata_set":{"File:FileSize":"1"}}"#,
        r#"{"metadata_set":{"Bogus:NotATag":"1"}}"#,
        r#"{"metadata_set":{"-o":"/tmp/evil"}}"#,
        r#"{"metadata_set":{"Comment<":"/etc/passwd"}}"#,
        r#"{"metadata_remove":["--"]}"#,
        r#"{"metadata_set":{},"metadata_remove":[]}"#,
    ] {
        let r = tool_run("removeMetadata", &[&j], Some(options));
        assert_eq!(r.error_codes(), ["invalid_options"], "{options}");
        assert_eq!(r.outputs().len(), 0, "{options}");
        assert_no_temp(d.path());
    }
    assert_eq!(walk(d.path()).len(), 1, "{:?}", walk(d.path()));
}

#[test]
fn pdf_edit_leaves_the_old_value_unrecoverable() {
    need!("exiftool", "qpdf");
    let d = tempfile::tempdir().unwrap();
    let p = pdf(d.path(), "doc.pdf", 1);
    let list = fields(&p);
    assert_eq!(
        field(&list, "PDF:Title").expect("title")["value"],
        "Secret Title"
    );
    let r = tool_run(
        "removeMetadata",
        &[&p],
        Some(r#"{"metadata_set":{"PDF:Title":"Public"}}"#),
    );
    r.ok();
    assert_eq!(exif_tag(&r.one(), "PDF:Title"), "Public");
    let raw = std::fs::read(r.one()).unwrap();
    assert!(
        !raw.windows(12).any(|w| w == b"Secret Title"),
        "old title in the file"
    );
}

#[test]
fn audio_tags_are_listed_and_edited_without_re_encoding() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let mp3 = tagged_mp3(d.path());
    let list = fields(&mp3);
    assert_eq!(field(&list, "title").expect("title")["value"], "Old");
    let r = tool_run(
        "removeMetadata",
        &[&mp3],
        Some(r#"{"metadata_set":{"title":"New"},"metadata_remove":["artist"]}"#),
    );
    r.ok();
    assert_eq!(format_tag(&r.one(), "title").as_deref(), Some("New"));
    assert_eq!(format_tag(&r.one(), "artist"), None);
    let bitrate = |p: &Path| ffprobe_json(p)["streams"][0]["bit_rate"].clone();
    assert_eq!(bitrate(&r.one()), bitrate(&mp3));
}

#[test]
fn video_keeps_its_streams_and_takes_custom_tags() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "clip.mp4", 1.0, "64x64");
    let r = tool_run(
        "removeMetadata",
        &[&v],
        Some(r#"{"metadata_set":{"title":"Clip","com.apple.quicktime.make":"Pear"}}"#),
    );
    r.ok();
    assert_eq!(format_tag(&r.one(), "title").as_deref(), Some("Clip"));
    assert_eq!(
        format_tag(&r.one(), "com.apple.quicktime.make").as_deref(),
        Some("Pear")
    );
    assert_eq!(
        ffprobe_json(&r.one())["streams"].as_array().unwrap().len(),
        ffprobe_json(&v)["streams"].as_array().unwrap().len()
    );
}

#[test]
fn a_missing_tool_is_reported_and_media_does_not_need_it() {
    need!("ffmpeg", "ffprobe", "magick");
    let d = tempfile::tempdir().unwrap();
    let j = jpg(d.path(), "photo.jpg");
    let r = run_env(
        &["metadata", "--json", s(&j)],
        &[("NASHI_DISABLE_TOOLS", "exiftool")],
    );
    assert_eq!(r.code, 3, "{}", r.stdout);
    let e: Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(e["error"]["code"], "missing_dependency");
    let edit = tool_run_env(
        "removeMetadata",
        &[&j],
        Some(r#"{"metadata_set":{"XMP-dc:Title":"x"}}"#),
        &[("NASHI_DISABLE_TOOLS", "exiftool")],
    );
    assert_eq!(edit.error_codes(), ["missing_dependency"]);
    let mp3 = tagged_mp3(d.path());
    let r = run_env(
        &["metadata", "--json", s(&mp3)],
        &[("NASHI_DISABLE_TOOLS", "exiftool")],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
}

#[test]
fn a_file_without_metadata_support_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let t = text(d.path(), "note.txt", "hello");
    let r = run(&["metadata", "--json", s(&t)]);
    assert_eq!(r.code, 1, "{}", r.stdout);
}

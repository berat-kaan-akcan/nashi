//! Split Video and Video Snapshots at chosen times. Skipped (not failed) when
//! an external tool is missing.

mod common;
use common::*;
use std::path::{Path, PathBuf};

fn seconds(p: &Path) -> f64 {
    ffprobe_json(p)["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() < 0.25,
        "{what}: {got} s, wanted {want} s"
    );
}

fn clip(dir: &Path) -> PathBuf {
    mp4(dir, "clip.mp4", 5.0, "64x64")
}

#[test]
fn split_points_give_one_clip_per_section() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    let before = hash(&v);
    // Points may arrive in any order.
    let r = tool_run("splitVideo", &[&v], Some(r#"{"split_points":[3,1]}"#));
    r.ok();
    let folder = r.one();
    assert_eq!(folder, d.path().join("clip-parts"));
    assert_eq!(
        names(&folder),
        ["clip-001.mp4", "clip-002.mp4", "clip-003.mp4"]
    );
    near(seconds(&folder.join("clip-001.mp4")), 1.0, "first");
    near(seconds(&folder.join("clip-002.mp4")), 2.0, "second");
    near(seconds(&folder.join("clip-003.mp4")), 2.0, "third");
    assert_eq!(hash(&v), before, "input must be untouched");
    assert_no_temp(d.path());
}

#[test]
fn equal_clips_divide_the_video_evenly() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    let r = tool_run("splitVideo", &[&v], Some(r#"{"equal_clips":2}"#));
    r.ok();
    assert_eq!(names(&r.one()).len(), 2);
    near(seconds(&r.one().join("clip-001.mp4")), 2.5, "first half");
    near(seconds(&r.one().join("clip-002.mp4")), 2.5, "second half");
}

#[test]
fn removed_clips_are_left_out_and_the_rest_keep_their_numbers() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    let r = tool_run(
        "splitVideo",
        &[&v],
        Some(r#"{"split_points":[1,3],"discard_clips":[1]}"#),
    );
    r.ok();
    assert_eq!(names(&r.one()), ["clip-001.mp4", "clip-003.mp4"]);
}

#[test]
fn kept_clips_can_be_joined_into_one_video() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    let r = tool_run(
        "splitVideo",
        &[&v],
        Some(r#"{"split_points":[1,3],"discard_clips":[1],"join_clips":true}"#),
    );
    r.ok();
    assert_eq!(r.one(), d.path().join("clip-split.mp4"));
    near(seconds(&r.one()), 3.0, "kept 1 s + 2 s");
    assert_no_temp(d.path());
}

#[test]
fn impossible_splits_are_refused_and_leave_nothing() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    for options in [
        r#"{"split_points":[9]}"#,
        r#"{"split_points":[0]}"#,
        r#"{"split_points":[2,2]}"#,
        r#"{"split_points":[2,2.01]}"#,
        r#"{"equal_clips":1}"#,
        r#"{"split_points":[2],"discard_clips":[0,1]}"#,
        r#"{"split_points":[2],"discard_clips":[5]}"#,
    ] {
        let r = tool_run("splitVideo", &[&v], Some(options));
        assert_eq!(r.error_codes(), ["invalid_options"], "{options}");
        assert_eq!(names(d.path()), ["clip.mp4"], "{options}");
    }
}

#[test]
fn plain_interval_split_still_works() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    let r = tool_run("splitVideo", &[&v], Some(r#"{"segment_seconds":2}"#));
    r.ok();
    assert!(names(&r.one()).len() >= 2);
}

#[test]
fn snapshots_save_the_chosen_times_as_png_files() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    let r = tool_run(
        "videoSnapshots",
        &[&v],
        Some(r#"{"snapshot_times":[1,3.5]}"#),
    );
    r.ok();
    assert_eq!(
        r.outputs(),
        [
            d.path().join("clip-frame-1.png"),
            d.path().join("clip-frame-2.png")
        ]
    );
    for out in r.outputs() {
        assert_eq!(&head(&out)[..4], b"\x89PNG", "{out:?}");
    }
    assert_no_temp(d.path());
}

#[test]
fn a_snapshot_at_the_very_end_saves_the_last_frame() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    let end = seconds(&v);
    let r = tool_run(
        "videoSnapshots",
        &[&v],
        Some(&format!(r#"{{"snapshot_times":[{end}]}}"#)),
    );
    r.ok();
    assert_eq!(&head(&r.one())[..4], b"\x89PNG");
}

#[test]
fn snapshot_times_outside_the_video_are_refused() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = clip(d.path());
    for options in [r#"{"snapshot_times":[99]}"#, r#"{"snapshot_times":[-1]}"#] {
        let r = tool_run("videoSnapshots", &[&v], Some(options));
        assert_eq!(r.error_codes(), ["invalid_options"], "{options}");
        assert_eq!(names(d.path()), ["clip.mp4"], "{options}");
    }
}

//! An archive may not expand past the safety limit. `NASHI_ARCHIVE_LIMIT_BYTES`
//! lowers it, so tests need no gigabytes. Skipped when a tool is missing.

mod common;
use common::*;
use std::path::{Path, PathBuf};

const LIMIT: &str = "100000";

/// A folder holding `count` files of 60 kB of zeros, which compress to almost nothing.
fn zeros(dir: &Path, count: usize) -> PathBuf {
    let folder = dir.join("payload");
    std::fs::create_dir_all(&folder).unwrap();
    for i in 0..count {
        std::fs::write(folder.join(format!("part{i}.bin")), vec![0u8; 60_000]).unwrap();
    }
    folder
}

fn extract(archive: &Path) -> Run {
    tool_run_env(
        "extractArchive",
        &[archive],
        None,
        &[("NASHI_ARCHIVE_LIMIT_BYTES", LIMIT)],
    )
}

fn assert_refused(r: &Run, dir: &Path) {
    assert_eq!(r.error_codes(), ["invalid_input"], "{:?}", r.events);
    assert_eq!(r.outputs().len(), 0);
    assert_no_temp(dir);
    assert!(
        walk(dir)
            .iter()
            .all(|p| !p.to_string_lossy().contains("-extracted")),
        "{:?}",
        walk(dir)
    );
}

#[test]
fn a_zip_past_the_limit_is_refused() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    let folder = zeros(d.path(), 2);
    let zip = d.path().join("bomb.zip");
    tool("7z", &["a", "-bso0", "-bsp0", s(&zip), s(&folder)]);
    let r = extract(&zip);
    assert_refused(&r, d.path());
}

#[test]
fn a_tar_gz_past_the_limit_is_refused() {
    need!("tar", "gzip");
    let d = tempfile::tempdir().unwrap();
    zeros(d.path(), 2);
    let archive = d.path().join("bomb.tar.gz");
    tool("tar", &["-czf", s(&archive), "-C", s(d.path()), "payload"]);
    let r = extract(&archive);
    assert_refused(&r, d.path());
}

#[test]
fn a_plain_gz_past_the_limit_is_refused() {
    need!("gzip");
    let d = tempfile::tempdir().unwrap();
    let big = d.path().join("big.bin");
    std::fs::write(&big, vec![0u8; 500_000]).unwrap();
    tool("gzip", &["-k", s(&big)]);
    std::fs::remove_file(&big).unwrap();
    let r = extract(&d.path().join("big.bin.gz"));
    assert_refused(&r, d.path());
}

#[test]
fn hard_links_count_as_full_copies() {
    need!("tar");
    let d = tempfile::tempdir().unwrap();
    let folder = zeros(d.path(), 1);
    for i in 0..2 {
        std::fs::hard_link(
            folder.join("part0.bin"),
            folder.join(format!("link{i}.bin")),
        )
        .unwrap();
    }
    let archive = d.path().join("links.tar");
    tool("tar", &["-cf", s(&archive), "-C", s(d.path()), "payload"]);
    let r = extract(&archive);
    assert_refused(&r, d.path());
}

#[test]
fn repacking_is_limited_too() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    let folder = zeros(d.path(), 2);
    let zip = d.path().join("bomb.zip");
    tool("7z", &["a", "-bso0", "-bsp0", s(&zip), s(&folder)]);
    let r = run_env(
        &["convert", "tar", s(&zip), "--json"],
        &[("NASHI_ARCHIVE_LIMIT_BYTES", LIMIT)],
    );
    assert_eq!(r.error_codes(), ["invalid_input"], "{:?}", r.events);
}

#[test]
fn an_archive_inside_the_limit_is_extracted() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    let folder = zeros(d.path(), 1);
    let zip = d.path().join("fine.zip");
    tool("7z", &["a", "-bso0", "-bsp0", s(&zip), s(&folder)]);
    let r = extract(&zip);
    r.ok();
    assert!(r.one().join("payload/part0.bin").exists());
}

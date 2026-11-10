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

#[test]
fn repacking_stores_links_instead_of_multiplying_their_target() {
    need!("7z", "tar");
    let d = tempfile::tempdir().unwrap();
    let folder = zeros(d.path(), 1);
    for i in 0..3 {
        std::os::unix::fs::symlink("part0.bin", folder.join(format!("link{i}.bin"))).unwrap();
    }
    let archive = d.path().join("links.tar");
    tool("tar", &["-cf", s(&archive), "-C", s(d.path()), "payload"]);
    // bsdtar already stores links, so force the 7z path.
    let r = run_env(
        &["convert", "zip", s(&archive), "--json"],
        &[("NASHI_DISABLE_TOOLS", "bsdtar")],
    );
    r.ok();
    let listing = std::process::Command::new("7z")
        .args(["l", "-slt", s(&r.one())])
        .output()
        .unwrap();
    let total: u64 = String::from_utf8_lossy(&listing.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("Size = ")?.trim().parse::<u64>().ok())
        .sum();
    assert!(total < 100_000, "links were followed: {total} bytes");
}

#[test]
fn a_password_protected_archive_says_so() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    let secret = d.path().join("secret.txt");
    std::fs::write(&secret, "hidden").unwrap();
    let zip_crypto = d.path().join("zipcrypto.zip");
    let aes = d.path().join("aes.zip");
    for (archive, method) in [(&zip_crypto, "-mem=ZipCrypto"), (&aes, "-mem=AES256")] {
        tool(
            "7z",
            &["a", "-bso0", "-pnashi", method, s(archive), s(&secret)],
        );
    }
    std::fs::remove_file(&secret).unwrap();
    // Each extractor in turn: 7z, then bsdtar, then unzip.
    let extractors = [("7z", ""), ("bsdtar", "7z"), ("unzip", "7z,bsdtar")];
    for archive in [&zip_crypto, &aes] {
        for (extractor, disabled) in extractors {
            if !which(extractor) {
                skipped(&format!("{extractor} on a protected zip"));
                continue;
            }
            // Info-ZIP's unzip cannot read AES at all, so it reports that instead.
            if extractor == "unzip" && archive == &aes {
                continue;
            }
            let r = tool_run_env(
                "extractArchive",
                &[archive],
                None,
                &[("NASHI_DISABLE_TOOLS", disabled)],
            );
            let errors = r.errors();
            assert_eq!(
                errors
                    .iter()
                    .map(|e| e["message"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                ["This archive is password protected."],
                "{extractor} {archive:?}: {:?}",
                r.events
            );
            assert_eq!(r.outputs().len(), 0);
            assert_no_temp(d.path());
        }
    }
}

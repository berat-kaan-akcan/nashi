//! Password-protected archives and PDFs: the password travels on stdin only,
//! never as an argument of nashi-cli or of the tool it runs. Skipped (not
//! failed) when a needed tool is missing.

mod common;
use common::*;
use std::path::{Path, PathBuf};

const PASSWORD: &str = "p@ss word $42!";
const PROTECTED: &str = "This archive is password protected.";
const WRONG: &str = "The password is incorrect, or the archive is damaged.";

/// `secret.txt` ("hidden") in a ZipCrypto and an AES-256 zip.
fn protected_zips(dir: &Path) -> Vec<PathBuf> {
    let secret = dir.join("secret.txt");
    std::fs::write(&secret, "hidden").unwrap();
    let mut out = Vec::new();
    for (name, method) in [
        ("zipcrypto.zip", "-mem=ZipCrypto"),
        ("aes.zip", "-mem=AES256"),
    ] {
        let archive = dir.join(name);
        let pw = format!("-p{PASSWORD}");
        tool("7z", &["a", "-bso0", &pw, method, s(&archive), s(&secret)]);
        out.push(archive);
    }
    std::fs::remove_file(secret).unwrap();
    out
}

fn extract(archive: &Path, password: Option<&str>, disabled: &str) -> Run {
    let mut args = vec!["tool", "extractArchive", s(archive), "--json"];
    if password.is_some() {
        args.push("--password-stdin");
    }
    let line = password.map(|p| format!("{p}\n"));
    run_full(
        &args,
        &[("NASHI_DISABLE_TOOLS", disabled)],
        line.as_deref().map(str::as_bytes),
    )
}

fn messages(r: &Run) -> Vec<String> {
    r.errors()
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_string())
        .collect()
}

/// Each extractor that can take a password on stdin: 7z, then bsdtar.
fn extractors() -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    for (extractor, disabled) in [("7z", ""), ("bsdtar", "7z")] {
        if which(extractor) {
            out.push((extractor, disabled));
        } else {
            skipped(&format!("{extractor} with a password"));
        }
    }
    out
}

#[test]
fn the_right_password_opens_it() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    for archive in protected_zips(d.path()) {
        for (extractor, disabled) in extractors() {
            let r = extract(&archive, Some(PASSWORD), disabled);
            r.ok();
            let folder = r.one();
            assert_eq!(
                std::fs::read_to_string(folder.join("secret.txt")).unwrap(),
                "hidden",
                "{extractor} {archive:?}"
            );
            std::fs::remove_dir_all(folder).unwrap();
            assert_no_temp(d.path());
        }
    }
}

#[test]
fn a_wrong_or_missing_password_is_named_and_leaves_nothing() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    for archive in protected_zips(d.path()) {
        for (extractor, disabled) in extractors() {
            for (password, expected) in [(None, PROTECTED), (Some("wrong"), WRONG)] {
                let r = extract(&archive, password, disabled);
                assert_eq!(
                    r.error_codes(),
                    ["password_required"],
                    "{extractor} {archive:?}"
                );
                assert_eq!(messages(&r), [expected], "{extractor} {archive:?}");
                assert_eq!(r.outputs().len(), 0);
                assert_no_temp(d.path());
                assert!(
                    walk(d.path())
                        .iter()
                        .all(|p| !p.to_string_lossy().contains("-extracted")),
                    "{extractor}: {:?}",
                    walk(d.path())
                );
            }
        }
    }
}

#[test]
fn an_empty_password_line_means_no_password() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    let archive = &protected_zips(d.path())[0];
    let r = extract(archive, Some(""), "");
    assert_eq!(messages(&r), [PROTECTED]);
}

#[test]
fn the_options_json_cannot_carry_a_password() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    let archive = &protected_zips(d.path())[0];
    // --options is an argument, which every user can read in /proc.
    let options = serde_json::json!({ "password": PASSWORD }).to_string();
    let r = tool_run("extractArchive", &[archive], Some(&options));
    assert_eq!(messages(&r), [PROTECTED]);
}

#[test]
fn converting_a_protected_zip_takes_the_password_too() {
    need!("7z");
    let d = tempfile::tempdir().unwrap();
    let archive = &protected_zips(d.path())[1];
    let r = run_full(
        &["convert", "tar", s(archive), "--json", "--password-stdin"],
        &[],
        Some(format!("{PASSWORD}\n").as_bytes()),
    );
    r.ok();
    assert!(r.one().extension().is_some_and(|e| e == "tar"));
}

#[test]
fn the_password_reaches_the_extractor_on_stdin_not_in_its_arguments() {
    let d = tempfile::tempdir().unwrap();
    let bin = d.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let log = d.path().join("log");
    let fake = bin.join("7z");
    // Records its arguments and stdin, then answers like 7z with a wrong password.
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\nprintf 'ARGS %s\\n' \"$*\" >> '{log}'\nprintf 'TTY %s\\n' \"$(tty 2>&1)\" >> '{log}'\nprintf 'STDIN ' >> '{log}'\ncat >> '{log}'\necho 'ERROR: Wrong password : secret.txt' >&2\nexit 2\n",
            log = log.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let archive = d.path().join("a.zip");
    // A real zip header, so the input is recognised; the fake 7z never reads it.
    std::fs::write(
        &archive,
        b"PK\x03\x04\x14\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    )
    .unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let r = run_full(
        &[
            "tool",
            "extractArchive",
            s(&archive),
            "--json",
            "--password-stdin",
        ],
        &[("PATH", &path)],
        Some(format!("{PASSWORD}\n").as_bytes()),
    );
    let log = std::fs::read_to_string(&log).unwrap();
    assert!(log.contains(&format!("STDIN {PASSWORD}\n")), "{log}");
    for line in log
        .lines()
        .filter(|l| l.starts_with("ARGS") || l.starts_with("TTY"))
    {
        assert!(!line.contains(PASSWORD), "{line}");
    }
    // No controlling terminal, so the extractor cannot prompt there instead.
    assert!(
        log.lines()
            .filter(|l| l.starts_with("TTY"))
            .all(|l| !l.contains("/dev/")),
        "{log}"
    );
    assert_eq!(messages(&r), [WRONG], "{:?}", r.events);
}

#[test]
fn unzip_alone_cannot_take_a_password_safely() {
    need!("7z", "unzip");
    let d = tempfile::tempdir().unwrap();
    let archive = &protected_zips(d.path())[0];
    // unzip only takes -P PASSWORD, an argument; ask for 7zip instead.
    let r = extract(archive, Some(PASSWORD), "7z,bsdtar");
    assert_eq!(r.error_codes(), ["missing_dependency"], "{:?}", r.events);
}

const PDF_PROTECTED: &str = "This PDF is password protected.";
const PDF_WRONG: &str = "The password is incorrect.";

/// A three-page text PDF that needs `PASSWORD` to open, and one with only an
/// owner password (no printing), which opens without one.
fn protected_pdfs(dir: &Path) -> (PathBuf, PathBuf) {
    let plain = pdf(dir, "plain.pdf", 3);
    let locked = dir.join("locked.pdf");
    let owner = dir.join("owner.pdf");
    let user = format!("--user-password={PASSWORD}");
    tool(
        "qpdf",
        &[
            "--encrypt",
            &user,
            "--owner-password=boss",
            "--bits=256",
            "--",
            s(&plain),
            s(&locked),
        ],
    );
    tool(
        "qpdf",
        &[
            "--encrypt",
            "--user-password=",
            "--owner-password=boss",
            "--bits=256",
            "--print=none",
            "--",
            s(&plain),
            s(&owner),
        ],
    );
    std::fs::remove_file(plain).unwrap();
    (locked, owner)
}

fn with_password(args: &[&str], password: &str, runtime: &Path) -> Run {
    let mut a = args.to_vec();
    a.extend(["--json", "--password-stdin"]);
    run_full(
        &a,
        &[("XDG_RUNTIME_DIR", s(runtime))],
        Some(format!("{password}\n").as_bytes()),
    )
}

fn text_of(pdf: &Path) -> String {
    let out = std::process::Command::new("pdftotext")
        .args([s(pdf), "-"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_protected_pdf_asks_for_its_password_instead_of_failing_or_writing_blanks() {
    need!("qpdf");
    let d = tempfile::tempdir().unwrap();
    let (locked, _) = protected_pdfs(d.path());
    // Compress used to write blank pages and split an empty folder, as if it worked.
    for args in [
        vec!["convert", "png", s(&locked), "--json"],
        vec!["tool", "compress", s(&locked), "--json"],
        vec!["tool", "splitPDF", s(&locked), "--json"],
        vec!["tool", "removeMetadata", s(&locked), "--json"],
    ] {
        let r = run(&args);
        assert_eq!(
            r.error_codes(),
            ["password_required"],
            "{args:?}: {:?}",
            r.events
        );
        assert_eq!(messages(&r), [PDF_PROTECTED], "{args:?}");
        assert_eq!(r.outputs().len(), 0);
    }
    let r = with_password(&["convert", "png", s(&locked)], "wrong", d.path());
    assert_eq!(r.error_codes(), ["password_required"]);
    assert_eq!(messages(&r), [PDF_WRONG]);
    // Nothing was written next to the inputs.
    assert_eq!(walk(d.path()).len(), 2, "{:?}", walk(d.path()));
}

#[test]
fn the_right_password_opens_a_pdf_for_every_tool() {
    need!("qpdf", "pdftotext");
    let d = tempfile::tempdir().unwrap();
    let runtime = tempfile::tempdir().unwrap();
    let (locked, _) = protected_pdfs(d.path());
    let r = with_password(&["tool", "splitPDF", s(&locked)], PASSWORD, runtime.path());
    r.ok();
    let parts = walk(&r.one());
    assert_eq!(parts.len(), 3, "{parts:?}");
    if have(&["gs"], "compressing a protected PDF") {
        let r = with_password(&["tool", "compress", s(&locked)], PASSWORD, runtime.path());
        r.ok();
        let out = r.one();
        // Named after the original, and the text is all there.
        assert_eq!(out.file_name().unwrap(), "locked-compressed.pdf");
        let text = text_of(&out);
        for page in 1..=3 {
            assert!(text.contains(&format!("Page {page} hello")), "{text}");
        }
    }
    let plain = pdf(d.path(), "other.pdf", 1);
    let r = with_password(
        &["tool", "mergePDF", s(&locked), s(&plain)],
        PASSWORD,
        runtime.path(),
    );
    r.ok();
    assert!(text_of(&r.one()).contains("Page 3 hello"));
    // The decrypted copies went to the private runtime folder and are gone.
    assert!(
        walk(runtime.path()).is_empty(),
        "{:?}",
        walk(runtime.path())
    );
    assert_no_temp(d.path());
}

#[test]
fn an_owner_password_alone_needs_no_password() {
    need!("qpdf", "pdftotext");
    let d = tempfile::tempdir().unwrap();
    let (_, owner) = protected_pdfs(d.path());
    let r = run(&["tool", "splitPDF", s(&owner), "--json"]);
    r.ok();
    assert_eq!(walk(&r.one()).len(), 3);
}

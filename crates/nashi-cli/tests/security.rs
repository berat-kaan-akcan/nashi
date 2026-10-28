//! Security regressions: orphaned children, symlink exfiltration, `%` patterns,
//! ImageMagick confinement, archive entry names, LibreOffice link fetching.

mod common;
use common::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

fn pgrep(pattern: &str) -> String {
    Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[test]
fn sigkilled_cli_leaves_no_running_children() {
    need!("ffmpeg", "ffprobe", "pgrep");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "orphan-check.mp4", 30.0, "1280x720");
    let mut child = Command::new(BIN)
        .args(["convert", "webm", s(&v), "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    use std::io::{BufRead, BufReader};
    let mut r = BufReader::new(child.stdout.take().unwrap());
    loop {
        let mut l = String::new();
        assert!(r.read_line(&mut l).unwrap() > 0);
        let e: Value = serde_json::from_str(l.trim()).unwrap();
        if e["event"] == "progress" && e["fraction"].as_f64().is_some_and(|f| f > 0.0) {
            break;
        }
    }
    assert!(!pgrep(s(&v)).is_empty(), "ffmpeg should be running");
    unsafe {
        kill(child.id() as i32, 9);
    }
    child.wait().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !pgrep(s(&v)).is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "ffmpeg survived the CLI's SIGKILL"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// A directory tree with a secret outside it and links pointing out and in.
fn linky_tree(root: &Path) -> (PathBuf, PathBuf) {
    let secret = root.join("secret.txt");
    std::fs::write(&secret, "TOP-SECRET").unwrap();
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("a.txt"), "alpha").unwrap();
    std::os::unix::fs::symlink(&secret, src.join("out-abs")).unwrap();
    std::os::unix::fs::symlink("../../secret.txt", src.join("out-rel")).unwrap();
    std::os::unix::fs::symlink("a.txt", src.join("in")).unwrap();
    (secret, src)
}

fn extract_with(env: &[(&str, &str)]) {
    let d = tempfile::tempdir().unwrap();
    let (_secret, src) = linky_tree(d.path());
    let zip = d.path().join("links.zip");
    tool(
        "bsdtar",
        &[
            "--format",
            "zip",
            "-cf",
            s(&zip),
            "-C",
            s(&src),
            "a.txt",
            "out-abs",
            "out-rel",
            "in",
        ],
    );
    let r = run_env(&["tool", "extractArchive", s(&zip), "--json"], env);
    r.ok();
    let out = r.one();
    // Policy: links that resolve outside the extracted folder are removed;
    // links inside it are kept.
    // (7z may instead store a refused link as a plain file holding the link
    // text, which is harmless.)
    for n in ["out-abs", "out-rel"] {
        if let Ok(m) = std::fs::symlink_metadata(out.join(n)) {
            assert!(!m.file_type().is_symlink(), "{n} kept as a symlink");
            assert!(!std::fs::read_to_string(out.join(n))
                .unwrap()
                .contains("TOP-SECRET"));
        }
    }
    assert_eq!(std::fs::read_to_string(out.join("in")).unwrap(), "alpha");
    // Repacking never follows outside links into the new archive.
    let r = run_env(&["convert", "tar", s(&zip), "--json"], env);
    r.ok();
    let data = std::fs::read(r.one()).unwrap();
    assert!(!data.windows(10).any(|w| w == b"TOP-SECRET"));
}

#[test]
fn extract_drops_outside_symlinks_7z() {
    need!("bsdtar", "7z");
    extract_with(&[]);
}

#[test]
fn extract_drops_outside_symlinks_bsdtar() {
    need!("bsdtar");
    extract_with(&[("NASHI_DISABLE_TOOLS", "7z")]);
}

#[test]
fn epub_chapter_symlink_is_not_read() {
    need!("bsdtar");
    let d = tempfile::tempdir().unwrap();
    let secret = d.path().join("secret.txt");
    std::fs::write(&secret, "TOP-SECRET").unwrap();
    let b = d.path().join("book");
    std::fs::create_dir_all(b.join("META-INF")).unwrap();
    std::fs::create_dir_all(b.join("OEBPS")).unwrap();
    std::fs::write(b.join("mimetype"), "application/epub+zip").unwrap();
    std::fs::write(
        b.join("META-INF/container.xml"),
        r#"<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
    )
    .unwrap();
    std::fs::write(
        b.join("OEBPS/content.opf"),
        r#"<package><manifest><item id="c1" href="c1.xhtml"/><item id="c2" href="c2.xhtml"/></manifest><spine><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#,
    )
    .unwrap();
    std::fs::write(
        b.join("OEBPS/c1.xhtml"),
        "<html><body><p>Visible chapter</p></body></html>",
    )
    .unwrap();
    std::os::unix::fs::symlink(&secret, b.join("OEBPS/c2.xhtml")).unwrap();
    let epub = d.path().join("evil.epub");
    tool(
        "bsdtar",
        &[
            "--format",
            "zip",
            "-cf",
            s(&epub),
            "-C",
            s(&b),
            "mimetype",
            "META-INF",
            "OEBPS",
        ],
    );
    for env in [
        vec![("NASHI_DISABLE_TOOLS", "pandoc")],
        vec![("NASHI_DISABLE_TOOLS", "pandoc,7z")],
    ] {
        let r = run_env(&["convert", "html", s(&epub), "--json"], &env);
        r.ok();
        let html = std::fs::read_to_string(r.one()).unwrap();
        assert!(html.contains("Visible chapter"));
        assert!(!html.contains("TOP-SECRET"), "symlinked chapter was read");
    }
}

#[test]
fn rtfd_zip_symlink_is_not_read() {
    need!("bsdtar", "soffice");
    let d = tempfile::tempdir().unwrap();
    let secret = d.path().join("secret.rtf");
    std::fs::write(&secret, r"{\rtf1\ansi TOP-SECRET\par}").unwrap();
    let b = d.path().join("Note.rtfd");
    std::fs::create_dir(&b).unwrap();
    std::os::unix::fs::symlink(&secret, b.join("TXT.rtf")).unwrap();
    // Directory bundle whose TXT.rtf is a link.
    let r = convert("txt", &[&b]);
    assert_eq!(r.error_codes(), ["invalid_input"]);
    // Zipped bundle carrying the link.
    let z = d.path().join("Zipped.rtfd");
    tool(
        "bsdtar",
        &[
            "--format",
            "zip",
            "-cf",
            s(&z),
            "-C",
            s(d.path()),
            "Note.rtfd",
        ],
    );
    let r = convert("txt", &[&z]);
    assert_eq!(r.error_codes(), ["invalid_input"]);
    assert!(walk(d.path())
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .all(|p| !std::fs::read_to_string(p)
            .unwrap_or_default()
            .contains("TOP-SECRET")));
}

#[test]
fn percent_in_output_directory() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out%d%s");
    let p = pdf(d.path(), "doc.pdf", 2);
    if which("gs") {
        let r = run(&["tool", "compress", s(&p), "--out-dir", s(&out), "--json"]);
        r.ok();
        assert_eq!(r.one(), out.join("doc-compressed.pdf"));
    }
    if which("pdfseparate") {
        let r = run_env(
            &["tool", "splitPDF", s(&p), "--out-dir", s(&out), "--json"],
            &[("NASHI_DISABLE_TOOLS", "qpdf")],
        );
        r.ok();
        assert!(r.one().join("doc-2.pdf").exists());
    }
    if which("gs") {
        let r = run_env(
            &["tool", "splitPDF", s(&p), "--out-dir", s(&out), "--json"],
            &[("NASHI_DISABLE_TOOLS", "qpdf,pdfseparate")],
        );
        r.ok();
    }
    if which("magick") {
        let i = png(d.path(), "pic.png");
        for t in ["jpg", "pdf"] {
            let r = run(&["convert", t, s(&i), "--out-dir", s(&out), "--json"]);
            r.ok();
            assert_eq!(r.one(), out.join(format!("pic.{t}")));
        }
    }
    // Nothing appeared outside the requested directory.
    let stray: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !["doc.pdf", "pic.png", "out%d%s"].contains(&n.as_str()))
        .collect();
    assert!(stray.is_empty(), "stray files: {stray:?}");
    assert_no_temp(d.path());
}

#[test]
fn imagemagick_does_not_sniff_scripts() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let secret = d.path().join("secret.txt");
    std::fs::write(&secret, "TOP-SECRET").unwrap();
    // MVG and SVG payloads saved with a .png name (no PNG magic → stays png).
    let mvg = text(
        d.path(),
        "evil.png",
        &format!(
            "push graphic-context\nviewbox 0 0 64 64\nimage over 0,0 64,64 'text:{}'\npop graphic-context\n",
            s(&secret)
        ),
    );
    let r = convert("jpg", &[&mvg]);
    assert_eq!(r.code, 1, "MVG disguised as PNG must not be rendered");
    assert_eq!(r.error_codes(), ["tool_failed"]);
    assert!(!d.path().join("evil.jpg").exists());
    // `@file` indirection and coder prefixes in names are inert.
    let at = png(d.path(), "@list.png");
    convert("jpg", &[&at]).ok();
    let colon = png(d.path(), "text:x.png");
    convert("jpg", &[&colon]).ok();
    assert_no_temp(d.path());
}

#[test]
fn imagemagick_policy_is_active() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let pol = d.path().join("pol");
    std::fs::create_dir(&pol).unwrap();
    std::fs::write(
        pol.join("policy.xml"),
        nashi_core::engines::image::MAGICK_POLICY,
    )
    .unwrap();
    let secret = text(d.path(), "s.txt", "TOP-SECRET");
    let out = Command::new("magick")
        .arg(format!("TEXT:{}", s(&secret)))
        .arg(d.path().join("o.png"))
        .env("MAGICK_CONFIGURE_PATH", &pol)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "TEXT coder must be blocked by the policy"
    );
    assert!(!d.path().join("o.png").exists());
}

#[test]
fn archive_entries_with_special_names() {
    if !which("7z") && !which("bsdtar") {
        eprintln!("skipped: 7z/bsdtar missing");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("src");
    std::fs::create_dir(&src).unwrap();
    for n in ["@list", "-rf", "normal.txt"] {
        std::fs::write(src.join(n), n).unwrap();
    }
    // `@list` would be read as a list file if passed bare.
    std::fs::write(d.path().join("list"), "/etc/hostname\n").unwrap();
    let tar = d.path().join("t.tar");
    tool(
        "tar",
        &[
            "-cf",
            s(&tar),
            "-C",
            s(&src),
            "--",
            "@list",
            "-rf",
            "normal.txt",
        ],
    );
    for env in [vec![], vec![("NASHI_DISABLE_TOOLS", "bsdtar")]] {
        if env.is_empty() && !which("bsdtar") || !env.is_empty() && !which("7z") {
            continue;
        }
        let r = run_env(&["convert", "zip", s(&tar), "--json"], &env);
        r.ok();
        let z = r.one();
        let x = run(&["tool", "extractArchive", s(&z), "--json"]);
        x.ok();
        let out = x.one();
        let mut names: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["-rf", "@list", "normal.txt"], "{env:?}");
        assert_eq!(std::fs::read_to_string(out.join("@list")).unwrap(), "@list");
    }
}

#[test]
fn animated_gif_keeps_frames_without_exiftool() {
    need!("magick", "ffmpeg");
    let d = tempfile::tempdir().unwrap();
    let g = d.path().join("anim.gif");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=32x32:rate=5:duration=1",
            s(&g),
        ],
    );
    let frames = |p: &Path| {
        let o = Command::new("magick")
            .args(["identify", "-format", "%n\n"])
            .arg(p)
            .env("LC_ALL", "C.UTF-8")
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .next()
            .unwrap_or("0")
            .trim()
            .parse::<u32>()
            .unwrap()
    };
    let n = frames(&g);
    assert!(n > 1);
    let r = tool_run_env(
        "removeMetadata",
        &[&g],
        None,
        &[("NASHI_DISABLE_TOOLS", "exiftool")],
    );
    r.ok();
    assert_eq!(frames(&r.one()), n);
}

#[test]
fn libreoffice_does_not_fetch_remote_images() {
    need!("soffice");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let d = tempfile::tempdir().unwrap();
    let md = text(
        d.path(),
        "remote.md",
        &format!("# Remote\n\n![x](http://127.0.0.1:{port}/a.png)\n"),
    );
    let html_like = text(
        d.path(),
        "remote.rtf",
        &format!(
            r#"{{\rtf1\ansi {{\field{{\*\fldinst INCLUDEPICTURE "http://127.0.0.1:{port}/b.png" \\d}}}} x\par}}"#
        ),
    );
    for f in [&md, &html_like] {
        let r = convert("pdf", &[f]);
        r.ok();
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        listener.accept().is_err(),
        "LibreOffice connected to a remote URL"
    );
}

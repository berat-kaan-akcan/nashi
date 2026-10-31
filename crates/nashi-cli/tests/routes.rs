//! Conversion routes. Each group is skipped (not failed) when its tool is missing.

mod common;
use common::*;
use std::path::Path;
use std::process::Command;

fn sniff_is(p: &Path, magic: &[u8], offset: usize) {
    let h = std::fs::read(p).unwrap();
    assert!(
        h.len() >= offset + magic.len() && &h[offset..offset + magic.len()] == magic,
        "{} does not look right: {:?}",
        p.display(),
        &h[..h.len().min(16)]
    );
}

#[test]
fn image_to_image_and_pdf() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = png(d.path(), "pic.png");
    let before = hash(&p);
    for (t, magic, off) in [
        ("jpg", &b"\xFF\xD8\xFF"[..], 0),
        ("webp", b"WEBP", 8),
        ("tiff", b"II*\0", 0),
        ("bmp", b"BM", 0),
        ("pdf", b"%PDF", 0),
    ] {
        let r = convert(t, &[&p]);
        r.ok();
        let out = r.one();
        assert_eq!(out, d.path().join(format!("pic.{t}")));
        sniff_is(&out, magic, off);
    }
    let j = jpg(d.path(), "photo.jpg");
    let r = convert("png", &[&j]);
    r.ok();
    sniff_is(&r.one(), b"\x89PNG", 0);
    assert_eq!(hash(&p), before);
    assert_no_temp(d.path());
}

#[test]
fn image_avif_both_ways() {
    need!("magick", "ffmpeg");
    let d = tempfile::tempdir().unwrap();
    let p = png(d.path(), "pic.png");
    let r = convert("avif", &[&p]);
    r.ok();
    let avif = r.one();
    sniff_is(&avif, b"ftyp", 4);
    let r = convert("jpg", &[&avif]);
    r.ok();
    sniff_is(&r.one(), b"\xFF\xD8\xFF", 0);
}

#[test]
fn image_heic_reports_missing_or_works() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let p = png(d.path(), "pic.png");
    let r = convert("heic", &[&p]);
    if r.code == 0 {
        sniff_is(&r.one(), b"ftyp", 4);
    } else {
        eprintln!("skipped: HEIC support (libheif) missing");
        assert_eq!(r.code, 3);
        assert_eq!(r.error_codes(), ["missing_dependency"]);
    }
    assert_no_temp(d.path());
}

#[test]
fn svg_routes() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let v = svg(d.path(), "logo.svg");
    for (t, magic) in [
        ("png", &b"\x89PNG"[..]),
        ("pdf", b"%PDF"),
        ("jpg", b"\xFF\xD8\xFF"),
    ] {
        let r = convert(t, &[&v]);
        r.ok();
        sniff_is(&r.one(), magic, 0);
    }
}

#[test]
fn audio_routes() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let w = wav(d.path(), "tone.wav", 1.0);
    for t in ["mp3", "m4a", "flac", "ogg", "opus", "aiff", "wma"] {
        let r = convert(t, &[&w]);
        r.ok();
        let out = r.one();
        let j = ffprobe_json(&out);
        assert_eq!(j["streams"][0]["codec_type"], "audio", "{t}");
        let dur: f64 = j["format"]["duration"].as_str().unwrap().parse().unwrap();
        assert!((dur - 1.0).abs() < 0.2, "{t} duration {dur}");
    }
    // And back from mp3.
    let r = convert("wav", &[&d.path().join("tone.mp3")]);
    assert_eq!(r.one(), d.path().join("tone (1).wav"));
    // Progress fractions were reported.
    assert!(r
        .events
        .iter()
        .any(|e| e["event"] == "progress" && e["fraction"].as_f64().is_some()));
}

#[test]
fn video_routes() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let v = mp4(d.path(), "clip.mp4", 1.0, "161x121"); // odd size on purpose
    for t in ["mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"] {
        let r = convert(t, &[&v]);
        r.ok();
        let out = r.one();
        let j = ffprobe_json(&out);
        assert!(
            j["streams"].as_array().is_some_and(|s| !s.is_empty()),
            "{t}"
        );
    }
    let r = convert("mp4", &[&d.path().join("clip.gif")]);
    r.ok();
    let j = ffprobe_json(&r.one());
    assert_eq!(j["streams"][0]["codec_name"], "h264");
}

#[test]
fn audio_target_from_silent_video_is_invalid_input() {
    need!("ffmpeg", "ffprobe");
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("silent.mp4");
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x64:duration=1",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            s(&p),
        ],
    );
    let r = convert("mp3", &[&p]);
    assert_eq!(r.error_codes(), ["invalid_input"]);
}

#[test]
fn pdf_routes() {
    need!("pdftoppm", "pdftotext");
    let d = tempfile::tempdir().unwrap();
    let one = pdf(d.path(), "one.pdf", 1);
    let three = pdf(d.path(), "three.pdf", 3);
    let r = convert("png", &[&one]);
    r.ok();
    assert_eq!(r.one(), d.path().join("one.png"));
    let r = convert("jpg", &[&three]);
    r.ok();
    let dir = r.one();
    assert_eq!(dir, d.path().join("three-pages"));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
    let r = convert("txt", &[&three]);
    r.ok();
    let t = std::fs::read_to_string(r.one()).unwrap();
    assert!(t.contains("Page 1 hello") && t.contains("Page 3 hello"));
    let r = convert("md", &[&three]);
    r.ok();
    assert!(std::fs::read_to_string(r.one()).unwrap().contains("---"));
    let r = convert("epub", &[&one]);
    r.ok();
    sniff_is(&r.one(), b"PK\x03\x04", 0);
    assert_no_temp(d.path());
}

#[test]
fn pdf_to_office() {
    need!("soffice");
    let d = tempfile::tempdir().unwrap();
    let p = pdf(d.path(), "doc.pdf", 1);
    for t in ["docx", "pptx"] {
        let r = convert(t, &[&p]);
        r.ok();
        sniff_is(&r.one(), b"PK\x03\x04", 0);
    }
    // Presentations: pptx -> pdf / png.
    let pptx = d.path().join("doc.pptx");
    let r = convert("pdf", &[&pptx]);
    r.ok();
    assert_eq!(r.one(), d.path().join("doc (1).pdf"));
    if which("pdftoppm") {
        let r = convert("png", &[&pptx]);
        r.ok();
        sniff_is(&r.one(), b"\x89PNG", 0);
    }
}

#[test]
fn text_and_markdown_routes() {
    need!("soffice");
    let d = tempfile::tempdir().unwrap();
    let t = text(d.path(), "notes.txt", "Hello text\n\nSecond paragraph\n");
    let m = text(d.path(), "readme.md", "# Title\n\nHello **world**\n");
    for target in ["docx", "pdf", "md", "html", "epub"] {
        let r = convert(target, &[&t]);
        r.ok();
        r.one();
    }
    assert_eq!(
        std::fs::read_to_string(d.path().join("notes.md")).unwrap(),
        "Hello text\n\nSecond paragraph\n"
    );
    for target in ["docx", "pdf", "txt", "html", "epub"] {
        let r = convert(target, &[&m]);
        r.ok();
        r.one();
    }
    let txt = std::fs::read_to_string(d.path().join("readme.txt")).unwrap();
    assert!(txt.contains("Hello world") && !txt.contains("**"));
    if which("pdftoppm") {
        let r = convert("png", &[&m]);
        r.ok();
        sniff_is(&r.one(), b"\x89PNG", 0);
    }
}

/// A crafted Markdown file must not pull other local files into the EPUB
/// (pandoc runs under `--sandbox` or bubblewrap, never unconfined).
#[test]
fn pandoc_does_not_embed_outside_files() {
    need!("pandoc", "magick", "unzip");
    let d = tempfile::tempdir().unwrap();
    let secret_dir = tempfile::tempdir().unwrap();
    let secret = png(secret_dir.path(), "secret.png");
    let m = text(
        d.path(),
        "evil.md",
        &format!("# Title\n\n![x]({})\n", secret.display()),
    );
    let r = convert("epub", &[&m]);
    if r.error_codes() == ["missing_dependency"] {
        eprintln!("skipped: pandoc unusable without bubblewrap");
        return;
    }
    r.ok();
    let listing = Command::new("unzip")
        .arg("-l")
        .arg(r.one())
        .output()
        .unwrap();
    let listing = String::from_utf8_lossy(&listing.stdout);
    assert!(
        !listing.contains("media/"),
        "outside file was embedded:\n{listing}"
    );
}

#[test]
fn docx_and_rtf_routes() {
    need!("soffice");
    let d = tempfile::tempdir().unwrap();
    let t = text(d.path(), "src.txt", "Body text\n");
    convert("docx", &[&t]).ok();
    let docx = d.path().join("src.docx");
    for target in ["pdf", "txt", "md", "html", "epub"] {
        let r = convert(target, &[&docx]);
        r.ok();
        r.one();
    }
    if which("pdftoppm") {
        let r = convert("jpg", &[&docx]);
        r.ok();
        sniff_is(&r.one(), b"\xFF\xD8\xFF", 0);
    }
    let rtf = text(d.path(), "r.rtf", r"{\rtf1\ansi Hello RTF\par}");
    let r = convert("docx", &[&rtf]);
    r.ok();
    let r = convert("txt", &[&rtf]);
    r.ok();
    assert!(std::fs::read_to_string(r.one())
        .unwrap()
        .contains("Hello RTF"));
    // RTFD bundle (directory with TXT.rtf).
    let bundle = d.path().join("Note.rtfd");
    std::fs::create_dir(&bundle).unwrap();
    std::fs::write(bundle.join("TXT.rtf"), r"{\rtf1\ansi Bundle text\par}").unwrap();
    let r = convert("txt", &[&bundle]);
    r.ok();
    assert_eq!(r.one(), d.path().join("Note.txt"));
    assert!(std::fs::read_to_string(r.one())
        .unwrap()
        .contains("Bundle text"));
}

#[test]
fn spreadsheet_routes() {
    need!("soffice");
    let d = tempfile::tempdir().unwrap();
    let c = text(d.path(), "t.csv", "a,b\n1,\"x,y\"\n");
    for target in ["xlsx", "ods", "tsv", "pdf"] {
        let r = convert(target, &[&c]);
        r.ok();
        r.one();
    }
    assert_eq!(
        std::fs::read_to_string(d.path().join("t.tsv")).unwrap(),
        "a\tb\n1\tx,y\n"
    );
    let r = convert("csv", &[&d.path().join("t.xlsx")]);
    r.ok();
    assert_eq!(
        std::fs::read_to_string(r.one()).unwrap(),
        "a,b\n1,\"x,y\"\n"
    );
    let r = convert("csv", &[&d.path().join("t.tsv")]);
    r.ok();
    let r = convert("xlsx", &[&d.path().join("t.ods")]);
    r.ok();
}

#[test]
fn epub_source_routes() {
    need!("soffice");
    if !which("7z") && !which("bsdtar") && !which("pandoc") {
        eprintln!("skipped: no epub reader (7z/bsdtar/pandoc)");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let t = text(d.path(), "book.txt", "Chapter one text\n\nMore text\n");
    convert("epub", &[&t]).ok();
    let e = d.path().join("book.epub");
    for target in ["html", "txt", "md", "docx", "pdf"] {
        let r = convert(target, &[&e]);
        r.ok();
        r.one();
    }
    let txt = std::fs::read_to_string(d.path().join("book (1).txt")).unwrap();
    assert!(txt.contains("Chapter one text"));
}

fn textclipping(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    // Minimal binary plist: {"UTI-Data": {"public.utf8-plain-text": <data>}}
    let mut d = b"bplist00".to_vec();
    let mut offs = Vec::new();
    offs.push(d.len());
    d.extend_from_slice(&[0xD1, 1, 2]);
    offs.push(d.len());
    d.push(0x58);
    d.extend_from_slice(b"UTI-Data");
    offs.push(d.len());
    d.extend_from_slice(&[0xD1, 3, 4]);
    offs.push(d.len());
    let k = b"public.utf8-plain-text";
    d.extend_from_slice(&[0x5F, 0x10, k.len() as u8]);
    d.extend_from_slice(k);
    offs.push(d.len());
    d.extend_from_slice(&[0x4F, 0x10, text.len() as u8]);
    d.extend_from_slice(text.as_bytes());
    let table = d.len();
    d.extend(offs.iter().map(|o| *o as u8));
    d.extend_from_slice(&[0, 0, 0, 0, 0, 0, 1, 1]);
    d.extend_from_slice(&(offs.len() as u64).to_be_bytes());
    d.extend_from_slice(&0u64.to_be_bytes());
    d.extend_from_slice(&(table as u64).to_be_bytes());
    let p = dir.join(name);
    std::fs::write(&p, d).unwrap();
    p
}

#[test]
fn textclipping_native_routes() {
    let d = tempfile::tempdir().unwrap();
    let c = textclipping(d.path(), "snip.textclipping", "Clipped words");
    for t in ["txt", "md", "epub"] {
        let r = convert(t, &[&c]);
        r.ok();
        r.one();
    }
    assert_eq!(
        std::fs::read_to_string(d.path().join("snip.txt")).unwrap(),
        "Clipped words"
    );
    if which("soffice") {
        let r = convert("docx", &[&c]);
        r.ok();
    } else {
        eprintln!("skipped: soffice missing (textclipping -> docx)");
    }
    // Garbage clipping -> invalid_input.
    let bad = text(d.path(), "bad.textclipping", "nothing here");
    assert_eq!(convert("txt", &[&bad]).error_codes(), ["invalid_input"]);
}

#[test]
fn subtitle_routes() {
    let d = tempfile::tempdir().unwrap();
    let srt = text(d.path(), "s.srt", "1\n00:00:01,000 --> 00:00:02,500\nHello <b>there</b>\n\n2\n00:00:03,000 --> 00:00:04,000\nBye\n");
    let r = convert("vtt", &[&srt]);
    r.ok();
    let vtt = r.one();
    let v = std::fs::read_to_string(&vtt).unwrap();
    assert!(v.starts_with("WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nHello <b>there</b>"));
    let r = convert("srt", &[&vtt]);
    assert_eq!(
        std::fs::read_to_string(r.one()).unwrap(),
        std::fs::read_to_string(&srt).unwrap() + "\n"
    );
    let r = convert("txt", &[&vtt]);
    assert_eq!(
        std::fs::read_to_string(r.one()).unwrap(),
        "Hello there\nBye\n"
    );
    let empty = text(d.path(), "e.srt", "nothing");
    assert_eq!(convert("vtt", &[&empty]).error_codes(), ["invalid_input"]);
}

fn make_tree(dir: &Path) -> std::path::PathBuf {
    let src = dir.join("tree");
    std::fs::create_dir_all(src.join("sub")).unwrap();
    std::fs::write(src.join("a.txt"), "alpha").unwrap();
    std::fs::write(src.join("sub/b.txt"), "beta").unwrap();
    src
}

fn archive_routes_with(env: &[(&str, &str)]) {
    let d = tempfile::tempdir().unwrap();
    let src = make_tree(d.path());
    let zip = d.path().join("pack.zip");
    if which("7z") {
        let st = std::process::Command::new("7z")
            .args(["a", "-tzip", "-bso0", s(&zip), "a.txt", "sub"])
            .current_dir(&src)
            .status()
            .unwrap();
        assert!(st.success());
    } else {
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
                "sub",
            ],
        );
    }
    let conv = |t: &str, p: &Path| {
        let r = run_env(&["convert", t, s(p), "--json"], env);
        r.ok();
        r.one()
    };
    let tar = conv("tar", &zip);
    assert_eq!(tar, d.path().join("pack.tar"));
    sniff_is(&tar, b"ustar", 257);
    let tgz = conv("gz", &zip);
    assert_eq!(tgz, d.path().join("pack.tar.gz"));
    sniff_is(&tgz, b"\x1F\x8B", 0);
    let z2 = conv("zip", &tgz);
    assert_eq!(z2, d.path().join("pack (1).zip"));
    let z3 = conv("zip", &tar);
    sniff_is(&z3, b"PK\x03\x04", 0);
    // Extract each and verify content.
    for a in [&zip, &tgz, &tar, &z2] {
        let r = run_env(&["tool", "extractArchive", s(a), "--json"], env);
        r.ok();
        let out = r.one();
        assert!(out
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("-extracted"));
        assert_eq!(
            std::fs::read_to_string(out.join("sub/b.txt")).unwrap(),
            "beta"
        );
    }
    // Single-file .gz (not a tarball).
    let gz = d.path().join("note.txt.gz");
    if which("gzip") {
        let f = std::fs::File::create(&gz).unwrap();
        let st = std::process::Command::new("gzip")
            .args(["-c", s(&src.join("a.txt"))])
            .stdout(f)
            .status()
            .unwrap();
        assert!(st.success());
        let r = run_env(&["tool", "extractArchive", s(&gz), "--json"], env);
        r.ok();
        assert_eq!(
            std::fs::read_to_string(r.one().join("note.txt")).unwrap(),
            "alpha"
        );
        let z = conv("zip", &gz);
        sniff_is(&z, b"PK\x03\x04", 0);
    }
    assert_no_temp(d.path());
}

#[test]
fn archive_routes_7z() {
    need!("7z");
    archive_routes_with(&[]);
}

#[test]
fn archive_routes_bsdtar_fallback() {
    need!("bsdtar");
    archive_routes_with(&[("NASHI_DISABLE_TOOLS", "7z")]);
}

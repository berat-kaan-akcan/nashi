//! Every route in the catalog: makes a sample of each source format, converts it to
//! each supported target, and checks that the output really is that format. New
//! routes are covered by themselves. A route is skipped (not failed) when a tool it
//! needs is missing; skips are printed, as is the source no tool here can create.

mod common;
use common::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for b in data {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A RAR4 archive with one stored file; no free tool writes RAR.
fn rar_bytes(name: &str, data: &[u8]) -> Vec<u8> {
    let block = |body: Vec<u8>| {
        let crc = (crc32(&body) & 0xFFFF) as u16;
        let mut out = crc.to_le_bytes().to_vec();
        out.extend(body);
        out
    };
    let mut archive = b"Rar!\x1a\x07\x00".to_vec();
    let mut main = vec![0x73, 0, 0];
    main.extend(13u16.to_le_bytes());
    main.extend([0; 6]);
    archive.extend(block(main));
    let mut file = vec![0x74, 0x00, 0x80];
    file.extend(((32 + name.len()) as u16).to_le_bytes());
    file.extend((data.len() as u32).to_le_bytes());
    file.extend((data.len() as u32).to_le_bytes());
    file.push(3);
    file.extend(crc32(data).to_le_bytes());
    file.extend(0x5A21_0000u32.to_le_bytes());
    file.extend([20, 0x30]);
    file.extend((name.len() as u16).to_le_bytes());
    file.extend(0o100644u32.to_le_bytes());
    file.extend(name.as_bytes());
    archive.extend(block(file));
    archive.extend(data);
    archive.extend(block(vec![0x7B, 0x00, 0x40, 7, 0]));
    archive
}

#[test]
fn the_hand_made_rar_is_a_real_rar() {
    if !which("unrar") {
        eprintln!("skipped: unrar");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.rar");
    std::fs::write(&p, rar_bytes("hello.txt", b"hello rar\n")).unwrap();
    let o = Command::new("unrar").args(["t"]).arg(&p).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let o = Command::new("unrar")
        .arg("x")
        .arg(&p)
        .current_dir(d.path())
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    assert_eq!(
        std::fs::read(d.path().join("hello.txt")).unwrap(),
        b"hello rar
"
    );
}

fn ran(program: &str, args: &[&str]) -> Result<(), String> {
    let o = Command::new(program)
        .args(args)
        .env("LC_ALL", "C.UTF-8")
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if o.status.success() {
        Ok(())
    } else {
        Err(format!("{program}: {}", String::from_utf8_lossy(&o.stderr)))
    }
}

fn needs(tools: &[&str]) -> Result<(), String> {
    match tools.iter().find(|t| !which(t)) {
        Some(t) => Err(format!("needs {t}")),
        None => Ok(()),
    }
}

/// Make a file with `soffice`, named by the source's stem and the filter's extension.
fn office(from: &Path, filter: &str, ext: &str, dir: &Path) -> Result<PathBuf, String> {
    needs(&["soffice"])?;
    let profile = format!(
        "-env:UserInstallation=file://{}/profile-{ext}",
        dir.display()
    );
    ran(
        "soffice",
        &[
            &profile,
            "--headless",
            "--convert-to",
            filter,
            "--outdir",
            s(dir),
            s(from),
        ],
    )?;
    let made = from.with_extension(ext);
    if made.exists() {
        Ok(made)
    } else {
        Err(format!("soffice made no .{ext}"))
    }
}

/// A sample of `ext`, or why there cannot be one on this machine.
fn sample(ext: &str, dir: &Path) -> Result<PathBuf, String> {
    let p = dir.join(format!("sample.{ext}"));
    let write = |text: &str| {
        std::fs::write(&p, text).map_err(|e| e.to_string())?;
        Ok(p.clone())
    };
    match ext {
        "jpg" | "png" | "webp" | "heic" | "tiff" | "avif" | "bmp" => {
            needs(&["magick"])?;
            ran("magick", &["-size", "64x48", "plasma:", s(&p)])?;
            Ok(p)
        }
        "svg" => write(r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48"><rect width="64" height="48" fill="#c00"/></svg>"##),
        "gif" => {
            needs(&["magick"])?;
            ran("magick", &["-size", "64x48", "xc:red", "xc:blue", s(&p)])?;
            Ok(p)
        }
        "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aiff" | "wma" => {
            needs(&["ffmpeg"])?;
            ran("ffmpeg", &["-v", "error", "-f", "lavfi", "-i", "sine=d=1:r=44100", s(&p)])?;
            Ok(p)
        }
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "wmv" => {
            needs(&["ffmpeg"])?;
            ran(
                "ffmpeg",
                &["-v", "error", "-f", "lavfi", "-i", "testsrc=d=1:s=64x48:r=10", "-f", "lavfi", "-i",
                  "sine=d=1:r=44100", "-shortest", s(&p)],
            )?;
            Ok(p)
        }
        "pdf" => Ok(pdf(dir, "sample.pdf", 2)),
        "txt" => write("Sample text\nSecond line\n"),
        "md" => write("# Title\n\nSample text\n"),
        "html" => write("<!doctype html><html><head><title>t</title></head><body><p>Sample text</p></body></html>"),
        "rtf" => write(r"{\rtf1\ansi Sample text\par}"),
        "csv" => write("a,b\n1,2\n"),
        "tsv" => write("a\tb\n1\t2\n"),
        "srt" => write("1\n00:00:01,000 --> 00:00:02,000\nHello\n\n"),
        "vtt" => write("WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n\n"),
        "rtfd" => {
            std::fs::create_dir(&p).map_err(|e| e.to_string())?;
            std::fs::write(p.join("TXT.rtf"), r"{\rtf1\ansi Sample text\par}").map_err(|e| e.to_string())?;
            Ok(p)
        }
        "textclipping" => Ok(textclipping(dir, "sample.textclipping", "Clipped words")),
        "epub" => {
            let t = sample("txt", dir)?;
            let r = run(&["convert", "--json", "epub", s(&t)]);
            r.outputs().first().cloned().ok_or_else(|| r.stdout.clone())
        }
        "docx" => office(&sample("txt", dir)?, "docx", "docx", dir),
        "pptx" => {
            needs(&["soffice"])?;
            let source = sample("pdf", dir)?;
            let r = run(&["convert", "--json", "pptx", s(&source)]);
            r.outputs().first().cloned().ok_or_else(|| r.stdout.clone())
        }
        "ppt" => office(&sample("pptx", dir)?, "ppt:MS PowerPoint 97", "ppt", dir),
        "xlsx" => office(&sample("csv", dir)?, "xlsx", "xlsx", dir),
        "xls" => office(&sample("csv", dir)?, "xls:MS Excel 97", "xls", dir),
        "ods" => office(&sample("csv", dir)?, "ods", "ods", dir),
        "zip" => {
            needs(&["7z"])?;
            let t = sample("txt", dir)?;
            ran("7z", &["a", "-bd", "-tzip", s(&p), s(&t)])?;
            Ok(p)
        }
        "tar" => {
            needs(&["tar"])?;
            sample("txt", dir)?;
            ran("tar", &["-cf", s(&p), "-C", s(dir), "sample.txt"])?;
            Ok(p)
        }
        "gz" => {
            needs(&["gzip"])?;
            let t = sample("txt", dir)?;
            let o = Command::new("gzip").args(["-c"]).arg(&t).output().map_err(|e| e.to_string())?;
            std::fs::write(&p, o.stdout).map_err(|e| e.to_string())?;
            Ok(p)
        }
        "rar" => {
            std::fs::write(&p, rar_bytes("hello.txt", b"hello rar\n")).map_err(|e| e.to_string())?;
            Ok(p)
        }
        // Only Apple's Pages writes this, and its contents are not documented.
        "pages" => Err("no tool here can create a .pages file".to_string()),
        other => Err(format!("no sample for .{other}")),
    }
}

/// Whether `p` looks like a file of format `target`.
fn looks_like(target: &str, p: &Path) -> Result<(), String> {
    let b = std::fs::read(p).map_err(|e| e.to_string())?;
    let at = |offset: usize, magic: &[u8]| {
        b.len() >= offset + magic.len() && &b[offset..offset + magic.len()] == magic
    };
    let zip_with =
        |entry: &str| at(0, b"PK\x03\x04") && b.windows(entry.len()).any(|w| w == entry.as_bytes());
    let ok = match target {
        "jpg" => at(0, b"\xFF\xD8\xFF"),
        "png" => at(0, b"\x89PNG"),
        "webp" => at(0, b"RIFF") && at(8, b"WEBP"),
        "heic" => at(4, b"ftyp") && (at(8, b"heic") || at(8, b"heix") || at(8, b"mif1")),
        "avif" => at(4, b"ftyp") && (at(8, b"avif") || at(8, b"mif1")),
        "tiff" => at(0, b"II*\0") || at(0, b"MM\0*"),
        "bmp" => at(0, b"BM"),
        "gif" => at(0, b"GIF8"),
        "pdf" => at(0, b"%PDF"),
        "mp3" => at(0, b"ID3") || (b.len() > 1 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0),
        "m4a" | "mp4" => at(4, b"ftyp"),
        "mov" => at(4, b"ftyp") || at(4, b"moov") || at(4, b"wide") || at(4, b"mdat"),
        "wav" => at(0, b"RIFF") && at(8, b"WAVE"),
        "avi" => at(0, b"RIFF") && at(8, b"AVI "),
        "flac" => at(0, b"fLaC"),
        "ogg" | "opus" => at(0, b"OggS"),
        "aiff" => at(0, b"FORM") && at(8, b"AIF"),
        "wma" | "wmv" => at(0, b"\x30\x26\xB2\x75"),
        "mkv" | "webm" => at(0, b"\x1A\x45\xDF\xA3"),
        "docx" => zip_with("word/"),
        "pptx" => zip_with("ppt/"),
        "xlsx" => zip_with("xl/"),
        "epub" => zip_with("mimetype"),
        "ods" => zip_with("mimetype"),
        "zip" => at(0, b"PK\x03\x04"),
        "gz" => at(0, b"\x1F\x8B"),
        "tar" => at(257, b"ustar"),
        "ppt" | "xls" => at(0, b"\xD0\xCF\x11\xE0"),
        "rtf" => at(0, b"{\\rtf"),
        "vtt" => at(0, b"WEBVTT") || at(3, b"WEBVTT"),
        "srt" => b.windows(3).any(|w| w == b"-->"),
        "html" => b.contains(&b'<'),
        _ => !b.is_empty(),
    };
    if ok {
        Ok(())
    } else {
        Err(format!("not {target}: {:?}", &b[..b.len().min(12)]))
    }
}

fn targets_of(ext: &str) -> Vec<String> {
    let r = run(&["catalog", "--json"]);
    let v: Value = serde_json::from_str(&r.stdout).unwrap();
    let f = v["formats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ext"] == ext);
    let f = f.unwrap_or_else(|| panic!("{ext} is not in the catalog"));
    let list = |key: &str| -> Vec<String> {
        f[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_string())
            .collect()
    };
    let unsupported = list("unsupported_targets");
    list("targets")
        .into_iter()
        .filter(|t| !unsupported.contains(t))
        .collect()
}

/// Run every route of `sources`, and fail with the list of those that went wrong.
fn check_routes(sources: &[&str]) {
    let mut failed = Vec::new();
    let mut done = 0;
    for ext in sources {
        let d = tempfile::tempdir().unwrap();
        let source = match sample(ext, d.path()) {
            Ok(p) => p,
            Err(why) if why.starts_with("needs ") || why.starts_with("no tool") => {
                eprintln!("skipped .{ext}: {why}");
                continue;
            }
            Err(why) => {
                failed.push(format!(".{ext} sample: {why}"));
                continue;
            }
        };
        for target in targets_of(ext) {
            let out = d.path().join(format!("out-{target}"));
            let r = run(&[
                "convert",
                "--json",
                &target,
                "--out-dir",
                s(&out),
                s(&source),
            ]);
            if r.error_codes().iter().any(|c| c == "missing_dependency") {
                eprintln!("skipped {ext} -> {target}: {:?}", r.errors());
                continue;
            }
            done += 1;
            let outputs = r.outputs();
            if r.code != 0 || outputs.is_empty() {
                failed.push(format!("{ext} -> {target}: {:?}", r.errors()));
                continue;
            }
            for made in outputs {
                let files = if made.is_dir() {
                    walk(&made)
                } else {
                    vec![made.clone()]
                };
                if files.is_empty() {
                    failed.push(format!("{ext} -> {target}: empty folder"));
                }
                for file in files.iter().filter(|f| f.is_file()) {
                    if let Err(why) = looks_like(&target, file) {
                        failed.push(format!("{ext} -> {target}: {why}"));
                    }
                }
            }
        }
    }
    eprintln!("{done} routes ran");
    assert!(
        failed.is_empty(),
        "{} failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

fn textclipping(dir: &Path, name: &str, text: &str) -> PathBuf {
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
fn every_catalog_format_has_a_route_test() {
    let r = run(&["catalog", "--json"]);
    let v: Value = serde_json::from_str(&r.stdout).unwrap();
    let covered: Vec<&str> = [
        IMAGES, AUDIO, VIDEO, DOCUMENTS, OFFICE, SHEETS, TEXTS, ARCHIVES,
    ]
    .concat();
    for f in v["formats"].as_array().unwrap() {
        let ext = f["ext"].as_str().unwrap();
        assert!(covered.contains(&ext), ".{ext} has no route test");
    }
}

const IMAGES: &[&str] = &[
    "jpg", "png", "webp", "heic", "tiff", "svg", "avif", "bmp", "gif",
];
const AUDIO: &[&str] = &["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"];
const VIDEO: &[&str] = &["mp4", "mov", "mkv", "webm", "avi", "wmv"];
const DOCUMENTS: &[&str] = &["pdf", "docx", "pages", "epub", "html"];
const OFFICE: &[&str] = &["ppt", "pptx"];
const SHEETS: &[&str] = &["xlsx", "xls", "ods", "csv", "tsv"];
const TEXTS: &[&str] = &["md", "rtf", "rtfd", "textclipping", "txt", "srt", "vtt"];
const ARCHIVES: &[&str] = &["zip", "tar", "gz", "rar"];

#[test]
fn image_routes() {
    check_routes(IMAGES);
}
#[test]
fn audio_routes() {
    check_routes(AUDIO);
}
#[test]
fn video_routes() {
    check_routes(VIDEO);
}
#[test]
fn document_routes() {
    check_routes(DOCUMENTS);
}
#[test]
fn presentation_routes() {
    check_routes(OFFICE);
}
#[test]
fn spreadsheet_routes() {
    check_routes(SHEETS);
}
#[test]
fn text_routes() {
    check_routes(TEXTS);
}
#[test]
fn archive_routes() {
    check_routes(ARCHIVES);
}

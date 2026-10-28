//! File type detection (extension + magic bytes) and `inspect`.

use serde::Serialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use crate::catalog::{category_of, Category};
use crate::deps::{probe_output, Deps};

/// Detected type of an input file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    /// Canonical catalog extension, or the lowercase raw extension when unknown.
    pub ext: String,
    pub category: Category,
    /// File name without the (possibly compound) extension.
    pub stem: String,
}

fn canonical(ext: &str) -> &str {
    match ext {
        "jpeg" | "jpe" | "jfif" => "jpg",
        "tif" => "tiff",
        "aif" | "aifc" => "aiff",
        "oga" => "ogg",
        "markdown" | "mdown" | "mkd" => "md",
        "text" => "txt",
        "tgz" => "gz",
        "htm" => "html",
        "qt" => "mov",
        other => other,
    }
}

/// Split a file name into `(stem, ext)` handling `.tar.gz` / `.tgz`.
pub fn split_name(name: &str) -> (String, String) {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") && name.len() > 7 {
        return (name[..name.len() - 7].to_string(), "gz".into());
    }
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => {
            let ext = canonical(&lower[i + 1..]).to_string();
            (name[..i].to_string(), ext)
        }
        _ => (name.to_string(), String::new()),
    }
}

/// Type determined from content. `None` = no opinion.
pub fn sniff_bytes(b: &[u8]) -> Option<&'static str> {
    let starts = |m: &[u8]| b.starts_with(m);
    let at = |off: usize, m: &[u8]| b.len() >= off + m.len() && &b[off..off + m.len()] == m;
    if starts(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if starts(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return Some("gif");
    }
    if starts(b"RIFF") {
        if at(8, b"WEBP") {
            return Some("webp");
        }
        if at(8, b"WAVE") {
            return Some("wav");
        }
        if at(8, b"AVI ") {
            return Some("avi");
        }
    }
    if starts(b"%PDF-") {
        return Some("pdf");
    }
    if starts(b"BM") && b.len() > 14 && at(6, &[0, 0, 0, 0]) {
        return Some("bmp");
    }
    if starts(b"II*\0") || starts(b"MM\0*") {
        return Some("tiff");
    }
    if starts(b"PK\x03\x04") {
        return Some("zip");
    }
    if starts(&[0x1F, 0x8B]) {
        return Some("gz");
    }
    if starts(b"Rar!\x1a\x07") {
        return Some("rar");
    }
    if at(257, b"ustar") {
        return Some("tar");
    }
    if at(4, b"ftyp") {
        let brand = &b[8..12.min(b.len())];
        return Some(match brand {
            b"heic" | b"heix" | b"hevc" | b"heim" | b"heis" | b"mif1" | b"msf1" => "heic",
            b"avif" | b"avis" => "avif",
            b"M4A " | b"M4B " => "m4a",
            b"qt  " => "mov",
            _ => "mp4",
        });
    }
    if starts(&[0x1A, 0x45, 0xDF, 0xA3]) {
        let head = &b[..b.len().min(64)];
        return Some(if head.windows(4).any(|w| w == b"webm") {
            "webm"
        } else {
            "mkv"
        });
    }
    if starts(b"OggS") {
        return Some(if b.windows(8).any(|w| w == b"OpusHead") {
            "opus"
        } else {
            "ogg"
        });
    }
    if starts(b"fLaC") {
        return Some("flac");
    }
    if starts(b"ID3") || (b.len() > 1 && b[0] == 0xFF && (b[1] & 0xE6) == 0xE2) {
        return Some("mp3");
    }
    if starts(b"FORM") && (at(8, b"AIFF") || at(8, b"AIFC")) {
        return Some("aiff");
    }
    if starts(&[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11]) {
        return Some("wmv");
    }
    if starts(b"{\\rtf") {
        return Some("rtf");
    }
    if starts(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return Some("ole");
    }
    if starts(b"bplist00") {
        return Some("bplist");
    }
    let text = String::from_utf8_lossy(&b[..b.len().min(512)]);
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if (t.starts_with("<?xml") || t.starts_with("<svg")) && text.contains("<svg") {
        return Some("svg");
    }
    if t.starts_with("WEBVTT") {
        return Some("vtt");
    }
    None
}

/// Whether a sniffed type is compatible with (does not contradict) an extension.
fn compatible(ext: &str, sniffed: &str) -> bool {
    if ext == sniffed {
        return true;
    }
    let group = |e: &str| -> u8 {
        match e {
            "mp4" | "mov" | "m4a" | "heic" | "avif" => 1,
            "mkv" | "webm" => 2,
            "ogg" | "opus" => 3,
            "wmv" | "wma" => 4,
            "zip" | "docx" | "xlsx" | "pptx" | "epub" | "ods" | "pages" | "rtfd" => 5,
            "ole" | "ppt" | "xls" => 6,
            "bplist" | "textclipping" => 7,
            "gz" | "tar" => 8,
            _ => 0,
        }
    };
    let g = group(ext);
    g != 0 && g == group(sniffed)
}

fn read_head(p: &Path) -> Vec<u8> {
    let mut buf = vec![0u8; 1024];
    match File::open(p).and_then(|mut f| f.read(&mut buf)) {
        Ok(n) => {
            buf.truncate(n);
            buf
        }
        Err(_) => Vec::new(),
    }
}

/// Detect the type of `path` (does not need to exist).
pub fn detect(path: &Path) -> Detected {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (stem, mut ext) = split_name(&name);
    if path.is_dir() {
        let category = if ext == "rtfd" {
            Category::Document
        } else {
            Category::Unknown
        };
        return Detected {
            ext,
            category,
            stem,
        };
    }
    let head = read_head(path);
    if let Some(s) = sniff_bytes(&head) {
        let known = category_of(&ext) != Category::Unknown;
        let s_known = category_of(s) != Category::Unknown;
        let textual = matches!(s, "svg" | "vtt");
        if s_known && (!known || (!textual && !compatible(&ext, s))) {
            ext = s.to_string();
        }
    }
    let category = category_of(&ext);
    Detected {
        ext,
        category,
        stem,
    }
}

/// `inspect` record.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Inspect {
    pub path: String,
    pub ext: String,
    pub category: Option<Category>,
    pub size: Option<u64>,
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pages: Option<u32>,
}

/// Media facts from ffprobe.
#[derive(Debug, Clone, Default)]
pub struct Probe {
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_video: bool,
    pub has_audio: bool,
    pub sample_rate: Option<u32>,
    /// `codec:width:height:pix_fmt:fps` of the first video stream and
    /// `codec:rate:channels` of the first audio stream (join compatibility).
    pub signature: String,
}

/// Run ffprobe on a file.
pub fn ffprobe(deps: &Deps, path: &Path) -> Option<Probe> {
    let ff = deps.path("ffprobe")?;
    let mut arg = std::ffi::OsString::from("file:");
    arg.push(path.as_os_str());
    let mut cmd = std::process::Command::new(ff);
    cmd.args([
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ])
    .arg(arg)
    .stdin(std::process::Stdio::null())
    .env("LC_ALL", "C.UTF-8");
    crate::process::die_with_parent(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let num = |x: &serde_json::Value| -> Option<f64> {
        x.as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| x.as_f64())
    };
    let mut p = Probe {
        duration: num(&v["format"]["duration"]),
        ..Default::default()
    };
    let mut vsig = String::from("-");
    let mut asig = String::from("-");
    for s in v["streams"].as_array().into_iter().flatten() {
        let attached = s["disposition"]["attached_pic"].as_i64() == Some(1);
        match s["codec_type"].as_str() {
            Some("video") if !attached && !p.has_video => {
                p.has_video = true;
                p.width = s["width"].as_u64().map(|x| x as u32);
                p.height = s["height"].as_u64().map(|x| x as u32);
                vsig = format!(
                    "{}:{}:{}:{}:{}",
                    s["codec_name"].as_str().unwrap_or("?"),
                    p.width.unwrap_or(0),
                    p.height.unwrap_or(0),
                    s["pix_fmt"].as_str().unwrap_or("?"),
                    s["r_frame_rate"].as_str().unwrap_or("?")
                );
                if p.duration.is_none() {
                    p.duration = num(&s["duration"]);
                }
            }
            Some("video") if attached && p.width.is_none() => {
                // Still image (e.g. jpg probed via ffprobe) — dimensions only.
            }
            Some("audio") if !p.has_audio => {
                p.has_audio = true;
                p.sample_rate = num(&s["sample_rate"]).map(|x| x as u32);
                asig = format!(
                    "{}:{}:{}",
                    s["codec_name"].as_str().unwrap_or("?"),
                    p.sample_rate.unwrap_or(0),
                    s["channels"].as_u64().unwrap_or(0)
                );
            }
            _ => {}
        }
    }
    p.signature = format!("{vsig}|{asig}");
    Some(p)
}

/// Page count of a PDF: qpdf, then pdfinfo, then gs (SAFER), then a naive scan.
/// The path is only ever passed as a plain argument, never as script source.
pub fn pdf_pages(deps: &Deps, path: &Path) -> Option<u32> {
    let p = path.to_string_lossy().into_owned();
    if let Some(q) = deps.path("qpdf") {
        if let Some(out) = probe_output(q, &["--show-npages", &p], Duration::from_secs(20)) {
            if let Some(n) = out.lines().find_map(|l| l.trim().parse().ok()) {
                return Some(n);
            }
        }
    }
    if let Some(pi) = deps.path("pdfinfo") {
        if let Some(out) = probe_output(pi, &[&p], Duration::from_secs(20)) {
            if let Some(n) = out
                .lines()
                .find_map(|l| l.strip_prefix("Pages:").and_then(|v| v.trim().parse().ok()))
            {
                return Some(n);
            }
        }
    }
    if let Some(gs) = deps.path("gs") {
        let file = format!("-sNashiFile={p}");
        let perm = format!("--permit-file-read={p}");
        let args = [
            "-q",
            "-dNODISPLAY",
            "-dSAFER",
            "-dBATCH",
            "-dNOPAUSE",
            &perm,
            &file,
            "-c",
            "NashiFile (r) file runpdfbegin pdfpagecount = quit",
        ];
        if let Some(out) = probe_output(gs, &args, Duration::from_secs(20)) {
            if let Some(n) = out.lines().find_map(|l| l.trim().parse().ok()) {
                return Some(n);
            }
        }
    }
    let data = std::fs::read(path).ok()?;
    let needle = b"/Type";
    let mut n = 0u32;
    let mut i = 0;
    while let Some(pos) = data[i..].windows(needle.len()).position(|w| w == needle) {
        let mut j = i + pos + needle.len();
        while j < data.len() && (data[j] == b' ' || data[j] == b'\n' || data[j] == b'\r') {
            j += 1;
        }
        if data[j..].starts_with(b"/Page") && !data[j..].starts_with(b"/Pages") {
            n += 1;
        }
        i = j;
    }
    (n > 0).then_some(n)
}

/// Inspect one file.
pub fn inspect(deps: &Deps, path: &Path) -> Inspect {
    let d = detect(path);
    let meta = std::fs::metadata(path).ok();
    let mut r = Inspect {
        path: path.to_string_lossy().into_owned(),
        ext: d.ext.clone(),
        category: Some(d.category),
        size: meta.as_ref().filter(|m| m.is_file()).map(|m| m.len()),
        ..Default::default()
    };
    if meta.is_none() {
        return r;
    }
    match d.category {
        Category::Audio | Category::Video | Category::Gif => {
            if let Some(p) = ffprobe(deps, path) {
                r.duration = p.duration;
                r.width = p.width;
                r.height = p.height;
            }
        }
        Category::Image => {
            if let Some(p) = ffprobe(deps, path).filter(|p| p.width.is_some()) {
                r.width = p.width;
                r.height = p.height;
            } else if let Some((w, h)) = crate::engines::image::identify_size(deps, path, &d.ext) {
                r.width = Some(w);
                r.height = Some(h);
            }
        }
        Category::Pdf => r.pages = pdf_pages(deps, path),
        _ => {}
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(split_name("a.TAR.GZ"), ("a".into(), "gz".into()));
        assert_eq!(split_name("Photo.JPEG"), ("Photo".into(), "jpg".into()));
        assert_eq!(split_name("x.tgz"), ("x".into(), "gz".into()));
        assert_eq!(split_name(".hidden"), (".hidden".into(), "".into()));
        assert_eq!(split_name("noext"), ("noext".into(), "".into()));
        assert_eq!(split_name("my.file.tif"), ("my.file".into(), "tiff".into()));
    }

    #[test]
    fn sniffing() {
        assert_eq!(sniff_bytes(b"\x89PNG\r\n\x1a\n...."), Some("png"));
        assert_eq!(sniff_bytes(b"%PDF-1.7"), Some("pdf"));
        assert_eq!(sniff_bytes(b"\0\0\0\x18ftypheic"), Some("heic"));
        assert_eq!(sniff_bytes(b"\0\0\0\x18ftypisom"), Some("mp4"));
        assert_eq!(sniff_bytes(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_bytes(b"<?xml version='1.0'?><svg>"), Some("svg"));
        assert_eq!(sniff_bytes(b"hello"), None);
    }

    #[test]
    fn ext_vs_magic() {
        let d = tempfile::tempdir().unwrap();
        // A PNG named .jpg is a PNG.
        let p = d.path().join("fake.jpg");
        std::fs::write(&p, b"\x89PNG\r\n\x1a\n0000000000").unwrap();
        assert_eq!(detect(&p).ext, "png");
        // A docx (zip) keeps its extension.
        let p = d.path().join("a.docx");
        std::fs::write(&p, b"PK\x03\x04rest").unwrap();
        assert_eq!(detect(&p).ext, "docx");
        assert_eq!(detect(&p).category, Category::Document);
        // A mov sniffed as mp4 stays mov.
        let p = d.path().join("clip.MOV");
        std::fs::write(&p, b"\0\0\0\x14ftypqt  ").unwrap();
        assert_eq!(detect(&p).ext, "mov");
        // Unknown extension falls back to content.
        let p = d.path().join("download.bin");
        std::fs::write(&p, b"%PDF-1.4\n").unwrap();
        assert_eq!(detect(&p).ext, "pdf");
        // Plain text formats have no magic.
        let p = d.path().join("notes.md");
        std::fs::write(&p, b"# hi").unwrap();
        assert_eq!(detect(&p).category, Category::Text);
        // Missing file: by extension.
        assert_eq!(
            detect(Path::new("/nonexistent/x.flac")).category,
            Category::Audio
        );
    }
}

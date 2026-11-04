//! URL downloading (`nashi-cli download`): direct HTTP/HTTPS files via `curl`
//! and video/audio/thumbnail extraction via `yt-dlp`.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::deps::{require, Deps};
use crate::detect::sniff_bytes;
use crate::events::{path_str, ErrorCode, Event, JobError, JobResult, Sink};
use crate::fsops::{files_in, Staged, WorkDir};
use crate::job::Summary;
use crate::process::{is_cancelled, Cmd};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadKind {
    Auto,
    Video,
    Audio,
    Thumbnail,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    /// Path component of the URL without query (`?...`) or fragment (`#...`).
    pub path: String,
}

/// Validate that `url` is a well-formed `http://` or `https://` URL with a
/// non-empty host and no control/whitespace characters.
pub fn validate_url(url: &str) -> JobResult<ParsedUrl> {
    if url.is_empty() {
        return Err(JobError::invalid_input("URL is empty"));
    }
    if url.bytes().any(|b| b <= 0x20 || b == 0x7f)
        || url.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(JobError::invalid_input(
            "URL must not contain whitespace or control characters",
        ));
    }
    let rest = if url.len() >= 7 && url[..7].eq_ignore_ascii_case("http://") {
        &url[7..]
    } else if url.len() >= 8 && url[..8].eq_ignore_ascii_case("https://") {
        &url[8..]
    } else {
        return Err(JobError::invalid_input(
            "only http:// and https:// URLs are supported",
        ));
    };
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..auth_end];
    let hostport = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority);
    let valid_host = if let Some(v6) = hostport.strip_prefix('[') {
        v6.split_once(']')
            .is_some_and(|(h, port)| !h.is_empty() && (port.is_empty() || port.starts_with(':')))
    } else {
        let host = hostport.split_once(':').map(|(h, _)| h).unwrap_or(hostport);
        !host.is_empty()
    };
    if !valid_host {
        return Err(JobError::invalid_input("URL has an empty or invalid host"));
    }
    let tail = &rest[auth_end..];
    let path = if tail.starts_with('/') {
        let end = tail.find(['?', '#']).unwrap_or(tail.len());
        tail[..end].to_string()
    } else {
        String::new()
    };
    Ok(ParsedUrl { path })
}

/// Whether `ext` (lowercase, no leading dot) is fetched directly under `--kind auto`.
pub fn is_direct_extension(ext: &str) -> bool {
    matches!(
        ext,
        "jpg"
            | "jpeg"
            | "jpe"
            | "jfif"
            | "png"
            | "webp"
            | "gif"
            | "svg"
            | "avif"
            | "heic"
            | "heif"
            | "bmp"
            | "tiff"
            | "tif"
            | "ico"
            | "pdf"
            | "docx"
            | "pptx"
            | "xlsx"
            | "xls"
            | "ppt"
            | "ods"
            | "odt"
            | "odp"
            | "rtf"
            | "epub"
            | "csv"
            | "tsv"
            | "txt"
            | "md"
            | "srt"
            | "vtt"
            | "mp4"
            | "m4v"
            | "webm"
            | "mkv"
            | "mov"
            | "avi"
            | "wmv"
            | "mp3"
            | "wav"
            | "flac"
            | "ogg"
            | "oga"
            | "opus"
            | "m4a"
            | "aac"
            | "aiff"
            | "aif"
            | "wma"
            | "zip"
            | "tar"
            | "gz"
            | "tar.gz"
            | "tgz"
            | "rar"
            | "7z"
    )
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (
                (b[i + 1] as char).to_digit(16),
                (b[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Extract `(stem, ext)` from a URL path's last segment, if any.
pub fn url_filename_and_ext(url_path: &str) -> (Option<String>, Option<String>) {
    let seg = url_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");
    if seg.is_empty() {
        return (None, None);
    }
    let decoded = percent_decode(seg);
    let lower = decoded.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") && decoded.len() > 7 {
        return (
            Some(decoded[..decoded.len() - 7].to_string()),
            Some("tar.gz".to_string()),
        );
    }
    if let Some((stem, ext)) = decoded.rsplit_once('.') {
        if !ext.is_empty() && ext.len() <= 16 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            let stem_opt = (!stem.is_empty()).then(|| stem.to_string());
            return (stem_opt, Some(ext.to_ascii_lowercase()));
        }
    }
    (Some(decoded), None)
}

/// Sanitise a downloaded `(stem, ext)`: strip control chars, replace `/` and
/// `\` with `_`, strip leading `.`/`-`, and truncate UTF-8 cleanly so
/// `stem.ext` is at most 150 bytes.
pub fn sanitize_filename(raw_stem: &str, raw_ext: &str) -> (String, String) {
    let mut ext: String = raw_ext
        .trim()
        .trim_matches('.')
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
        .collect::<String>()
        .to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 16 {
        ext = "bin".to_string();
    }

    let cleaned: String = raw_stem
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let trimmed = cleaned
        .trim_start_matches(|c: char| c == '.' || c == '-' || c.is_whitespace())
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace());

    let max_stem = 150usize.saturating_sub(ext.len() + 1).max(1);
    let mut stem = trimmed.to_string();
    if stem.len() > max_stem {
        let mut cut = max_stem;
        while cut > 0 && !stem.is_char_boundary(cut) {
            cut -= 1;
        }
        stem.truncate(cut);
    }
    let stem = stem
        .trim_start_matches(|c: char| c == '.' || c == '-' || c.is_whitespace())
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string();
    let stem = if stem.is_empty() {
        "download".to_string()
    } else {
        stem
    };
    (stem, ext)
}

fn split_raw_file(name: &str) -> (&str, &str) {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") && name.len() > 7 {
        return (&name[..name.len() - 7], "tar.gz");
    }
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => (&name[..i], &name[i + 1..]),
        _ => (name, "bin"),
    }
}

fn ext_from_content_type(ct: &str) -> Option<&'static str> {
    let mime = ct
        .lines()
        .last()
        .unwrap_or(ct)
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    Some(match mime.as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/avif" => "avif",
        "image/heic" | "image/heif" => "heic",
        "image/bmp" | "image/x-ms-bmp" => "bmp",
        "image/tiff" => "tiff",
        "image/x-icon" | "image/vnd.microsoft.icon" => "ico",
        "application/pdf" => "pdf",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/x-matroska" => "mkv",
        "video/quicktime" => "mov",
        "video/x-msvideo" => "avi",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/ogg" => "ogg",
        "audio/opus" => "opus",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "application/zip" => "zip",
        "application/gzip" | "application/x-gzip" => "gz",
        "application/x-tar" => "tar",
        "application/epub+zip" => "epub",
        "text/plain" => "txt",
        "text/markdown" => "md",
        "text/csv" => "csv",
        "text/vtt" => "vtt",
        _ => return None,
    })
}

/// Parse `XDG_DOWNLOAD_DIR="..."` from `user-dirs.dirs` content.
pub fn parse_xdg_download_dir(content: &str, home: Option<&Path>) -> Option<PathBuf> {
    for line in content.lines() {
        let l = line.trim();
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        let Some(rhs) = l.strip_prefix("XDG_DOWNLOAD_DIR=") else {
            continue;
        };
        let raw = rhs.trim();
        let unquoted = raw
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(raw);
        let expanded = if unquoted == "$HOME" {
            home?.to_path_buf()
        } else if let Some(rel) = unquoted.strip_prefix("$HOME/") {
            home?.join(rel)
        } else {
            let p = PathBuf::from(unquoted);
            if !p.is_absolute() {
                continue;
            }
            p
        };
        return Some(expanded);
    }
    None
}

/// Resolve the default XDG download directory.
pub fn xdg_download_dir() -> PathBuf {
    if let Some(v) = std::env::var_os("XDG_DOWNLOAD_DIR").filter(|v| !v.is_empty()) {
        let p = PathBuf::from(v);
        if p.is_absolute() {
            return p;
        }
    }
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let cfg_dir = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".config")));
    if let Some(cfg) = cfg_dir {
        if let Ok(text) = std::fs::read_to_string(cfg.join("user-dirs.dirs")) {
            if let Some(dir) = parse_xdg_download_dir(&text, home.as_deref()) {
                return dir;
            }
        }
    }
    if let Some(h) = home {
        return h.join("Downloads");
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Parse a `yt-dlp --newline` progress line (e.g. `[download]  42.3% of 10.0MiB`)
/// into a `0.0..=1.0` fraction.
pub fn parse_ytdlp_progress(line: &str) -> Option<f64> {
    let rest = line.trim_start().strip_prefix("[download]")?.trim_start();
    let pct_str = rest.split_once('%')?.0.trim();
    if pct_str.is_empty() || !pct_str.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let pct: f64 = pct_str.parse().ok()?;
    if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
        return None;
    }
    Some((pct / 100.0).clamp(0.0, 1.0))
}

/// Parse a `curl -#` stderr chunk (e.g. `#####       42.3%`) into `0.0..=1.0`.
pub fn parse_curl_progress(chunk: &str) -> Option<f64> {
    let without_pct = chunk.trim().strip_suffix('%')?;
    let token = without_pct.split_whitespace().next_back()?;
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let pct: f64 = token.parse().ok()?;
    if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
        return None;
    }
    Some((pct / 100.0).clamp(0.0, 1.0))
}

struct PctProgress<'a> {
    sink: &'a dyn Sink,
    url: &'a str,
    last_emit: Instant,
    last_fraction: f64,
}

impl<'a> PctProgress<'a> {
    fn new(sink: &'a dyn Sink, url: &'a str) -> Self {
        PctProgress {
            sink,
            url,
            last_emit: Instant::now() - Duration::from_secs(1),
            last_fraction: -1.0,
        }
    }

    fn emit_indeterminate(&self) {
        self.sink.emit(&Event::Progress {
            input: Some(self.url.to_string()),
            index: Some(0),
            fraction: None,
            message: "Downloading".to_string(),
        });
    }

    fn feed(&mut self, f: f64) {
        let f = (f.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;
        let done = (f - 1.0).abs() < 1e-6 && self.last_fraction < 1.0;
        if done
            || (f - self.last_fraction >= 0.01
                && self.last_emit.elapsed() >= Duration::from_millis(150))
        {
            self.last_fraction = f;
            self.last_emit = Instant::now();
            self.sink.emit(&Event::Progress {
                input: Some(self.url.to_string()),
                index: Some(0),
                fraction: Some(f),
                message: "Downloading".to_string(),
            });
        }
    }
}

fn job_id() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!(
        "{:04x}",
        (t ^ std::process::id().wrapping_mul(2_654_435_761)) & 0xffff
    )
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Run a download job, emitting the standard event stream (`start`, `progress`,
/// `output`, `error`, `done`) with `input = url` and `index = 0`.
pub fn run(
    url: &str,
    kind: DownloadKind,
    out_dir: Option<&Path>,
    deps: &Deps,
    sink: &dyn Sink,
) -> Summary {
    let mut sum = Summary {
        total: 1,
        ..Default::default()
    };
    sink.emit(&Event::Start {
        job: job_id(),
        total: 1,
    });

    let result = (|| -> JobResult<Vec<PathBuf>> {
        let parsed = validate_url(url)?;
        let dir = match out_dir {
            Some(d) => absolute(d),
            None => absolute(&xdg_download_dir()),
        };
        std::fs::create_dir_all(&dir)?;

        let effective = match kind {
            DownloadKind::Auto => {
                let (_, ext) = url_filename_and_ext(&parsed.path);
                if ext.as_deref().is_some_and(is_direct_extension) {
                    DownloadKind::Image
                } else {
                    DownloadKind::Video
                }
            }
            other => other,
        };

        match effective {
            DownloadKind::Image => download_curl(url, &parsed, &dir, deps, sink),
            DownloadKind::Video | DownloadKind::Audio | DownloadKind::Thumbnail => {
                download_ytdlp(url, effective, &dir, deps, sink)
            }
            DownloadKind::Auto => unreachable!(),
        }
    })();

    match result {
        Ok(paths) => {
            for p in &paths {
                sink.emit(&Event::Output {
                    input: Some(url.to_string()),
                    index: Some(0),
                    path: path_str(p),
                    message: None,
                });
            }
            sum.outputs = paths;
        }
        Err(e) if e.code == ErrorCode::Cancelled || is_cancelled() => {
            sum.cancelled = true;
        }
        Err(e) => {
            sink.emit(&Event::Error {
                input: Some(url.to_string()),
                index: Some(0),
                code: e.code,
                message: e.message,
            });
            sum.failed = 1;
            sum.error_codes.push(e.code);
        }
    }
    if is_cancelled() {
        sum.cancelled = true;
    }
    sink.emit(&Event::Done {
        outputs: sum.outputs.iter().map(|p| path_str(p)).collect(),
        failed: sum.failed,
        cancelled: sum.cancelled,
    });
    sum
}

fn download_curl(
    url: &str,
    parsed: &ParsedUrl,
    out_dir: &Path,
    deps: &Deps,
    sink: &dyn Sink,
) -> JobResult<Vec<PathBuf>> {
    let curl = require(deps, "curl")?;
    let work = WorkDir::new_in(out_dir)?;
    let body = work.join("body");

    let mut prog = PctProgress::new(sink, url);
    prog.emit_indeterminate();

    let out = Cmd::new(curl)
        .args([
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "-fL",
            "--max-redirs",
            "10",
            "--max-filesize",
            "2147483648",
            "-#",
            "-w",
            "%{content_type}\n",
            "-o",
        ])
        .arg(&body)
        .arg("--")
        .arg(url)
        .run_ok_lines_with_stderr(&mut |_| {}, &mut |chunk| {
            if let Some(f) = parse_curl_progress(chunk) {
                prog.feed(f);
            }
        })?;

    if !body.exists() {
        return Err(JobError::tool_failed("curl produced no output file"));
    }

    let (url_stem, url_ext) = url_filename_and_ext(&parsed.path);
    let ext = match url_ext {
        Some(e) => e,
        None => {
            if let Some(ct_ext) = ext_from_content_type(&out.stdout) {
                ct_ext.to_string()
            } else {
                let mut head = vec![0u8; 1024];
                let n = File::open(&body)
                    .and_then(|mut f| f.read(&mut head))
                    .unwrap_or(0);
                head.truncate(n);
                sniff_bytes(&head).unwrap_or("bin").to_string()
            }
        }
    };
    let (stem, ext) = sanitize_filename(url_stem.as_deref().unwrap_or("download"), &ext);
    let staged = Staged::file(out_dir, &stem, &ext)?;
    staged.take_from(&body)?;
    Ok(vec![staged.commit()?])
}

fn download_ytdlp(
    url: &str,
    kind: DownloadKind,
    out_dir: &Path,
    deps: &Deps,
    sink: &dyn Sink,
) -> JobResult<Vec<PathBuf>> {
    let ytdlp = require(deps, "yt-dlp")?;
    if kind == DownloadKind::Audio {
        require(deps, "ffmpeg")?;
    }
    let work = WorkDir::new_in(out_dir)?;

    let mut cmd = Cmd::new(ytdlp)
        .env("NO_COLOR", "1")
        .args([
            "--ignore-config",
            "--no-playlist",
            "--newline",
            "--color",
            "never",
            "--max-filesize",
            "2G",
            "-P",
        ])
        .arg(work.path())
        .args(["-o", "%(title).150B.%(ext)s"]);

    if let Some(ff) = deps.path("ffmpeg") {
        cmd = cmd.arg("--ffmpeg-location").arg(ff);
    }

    cmd = match kind {
        DownloadKind::Video | DownloadKind::Auto => {
            if deps.has("ffmpeg") {
                cmd.args([
                    "-f",
                    "bestvideo+bestaudio/best",
                    "--merge-output-format",
                    "mp4",
                ])
            } else {
                cmd.args(["-f", "best[ext=mp4]/best"])
            }
        }
        DownloadKind::Audio => cmd.args(["-x", "--audio-format", "mp3"]),
        DownloadKind::Thumbnail => {
            let c = cmd.args(["--write-thumbnail", "--skip-download"]);
            if deps.has("ffmpeg") {
                c.args(["--convert-thumbnails", "png"])
            } else {
                c
            }
        }
        DownloadKind::Image => unreachable!(),
    };
    cmd = cmd.arg("--").arg(url);

    let mut prog = PctProgress::new(sink, url);
    prog.emit_indeterminate();
    let prog_out = std::cell::RefCell::new(&mut prog);

    cmd.run_ok_lines_with_stderr(
        &mut |line| {
            if let Some(f) = parse_ytdlp_progress(line) {
                prog_out.borrow_mut().feed(f);
            }
        },
        &mut |chunk| {
            if let Some(f) = parse_ytdlp_progress(chunk) {
                prog_out.borrow_mut().feed(f);
            }
        },
    )?;

    let produced: Vec<PathBuf> = files_in(work.path())?
        .into_iter()
        .filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            !n.ends_with(".part") && !n.ends_with(".ytdl")
        })
        .collect();
    if produced.is_empty() {
        return Err(JobError::tool_failed("yt-dlp produced no output file"));
    }

    let mut committed = Vec::with_capacity(produced.len());
    for f in produced {
        let fname = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download.bin".to_string());
        let (raw_stem, raw_ext) = split_raw_file(&fname);
        let (stem, ext) = sanitize_filename(raw_stem, raw_ext);
        let staged = Staged::file(out_dir, &stem, &ext)?;
        staged.take_from(&f)?;
        committed.push(staged.commit()?);
    }
    Ok(committed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_validation() {
        assert_eq!(
            validate_url("https://example.com/a/b/cat.png?x=1#top").unwrap(),
            ParsedUrl {
                path: "/a/b/cat.png".into()
            }
        );
        assert_eq!(
            validate_url("HTTP://127.0.0.1:8080/test").unwrap(),
            ParsedUrl {
                path: "/test".into()
            }
        );
        assert!(validate_url("http://[::1]:8000/pic.jpg").is_ok());

        for bad in [
            "",
            "file:///etc/passwd",
            "ftp://example.com/a.zip",
            "javascript:alert(1)",
            "-o/tmp/pwn",
            "--config-location=/etc/passwd",
            "http://",
            "https:///etc/passwd",
            "http://:8080/a",
            "https://example.com/a b.png",
            "https://example.com/a\nb.png",
        ] {
            assert!(validate_url(bad).is_err(), "expected {bad:?} to fail");
        }
    }

    #[test]
    fn filename_sanitisation() {
        assert_eq!(
            sanitize_filename("-evil/path\\name", "PNG"),
            ("evil_path_name".into(), "png".into())
        );
        assert_eq!(
            sanitize_filename("...---...", "mp4"),
            ("download".into(), "mp4".into())
        );
        assert_eq!(
            sanitize_filename("hello\x00\x1bworld", ""),
            ("helloworld".into(), "bin".into())
        );
        // UTF-8 multi-byte truncation <= 150 bytes total.
        let long_tr = "Çok Güzel Şarkı ğüşiöç ".repeat(20);
        let (stem, ext) = sanitize_filename(&long_tr, "mp3");
        let full = format!("{stem}.{ext}");
        assert!(full.len() <= 150, "len {} > 150", full.len());
        assert!(!stem.starts_with(['.', '-']));
        assert!(!stem.contains('/'));
    }

    #[test]
    fn progress_parsers() {
        assert_eq!(
            parse_ytdlp_progress("[download]  42.5% of   10.00MiB at  2.00MiB/s ETA 00:03"),
            Some(0.425)
        );
        assert_eq!(
            parse_ytdlp_progress("[download] 100% of 4.20MiB in 00:01"),
            Some(1.0)
        );
        assert_eq!(
            parse_ytdlp_progress("[download] Destination: video.mp4"),
            None
        );
        assert_eq!(parse_ytdlp_progress("[info] Extracting URL"), None);

        assert_eq!(
            parse_curl_progress("######################                      42.3%"),
            Some(0.423)
        );
        assert_eq!(parse_curl_progress("100.0%"), Some(1.0));
        assert_eq!(parse_curl_progress("curl: (22) error"), None);
    }

    #[test]
    fn xdg_dir_parsing() {
        let cfg = "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOWNLOAD_DIR=\"$HOME/İndirilenler\"\n";
        assert_eq!(
            parse_xdg_download_dir(cfg, Some(Path::new("/home/berat"))),
            Some(PathBuf::from("/home/berat/İndirilenler"))
        );
        let abs = "XDG_DOWNLOAD_DIR=\"/mnt/data/dl\"\n";
        assert_eq!(
            parse_xdg_download_dir(abs, None),
            Some(PathBuf::from("/mnt/data/dl"))
        );
    }

    #[test]
    fn url_ext_and_auto_detection() {
        let (stem, ext) = url_filename_and_ext("/media/My%20Photo.JPEG");
        assert_eq!(stem.as_deref(), Some("My Photo"));
        assert_eq!(ext.as_deref(), Some("jpeg"));
        assert!(is_direct_extension("jpeg"));
        assert!(is_direct_extension("pdf"));
        assert!(!is_direct_extension("html"));
        assert!(!is_direct_extension("php"));
    }
}

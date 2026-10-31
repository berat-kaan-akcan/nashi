//! Discovery of external programs.
//!
//! Only `PATH` lookups and `--version` style probes; nothing is ever run
//! through a shell. Set `NASHI_DISABLE_TOOLS=ffmpeg,magick,...` to make tools
//! appear missing (used by tests and for troubleshooting).

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Static metadata for a protocol tool id.
pub struct DepDef {
    pub id: &'static str,
    pub package: &'static str,
    pub alt_packages: &'static [&'static str],
    pub optional: bool,
    pub used_for: &'static [&'static str],
    version_args: &'static [&'static str],
}

const fn d(
    id: &'static str,
    package: &'static str,
    optional: bool,
    used_for: &'static [&'static str],
    version_args: &'static [&'static str],
) -> DepDef {
    DepDef {
        id,
        package,
        alt_packages: &[],
        optional,
        used_for,
        version_args,
    }
}

/// Protocol tool ids, in protocol order, plus `libheif` (HEIC support).
pub const DEPS: &[DepDef] = &[
    d(
        "ffmpeg",
        "ffmpeg",
        false,
        &[
            "audio",
            "video",
            "gif",
            "avif",
            "compress",
            "removeMetadata",
            "muteVideo",
            "normalizeAudio",
            "joinVideos",
            "trimVideo",
            "trimAudio",
            "splitVideo",
            "videoSnapshots",
            "audioToVideo",
        ],
        &["-version"],
    ),
    d(
        "ffprobe",
        "ffmpeg",
        false,
        &["inspect", "progress", "joinVideos"],
        &["-version"],
    ),
    d(
        "magick",
        "imagemagick",
        false,
        &[
            "image",
            "createPDF",
            "createCollage",
            "compress",
            "cropImage",
            "editImage",
            "frameImage",
            "redactImage",
        ],
        &["-version"],
    ),
    DepDef {
        id: "soffice",
        package: "libreoffice-fresh",
        alt_packages: &["libreoffice-still"],
        optional: false,
        used_for: &["document", "presentation", "spreadsheet", "text"],
        version_args: &["--version"],
    },
    d(
        "exiftool",
        "perl-image-exiftool",
        false,
        &["removeMetadata"],
        &["-ver"],
    ),
    d(
        "qpdf",
        "qpdf",
        true,
        &["mergePDF", "splitPDF", "organizePDF", "removeMetadata"],
        &["--version"],
    ),
    d("pdftoppm", "poppler", false, &["pdf", "document"], &["-v"]),
    d("pdftotext", "poppler", false, &["pdf"], &["-v"]),
    d(
        "gs",
        "ghostscript",
        false,
        &["compress", "removeMetadata", "mergePDF", "splitPDF"],
        &["--version"],
    ),
    d("7z", "7zip", true, &["archive", "extractArchive"], &[]),
    d("zbarimg", "zbar", false, &["readQRCode"], &["--version"]),
    d("pandoc", "pandoc-cli", true, &["epub"], &["--version"]),
    d("rsvg-convert", "librsvg", true, &["svg"], &["--version"]),
    d("yt-dlp", "yt-dlp", true, &["download"], &["--version"]),
    d("libheif", "libheif", true, &["heic"], &[]),
];

/// Helper programs probed but not reported by `deps` (fallbacks).
const EXTRA: &[&str] = &[
    "bsdtar",
    "tar",
    "unzip",
    "gzip",
    "pdfunite",
    "pdfseparate",
    "pdfinfo",
    "heif-enc",
    "heif-dec",
    "montage",
    // Isolates pandoc when its own `--sandbox` is unusable (see engines/office.rs).
    "bwrap",
];

/// Result of probing the system.
#[derive(Debug, Clone, Default)]
pub struct Deps {
    paths: HashMap<String, PathBuf>,
    magick_formats: HashMap<String, (bool, bool)>,
}

fn disabled() -> HashSet<String> {
    std::env::var("NASHI_DISABLE_TOOLS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn is_executable(p: &Path) -> bool {
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Find `name` on `PATH`.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| is_executable(p))
}

/// Run a short probe command and capture stdout+stderr, with a timeout.
pub fn probe_output(prog: &Path, args: &[&str], timeout: Duration) -> Option<String> {
    let mut cmd = Command::new(prog);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("LC_ALL", "C.UTF-8");
    crate::process::die_with_parent(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    }
    let out = child.wait_with_output().ok()?;
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    Some(s)
}

/// Extract a version number from a `--version` banner.
pub fn parse_version(id: &str, text: &str) -> Option<String> {
    let line = text.lines().find(|l| !l.trim().is_empty())?;
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let start = match id {
        "magick" => tokens
            .iter()
            .position(|t| *t == "ImageMagick")
            .map(|i| i + 1),
        "7z" => tokens.iter().position(|t| *t == "7-Zip").map(|i| i + 1),
        _ => Some(0),
    }
    .unwrap_or(0);
    tokens[start.min(tokens.len())..].iter().find_map(|t| {
        let t = t.trim_start_matches(['n', 'v']);
        if t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            Some(t.trim_end_matches([',', ';']).to_string())
        } else {
            None
        }
    })
}

impl Deps {
    /// No tools at all (for tests).
    pub fn none() -> Deps {
        Deps::default()
    }

    /// Probe `PATH` for every known program.
    pub fn probe() -> Deps {
        let off = disabled();
        let mut paths = HashMap::new();
        let names = DEPS
            .iter()
            .map(|d| d.id)
            .filter(|id| *id != "libheif")
            .chain(EXTRA.iter().copied());
        for name in names {
            if off.contains(name) {
                continue;
            }
            if let Some(p) = which(name) {
                paths.insert(name.to_string(), p);
            }
        }
        // ImageMagick 6 fallback: `convert`, only when it really is ImageMagick.
        if !paths.contains_key("magick") && !off.contains("magick") {
            if let Some(p) = which("convert") {
                let banner =
                    probe_output(&p, &["-version"], Duration::from_secs(5)).unwrap_or_default();
                if banner.contains("ImageMagick") {
                    paths.insert("magick".into(), p);
                    paths.insert("magick6".into(), PathBuf::new());
                }
            }
        }
        let mut magick_formats = HashMap::new();
        if let Some(p) = paths.get("magick") {
            if let Some(list) = probe_output(p, &["-list", "format"], Duration::from_secs(10)) {
                magick_formats = parse_magick_formats(&list);
            }
        }
        let mut deps = Deps {
            paths,
            magick_formats,
        };
        let heif = (deps.magick_can("HEIC", true) && deps.magick_can("HEIC", false))
            || (deps.has("heif-enc") && deps.has("heif-dec"));
        if heif && !off.contains("libheif") {
            deps.paths.insert("libheif".into(), PathBuf::new());
        }
        deps
    }

    /// Process-wide probe result.
    pub fn global() -> &'static Deps {
        static G: OnceLock<Deps> = OnceLock::new();
        G.get_or_init(Deps::probe)
    }

    pub fn has(&self, id: &str) -> bool {
        self.paths.contains_key(id)
    }

    pub fn path(&self, id: &str) -> Option<&Path> {
        self.paths
            .get(id)
            .map(|p| p.as_path())
            .filter(|p| !p.as_os_str().is_empty())
    }

    /// Whether this is ImageMagick 6 (`convert`) rather than 7 (`magick`).
    pub fn is_magick6(&self) -> bool {
        self.has("magick6")
    }

    /// Whether ImageMagick can read (`write == false`) or write a format.
    pub fn magick_can(&self, format: &str, write: bool) -> bool {
        self.magick_formats
            .get(&format.to_ascii_uppercase())
            .map(|(r, w)| if write { *w } else { *r })
            .unwrap_or(false)
    }

    /// Mark a tool present (tests).
    pub fn with(mut self, id: &str, path: &str) -> Deps {
        self.paths.insert(id.into(), PathBuf::from(path));
        self
    }
}

fn parse_magick_formats(list: &str) -> HashMap<String, (bool, bool)> {
    let mut m = HashMap::new();
    for line in list.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 3 {
            continue;
        }
        let name = tokens[0].trim_end_matches('*');
        let mode = tokens[2];
        if mode.len() == 3 && mode.chars().all(|c| matches!(c, 'r' | 'w' | '+' | '-')) {
            let b = mode.as_bytes();
            m.insert(name.to_ascii_uppercase(), (b[0] == b'r', b[1] == b'w'));
        }
    }
    m
}

/// One entry of `deps --json`.
#[derive(Debug, Serialize)]
pub struct DepView {
    pub id: &'static str,
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub package: &'static str,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub alt_packages: &'static [&'static str],
    pub optional: bool,
    pub used_for: &'static [&'static str],
}

/// Full dependency report, including versions (slower: runs each tool).
pub fn report(deps: &Deps) -> Vec<DepView> {
    let handles: Vec<_> = DEPS
        .iter()
        .map(|def| {
            let path = deps.path(def.id).map(Path::to_path_buf);
            let available = deps.has(def.id);
            let args = def.version_args;
            let id = def.id;
            let h = std::thread::spawn(move || {
                let p = path.as_ref()?;
                let text = probe_output(p, args, Duration::from_secs(8))?;
                parse_version(id, &text)
            });
            (
                def,
                available,
                deps.path(def.id).map(|p| p.to_string_lossy().into_owned()),
                h,
            )
        })
        .collect();
    handles
        .into_iter()
        .map(|(def, available, path, h)| DepView {
            id: def.id,
            available,
            version: h.join().ok().flatten(),
            path,
            package: def.package,
            alt_packages: def.alt_packages,
            optional: def.optional,
            used_for: def.used_for,
        })
        .collect()
}

/// Convenience for engines: path of a required program or a `missing_dependency` error.
pub fn require<'a>(deps: &'a Deps, id: &str) -> Result<&'a Path, crate::JobError> {
    deps.path(id).ok_or_else(|| {
        let pkg = DEPS
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.package)
            .unwrap_or(id);
        crate::JobError::missing(id, pkg)
    })
}

/// Name used in messages for a program path.
pub fn prog_name(p: &Path) -> String {
    p.file_name()
        .unwrap_or(OsStr::new("?"))
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(
            parse_version("ffmpeg", "ffmpeg version n9.0.2 Copyright").as_deref(),
            Some("9.0.2")
        );
        assert_eq!(
            parse_version("magick", "Version: ImageMagick 7.1.2-34 Q16-HDRI x86_64").as_deref(),
            Some("7.1.2-34")
        );
        assert_eq!(
            parse_version("soffice", "LibreOffice 26.8.1.1 bf8adb").as_deref(),
            Some("26.8.1.1")
        );
        assert_eq!(parse_version("gs", "10.08.0\n").as_deref(), Some("10.08.0"));
        assert_eq!(
            parse_version("qpdf", "qpdf version 12.4.2").as_deref(),
            Some("12.4.2")
        );
        assert_eq!(
            parse_version("7z", "\n7-Zip 26.05 (x64) : Copyright").as_deref(),
            Some("26.05")
        );
        assert_eq!(
            parse_version("pdftoppm", "pdftoppm version 26.10.0").as_deref(),
            Some("26.10.0")
        );
    }

    #[test]
    fn magick_format_parse() {
        let m = parse_magick_formats(
            "     HEIC* HEIC      rw+   High Efficiency\n      PNG* PNG       rw-   Portable\n",
        );
        assert_eq!(m.get("HEIC"), Some(&(true, true)));
        assert_eq!(m.get("PNG"), Some(&(true, true)));
    }
}

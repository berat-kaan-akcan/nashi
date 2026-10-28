//! Feature catalog: formats, conversion targets, tools and file actions.
//!
//! The source of truth for *which* formats, targets and tools exist is the
//! upstream Nashi `FeatureCatalog.json` (MIT, see `NOTICE`), embedded at build
//! time. This module adds the Linux-specific metadata the upstream file does
//! not carry: categories, Turkish titles, freedesktop icon names, whether a tool
//! needs an editor, and which external programs each route/tool requires.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::deps::Deps;

const UPSTREAM_JSON: &str = include_str!("../../../reference/upstream-FeatureCatalog.json");

/// Parsed upstream catalog.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upstream {
    pub conversion_matrix: BTreeMap<String, Vec<String>>,
    pub single_tools: BTreeMap<String, Vec<String>>,
    pub multiple_tools: BTreeMap<String, Vec<String>>,
    pub tool_names: BTreeMap<String, String>,
    pub file_actions: Vec<String>,
}

/// The embedded upstream catalog.
pub fn upstream() -> &'static Upstream {
    static U: OnceLock<Upstream> = OnceLock::new();
    U.get_or_init(|| {
        serde_json::from_str(UPSTREAM_JSON).expect("embedded upstream catalog is valid")
    })
}

/// File category, as defined by `docs/CLI-PROTOCOL.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Image,
    Audio,
    Video,
    Gif,
    Pdf,
    Document,
    Presentation,
    Spreadsheet,
    Text,
    Subtitle,
    Archive,
    Unknown,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Image => "image",
            Category::Audio => "audio",
            Category::Video => "video",
            Category::Gif => "gif",
            Category::Pdf => "pdf",
            Category::Document => "document",
            Category::Presentation => "presentation",
            Category::Spreadsheet => "spreadsheet",
            Category::Text => "text",
            Category::Subtitle => "subtitle",
            Category::Archive => "archive",
            Category::Unknown => "unknown",
        }
    }
}

/// Every source format, in upstream order, with its category.
pub const FORMATS: &[(&str, Category)] = &[
    ("jpg", Category::Image),
    ("png", Category::Image),
    ("webp", Category::Image),
    ("heic", Category::Image),
    ("tiff", Category::Image),
    ("svg", Category::Image),
    ("avif", Category::Image),
    ("bmp", Category::Image),
    ("mp3", Category::Audio),
    ("m4a", Category::Audio),
    ("wav", Category::Audio),
    ("flac", Category::Audio),
    ("ogg", Category::Audio),
    ("opus", Category::Audio),
    ("aiff", Category::Audio),
    ("wma", Category::Audio),
    ("mp4", Category::Video),
    ("mov", Category::Video),
    ("mkv", Category::Video),
    ("webm", Category::Video),
    ("avi", Category::Video),
    ("wmv", Category::Video),
    ("gif", Category::Gif),
    ("pdf", Category::Pdf),
    ("ppt", Category::Presentation),
    ("pptx", Category::Presentation),
    ("docx", Category::Document),
    ("pages", Category::Document),
    ("md", Category::Text),
    ("rtf", Category::Document),
    ("rtfd", Category::Document),
    ("textclipping", Category::Text),
    ("epub", Category::Document),
    ("txt", Category::Text),
    ("xlsx", Category::Spreadsheet),
    ("xls", Category::Spreadsheet),
    ("ods", Category::Spreadsheet),
    ("csv", Category::Spreadsheet),
    ("tsv", Category::Spreadsheet),
    ("srt", Category::Subtitle),
    ("vtt", Category::Subtitle),
    ("zip", Category::Archive),
    ("tar", Category::Archive),
    ("gz", Category::Archive),
    ("rar", Category::Archive),
];

/// Category of a canonical extension (`Unknown` when not in the catalog).
pub fn category_of(ext: &str) -> Category {
    FORMATS
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, c)| *c)
        .unwrap_or(Category::Unknown)
}

/// Category of a *target* extension (targets include a few formats that are
/// not sources, e.g. `html`).
pub fn target_category(ext: &str) -> Category {
    match ext {
        "html" => Category::Document,
        other => category_of(other),
    }
}

/// Bilingual title.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Title {
    pub en: &'static str,
    pub tr: &'static str,
}

/// Static description of a tool.
#[derive(Debug, Clone, Copy)]
pub struct ToolDef {
    pub id: &'static str,
    pub title: Title,
    pub icon: &'static str,
    pub needs_editor: bool,
    pub multi_input: bool,
}

const fn t(
    id: &'static str,
    en: &'static str,
    tr: &'static str,
    icon: &'static str,
    needs_editor: bool,
    multi_input: bool,
) -> ToolDef {
    ToolDef {
        id,
        title: Title { en, tr },
        icon,
        needs_editor,
        multi_input,
    }
}

/// Every tool id from upstream `toolNames`. English titles equal the upstream
/// names (checked by a unit test).
pub const TOOLS: &[ToolDef] = &[
    t(
        "compress",
        "Compress",
        "Sıkıştır",
        "package-x-generic",
        false,
        false,
    ),
    t(
        "removeMetadata",
        "Metadata",
        "Meta Verileri Sil",
        "edit-clear-all",
        false,
        false,
    ),
    t(
        "editImage",
        "Edit Photo",
        "Fotoğrafı Düzenle",
        "document-edit",
        true,
        false,
    ),
    t(
        "annotateImage",
        "Annotate Photo",
        "Fotoğrafa Not Ekle",
        "draw-freehand",
        true,
        false,
    ),
    t(
        "frameImage",
        "Add BG",
        "Arka Plan Ekle",
        "draw-rectangle",
        true,
        false,
    ),
    t(
        "cropImage",
        "Crop Image",
        "Görseli Kırp",
        "transform-crop",
        true,
        false,
    ),
    t(
        "normalizeAudio",
        "Normalize Volume",
        "Ses Seviyesini Eşitle",
        "audio-volume-high",
        false,
        false,
    ),
    t(
        "muteVideo",
        "Remove Audio",
        "Sesi Kaldır",
        "audio-volume-muted",
        false,
        false,
    ),
    t(
        "trimVideo",
        "Trim Video",
        "Videoyu Kes",
        "edit-cut",
        true,
        false,
    ),
    t(
        "splitPDF",
        "Split PDF",
        "PDF'yi Böl",
        "view-split-left-right",
        false,
        false,
    ),
    t(
        "organizePDF",
        "Organize PDF",
        "PDF'yi Düzenle",
        "view-sort",
        true,
        false,
    ),
    t(
        "extractArchive",
        "Extract Archive",
        "Arşivi Çıkar",
        "archive-extract",
        false,
        false,
    ),
    t(
        "createPDF",
        "Create PDF",
        "PDF Oluştur",
        "application-pdf",
        false,
        true,
    ),
    t(
        "mergePDF",
        "Merge PDFs",
        "PDF'leri Birleştir",
        "merge",
        false,
        true,
    ),
    t(
        "cropVideo",
        "Crop Video",
        "Videoyu Kırp",
        "transform-crop",
        true,
        false,
    ),
    t(
        "changeVideoSpeed",
        "Change Video Speed",
        "Video Hızını Değiştir",
        "media-seek-forward",
        true,
        false,
    ),
    t(
        "joinVideos",
        "Join Videos",
        "Videoları Birleştir",
        "media-playlist-append",
        false,
        true,
    ),
    t(
        "videoSnapshots",
        "Video Snapshots",
        "Video Kareleri",
        "camera-photo",
        false,
        false,
    ),
    t(
        "splitVideo",
        "Split Video",
        "Videoyu Böl",
        "view-split-left-right",
        false,
        false,
    ),
    t(
        "audioToVideo",
        "Audio Visualizer",
        "Ses Görselleştirici",
        "view-media-visualization",
        false,
        false,
    ),
    t(
        "trimAudio",
        "Trim Audio",
        "Sesi Kes",
        "edit-cut",
        true,
        false,
    ),
    t(
        "audioChannels",
        "Convert Audio Channels",
        "Ses Kanallarını Dönüştür",
        "audio-card",
        false,
        false,
    ),
    t(
        "createCollage",
        "Create Collage",
        "Kolaj Oluştur",
        "view-grid",
        false,
        true,
    ),
    t(
        "redactImage",
        "Redact Photo",
        "Fotoğrafı Karart",
        "redact",
        true,
        false,
    ),
    t(
        "redactVideo",
        "Redact Video",
        "Videoyu Karart",
        "redact",
        true,
        false,
    ),
    t(
        "redactAudio",
        "Bleep Audio",
        "Sesi Sansürle",
        "audio-volume-muted",
        true,
        false,
    ),
    t(
        "readQRCode",
        "Read QR Codes",
        "QR Kodlarını Oku",
        "view-barcode-qr",
        false,
        false,
    ),
];

pub fn tool_def(id: &str) -> Option<&'static ToolDef> {
    TOOLS.iter().find(|d| d.id == id)
}

/// Static description of a file action (handled by the UI).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ActionDef {
    pub id: &'static str,
    pub title: Title,
    pub icon: &'static str,
}

pub const ACTIONS: &[ActionDef] = &[
    ActionDef {
        id: "copy",
        title: Title {
            en: "Copy",
            tr: "Kopyala",
        },
        icon: "edit-copy",
    },
    ActionDef {
        id: "copyPath",
        title: Title {
            en: "Copy Path",
            tr: "Yolu Kopyala",
        },
        icon: "edit-copy-path",
    },
    ActionDef {
        id: "terminal",
        title: Title {
            en: "Open Terminal Here",
            tr: "Burada Terminal Aç",
        },
        icon: "utilities-terminal",
    },
    ActionDef {
        id: "openFolder",
        title: Title {
            en: "Open Folder",
            tr: "Klasörü Aç",
        },
        icon: "folder-open",
    },
    ActionDef {
        id: "reveal",
        title: Title {
            en: "Show in File Manager",
            tr: "Dosya Yöneticisinde Göster",
        },
        icon: "system-file-manager",
    },
];

/// Upstream file actions that intentionally have no Linux counterpart.
pub const DROPPED_UPSTREAM_ACTIONS: &[&str] = &["share", "float", "move"];

/// Tools added on Linux for these extensions on top of upstream `singleTools`.
const EXTRA_SINGLE_TOOLS: &[(&str, &str)] = &[
    ("jpg", "readQRCode"),
    ("png", "readQRCode"),
    ("webp", "readQRCode"),
    ("heic", "readQRCode"),
    ("tiff", "readQRCode"),
    ("bmp", "readQRCode"),
];

/// Conversion targets for a canonical source extension (upstream order).
pub fn targets(ext: &str) -> &'static [String] {
    upstream()
        .conversion_matrix
        .get(ext)
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

/// Single-file tools for an extension (upstream + Linux extras).
pub fn single_tools(ext: &str) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = upstream()
        .single_tools
        .get(ext)
        .map(|v| v.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();
    for (e, tool) in EXTRA_SINGLE_TOOLS {
        if *e == ext && !v.contains(tool) {
            v.push(tool);
        }
    }
    v
}

/// Multi-file tools for an extension (upstream).
pub fn multiple_tools(ext: &str) -> Vec<&'static str> {
    upstream()
        .multiple_tools
        .get(ext)
        .map(|v| v.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default()
}

/// Whether `tool` applies to files with extension `ext` (in either list).
pub fn tool_applies(tool: &str, ext: &str) -> bool {
    single_tools(ext).contains(&tool) || multiple_tools(ext).contains(&tool)
}

/// Why a matrix route can never work on Linux, if so.
pub fn unsupported_reason(src: &str, dst: &str) -> Option<&'static str> {
    if dst == "pages" {
        return Some("Apple Pages documents cannot be written on Linux; convert to docx instead");
    }
    if src == "pdf" && dst == "xlsx" {
        return Some("Extracting spreadsheet tables from a PDF is not supported");
    }
    None
}

/// A requirement: at least one of these programs must be present. The first
/// entry is the preferred program and is what gets reported as missing.
pub type AnyOf = &'static [&'static str];

const FFMPEG: AnyOf = &["ffmpeg"];
const MAGICK: AnyOf = &["magick"];
const SOFFICE: AnyOf = &["soffice"];
const PDFTOPPM: AnyOf = &["pdftoppm"];
const PDFTOTEXT: AnyOf = &["pdftotext"];
const GS: AnyOf = &["gs"];
const ZBAR: AnyOf = &["zbarimg"];
const ARCHIVER: AnyOf = &["7z", "bsdtar"];
const EXTRACTOR: AnyOf = &["7z", "bsdtar", "tar", "unzip"];
const IMG_META: AnyOf = &["exiftool", "magick"];
const PDF_META: AnyOf = &["exiftool", "gs"];
const PDF_MERGE: AnyOf = &["qpdf", "pdfunite", "gs"];
const PDF_SPLIT: AnyOf = &["qpdf", "pdfseparate", "gs"];
const PDF_ORGANIZE: AnyOf = &["qpdf", "pdfseparate"];
const SVG_RENDER: AnyOf = &["rsvg-convert", "magick"];
const HEIC: AnyOf = &["libheif"];
const AVIF: AnyOf = &["magick", "ffmpeg"];

/// External programs a conversion route needs. `Err` = unsupported route.
pub fn route_requirements(src: &str, dst: &str) -> Result<Vec<AnyOf>, String> {
    if let Some(r) = unsupported_reason(src, dst) {
        return Err(r.to_string());
    }
    if !targets(src).iter().any(|t| t == dst) {
        return Err(format!("No conversion from .{src} to .{dst}"));
    }
    let sc = category_of(src);
    let dc = target_category(dst);
    let mut req: Vec<AnyOf> = Vec::new();
    match sc {
        Category::Image => {
            if src == "svg" && (dst == "png" || dst == "pdf") {
                req.push(SVG_RENDER);
            } else if src == "avif" || dst == "avif" {
                req.push(AVIF);
                if dst == "pdf" || src == "svg" || src == "heic" || dst == "heic" {
                    req.push(MAGICK);
                }
            } else {
                req.push(MAGICK);
            }
            if src == "heic" || dst == "heic" {
                req.push(HEIC);
            }
        }
        Category::Audio | Category::Video | Category::Gif => req.push(FFMPEG),
        Category::Pdf => match dst {
            "jpg" | "png" => req.push(PDFTOPPM),
            "txt" | "md" | "epub" => req.push(PDFTOTEXT),
            _ => req.push(SOFFICE),
        },
        Category::Subtitle => {}
        Category::Archive => {
            req.push(EXTRACTOR);
            req.push(ARCHIVER);
        }
        Category::Document | Category::Presentation | Category::Spreadsheet | Category::Text => {
            let native = matches!(
                (src, dst),
                ("txt", "md")
                    | ("txt", "epub")
                    | ("textclipping", "txt")
                    | ("textclipping", "md")
                    | ("textclipping", "epub")
            );
            if src == "epub" {
                req.push(&["pandoc", "bsdtar", "7z"]);
                if dst != "html" {
                    req.push(SOFFICE);
                }
            } else if !native {
                req.push(SOFFICE);
            }
            if dc == Category::Image {
                req.push(PDFTOPPM);
            }
        }
        Category::Unknown => return Err(format!("Unknown source format .{src}")),
    }
    Ok(req)
}

/// External programs a tool needs for a file category. `None` = the tool
/// does not apply to this category.
pub fn tool_requirements(tool: &str, cat: Category, ext: &str) -> Option<Vec<AnyOf>> {
    use Category as C;
    let heic = if ext == "heic" { vec![HEIC] } else { vec![] };
    let with = |mut base: Vec<AnyOf>, extra: Vec<AnyOf>| {
        base.extend(extra);
        Some(base)
    };
    match (tool, cat) {
        ("compress", C::Image) => with(vec![MAGICK], heic),
        ("compress", C::Audio | C::Video) => Some(vec![FFMPEG]),
        ("compress", C::Pdf) => Some(vec![GS]),
        ("removeMetadata", C::Image | C::Gif) => with(vec![IMG_META], heic),
        ("removeMetadata", C::Audio | C::Video) => Some(vec![FFMPEG]),
        ("removeMetadata", C::Pdf) => Some(vec![PDF_META]),
        (
            "editImage" | "frameImage" | "cropImage" | "redactImage" | "createCollage"
            | "createPDF",
            C::Image,
        ) => with(vec![MAGICK], heic),
        ("annotateImage", C::Image) => Some(vec![MAGICK]),
        ("readQRCode", C::Image) => Some(vec![ZBAR]),
        (
            "normalizeAudio" | "audioToVideo" | "trimAudio" | "audioChannels" | "redactAudio",
            C::Audio,
        ) => Some(vec![FFMPEG]),
        (
            "muteVideo" | "trimVideo" | "cropVideo" | "changeVideoSpeed" | "joinVideos"
            | "videoSnapshots" | "splitVideo" | "redactVideo",
            C::Video,
        ) => Some(vec![FFMPEG]),
        ("splitPDF", C::Pdf) => Some(vec![PDF_SPLIT]),
        ("organizePDF", C::Pdf) => Some(vec![PDF_ORGANIZE]),
        ("mergePDF", C::Pdf) => Some(vec![PDF_MERGE]),
        ("extractArchive", C::Archive) => Some(vec![EXTRACTOR]),
        _ => None,
    }
}

/// Programs from `req` that are not satisfied (preferred name of each group).
pub fn missing(req: &[AnyOf], deps: &Deps) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for group in req {
        if !group.iter().any(|t| deps.has(t)) {
            let name = group[0].to_string();
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// JSON views
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct FormatView {
    pub ext: &'static str,
    pub category: Category,
    pub title: String,
    pub targets: Vec<String>,
    pub unsupported_targets: Vec<String>,
    pub single_tools: Vec<&'static str>,
    pub multiple_tools: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct ToolView {
    pub id: &'static str,
    pub title: Title,
    pub icon: &'static str,
    pub needs_editor: bool,
    pub multi_input: bool,
    pub available: bool,
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CatalogView {
    pub formats: Vec<FormatView>,
    pub tools: Vec<ToolView>,
    pub actions: &'static [ActionDef],
}

/// Full catalog with availability.
pub fn catalog(deps: &Deps) -> CatalogView {
    let formats = FORMATS
        .iter()
        .map(|(ext, cat)| FormatView {
            ext,
            category: *cat,
            title: ext.to_uppercase(),
            targets: targets(ext).to_vec(),
            unsupported_targets: targets(ext)
                .iter()
                .filter(|d| unsupported_reason(ext, d).is_some())
                .cloned()
                .collect(),
            single_tools: single_tools(ext),
            multiple_tools: multiple_tools(ext),
        })
        .collect();
    let tools = TOOLS
        .iter()
        .map(|d| {
            // Available when at least one applicable format is fully served.
            let mut any_ok = false;
            let mut miss: Vec<String> = Vec::new();
            for (ext, cat) in FORMATS {
                if !tool_applies(d.id, ext) {
                    continue;
                }
                if let Some(req) = tool_requirements(d.id, *cat, ext) {
                    let m = missing(&req, deps);
                    if m.is_empty() {
                        any_ok = true;
                    }
                    for x in m {
                        if !miss.contains(&x) {
                            miss.push(x);
                        }
                    }
                }
            }
            ToolView {
                id: d.id,
                title: d.title,
                icon: d.icon,
                needs_editor: d.needs_editor,
                multi_input: d.multi_input,
                available: any_ok,
                missing: if any_ok { Vec::new() } else { miss },
            }
        })
        .collect();
    CatalogView {
        formats,
        tools,
        actions: ACTIONS,
    }
}

#[derive(Debug, Serialize)]
pub struct WheelFile {
    pub path: String,
    pub ext: String,
    pub category: Category,
}

#[derive(Debug, Serialize)]
pub struct WheelTarget {
    pub id: String,
    pub title: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct WheelTool {
    pub id: &'static str,
    pub title: Title,
    pub icon: &'static str,
    pub available: bool,
    pub needs_editor: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct WheelView {
    pub files: Vec<WheelFile>,
    pub convert: Vec<WheelTarget>,
    pub tools: Vec<WheelTool>,
    pub actions: &'static [ActionDef],
}

fn intersect(lists: Vec<Vec<String>>) -> Vec<String> {
    let mut iter = lists.into_iter();
    let Some(first) = iter.next() else {
        return Vec::new();
    };
    let rest: Vec<Vec<String>> = iter.collect();
    first
        .into_iter()
        .filter(|x| rest.iter().all(|l| l.contains(x)))
        .collect()
}

/// Wheel items for exactly these files. `files` are `(path, ext, category)`.
pub fn wheel(files: &[(String, String, Category)], deps: &Deps) -> WheelView {
    let wf = files
        .iter()
        .map(|(p, e, c)| WheelFile {
            path: p.clone(),
            ext: e.clone(),
            category: *c,
        })
        .collect();
    if files.is_empty() {
        return WheelView {
            files: wf,
            convert: vec![],
            tools: vec![],
            actions: ACTIONS,
        };
    }
    // Convert targets: intersection, minus routes that can never work.
    let lists = files.iter().map(|(_, e, _)| targets(e).to_vec()).collect();
    let convert = intersect(lists)
        .into_iter()
        .filter_map(|dst| {
            let mut miss = Vec::new();
            for (_, src, _) in files {
                match route_requirements(src, &dst) {
                    Ok(req) => {
                        for m in missing(&req, deps) {
                            if !miss.contains(&m) {
                                miss.push(m);
                            }
                        }
                    }
                    Err(_) => return None,
                }
            }
            Some(WheelTarget {
                title: dst.to_uppercase(),
                id: dst,
                available: miss.is_empty(),
                missing: miss,
            })
        })
        .collect();
    // Tools.
    let tool_ids: Vec<String> = if files.len() == 1 {
        single_tools(&files[0].1)
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        intersect(
            files
                .iter()
                .map(|(_, e, _)| multiple_tools(e).iter().map(|s| s.to_string()).collect())
                .collect(),
        )
    };
    let tools = tool_ids
        .iter()
        .filter_map(|id| {
            let def = tool_def(id)?;
            let mut miss = Vec::new();
            for (_, ext, cat) in files {
                let req = tool_requirements(id, *cat, ext)?;
                for m in missing(&req, deps) {
                    if !miss.contains(&m) {
                        miss.push(m);
                    }
                }
            }
            Some(WheelTool {
                id: def.id,
                title: def.title,
                icon: def.icon,
                available: miss.is_empty(),
                needs_editor: def.needs_editor,
                missing: miss,
            })
        })
        .collect();
    WheelView {
        files: wf,
        convert,
        tools,
        actions: ACTIONS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_upstream_ext_is_covered() {
        let u = upstream();
        let ours: BTreeSet<&str> = FORMATS.iter().map(|(e, _)| *e).collect();
        let theirs: BTreeSet<&str> = u.conversion_matrix.keys().map(|s| s.as_str()).collect();
        assert_eq!(
            ours, theirs,
            "FORMATS must match upstream conversionMatrix keys"
        );
        assert_eq!(FORMATS.len(), 45);
        for k in u.single_tools.keys().chain(u.multiple_tools.keys()) {
            assert!(ours.contains(k.as_str()), "tool list for unknown ext {k}");
        }
        // Every target is a known format or html.
        for (src, dsts) in &u.conversion_matrix {
            for d in dsts {
                assert!(
                    ours.contains(d.as_str()) || d == "html",
                    "{src} -> {d}: unknown target"
                );
            }
        }
    }

    #[test]
    fn every_upstream_tool_is_covered() {
        let u = upstream();
        for (id, name) in &u.tool_names {
            let d = tool_def(id).unwrap_or_else(|| panic!("tool {id} missing from TOOLS"));
            assert_eq!(d.title.en, name, "en title of {id} must match upstream");
            assert!(!d.title.tr.is_empty());
            assert!(!d.icon.is_empty());
        }
        assert_eq!(TOOLS.len(), u.tool_names.len());
        for list in u.single_tools.values().chain(u.multiple_tools.values()) {
            for id in list {
                assert!(tool_def(id).is_some(), "tool {id} used but not defined");
            }
        }
        for (_, id) in EXTRA_SINGLE_TOOLS {
            assert!(tool_def(id).is_some());
        }
    }

    #[test]
    fn every_tool_listed_for_an_ext_has_requirements() {
        for (ext, cat) in FORMATS {
            for tool in single_tools(ext).into_iter().chain(multiple_tools(ext)) {
                assert!(
                    tool_requirements(tool, *cat, ext).is_some(),
                    "{tool} listed for {ext} but has no requirements"
                );
            }
        }
    }

    #[test]
    fn every_route_is_planned_or_explicitly_unsupported() {
        for (src, _) in FORMATS {
            for dst in targets(src) {
                match route_requirements(src, dst) {
                    Ok(_) => {}
                    Err(reason) => assert!(
                        unsupported_reason(src, dst).is_some(),
                        "{src} -> {dst} unexpectedly unsupported: {reason}"
                    ),
                }
            }
        }
    }

    #[test]
    fn actions_match_upstream_minus_dropped() {
        let ids: Vec<&str> = ACTIONS.iter().map(|a| a.id).collect();
        assert_eq!(
            ids,
            ["copy", "copyPath", "terminal", "openFolder", "reveal"]
        );
        for a in &upstream().file_actions {
            assert!(
                ids.contains(&a.as_str()) || DROPPED_UPSTREAM_ACTIONS.contains(&a.as_str()),
                "upstream action {a} not handled"
            );
        }
    }

    #[test]
    fn wheel_intersections() {
        let deps = Deps::none();
        let f = |p: &str, e: &str| (p.to_string(), e.to_string(), category_of(e));
        let w = wheel(&[f("/a.mp4", "mp4")], &deps);
        assert!(w.convert.iter().any(|t| t.id == "mp3"));
        assert!(w
            .convert
            .iter()
            .all(|t| !t.available && t.missing == ["ffmpeg"]));
        assert!(w.tools.iter().any(|t| t.id == "muteVideo"));
        let w = wheel(&[f("/a.jpg", "jpg"), f("/b.png", "png")], &deps);
        let ids: Vec<&str> = w.convert.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["webp", "heic", "tiff", "avif", "bmp", "pdf"]);
        let tools: Vec<&str> = w.tools.iter().map(|t| t.id).collect();
        assert_eq!(
            tools,
            [
                "compress",
                "removeMetadata",
                "cropImage",
                "createPDF",
                "createCollage"
            ]
        );
        let w = wheel(&[f("/a.jpg", "jpg"), f("/b.mp3", "mp3")], &deps);
        assert!(w.convert.is_empty());
        let w = wheel(&[f("/a.docx", "docx")], &deps);
        assert!(
            w.convert.iter().all(|t| t.id != "pages"),
            "unsupported routes hidden"
        );
    }
}

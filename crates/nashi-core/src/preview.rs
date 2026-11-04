//! Preview rendering (`nashi-cli preview`): single image/video frame, audio
//! waveform PNG, and PDF page thumbnails for the UI editors.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::Category;
use crate::deps::{require, Deps};
use crate::detect::{detect, ffprobe, pdf_pages, png_dimensions};
use crate::engines::image;
use crate::events::{path_str, JobError, JobResult};
use crate::fsops::{files_in, unique_path, WorkDir};
use crate::options::{parse_pages, valid_color};
use crate::process::{check_cancel, failure, Cmd};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewKind {
    Frame,
    Waveform,
    Pages,
}

impl PreviewKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PreviewKind::Frame => "frame",
            PreviewKind::Waveform => "waveform",
            PreviewKind::Pages => "pages",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreviewRequest {
    pub file: PathBuf,
    pub kind: PreviewKind,
    pub out_dir: PathBuf,
    pub at: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pages: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewOut {
    pub kind: PreviewKind,
    pub files: Vec<String>,
    pub width: u32,
    pub height: u32,
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

fn same_file(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
        return false;
    };
    ma.dev() == mb.dev() && ma.ino() == mb.ino()
}

/// Move `src` (inside `WorkDir`) into `out_dir/<stem>.png`, never overwriting
/// `input_file` even if `out_dir` is the input's own directory.
fn commit_preview(src: &Path, out_dir: &Path, input_file: &Path, stem: &str) -> JobResult<PathBuf> {
    let preferred = out_dir.join(format!("{stem}.png"));
    let dst = if same_file(input_file, &preferred) {
        unique_path(out_dir, stem, "png")
    } else {
        preferred
    };
    match std::fs::rename(src, &dst) {
        Ok(()) => {}
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            std::fs::copy(src, &dst)?;
            let _ = std::fs::remove_file(src);
        }
        Err(e) => return Err(e.into()),
    }
    Ok(dst)
}

/// Render a preview according to `req`.
pub fn run(deps: &Deps, req: &PreviewRequest) -> JobResult<PreviewOut> {
    check_cancel()?;
    for (name, val) in [("width", req.width), ("height", req.height)] {
        if let Some(v) = val {
            if v == 0 || v > 16384 {
                return Err(JobError::invalid_options(format!(
                    "{name} must be in 1..=16384"
                )));
            }
        }
    }
    if let Some(at) = req.at {
        if !at.is_finite() {
            return Err(JobError::invalid_options("--at must be a finite number"));
        }
    }
    if let Some(c) = &req.color {
        if !valid_color(c) {
            return Err(JobError::invalid_options(format!("invalid color '{c}'")));
        }
    }

    let file = absolute(&req.file);
    let meta = std::fs::metadata(&file)
        .map_err(|e| JobError::invalid_input(format!("{}: {e}", file.display())))?;
    if meta.is_dir() {
        return Err(JobError::invalid_input(format!(
            "{} is a directory",
            file.display()
        )));
    }
    let det = detect(&file);
    let out_dir = absolute(&req.out_dir);
    std::fs::create_dir_all(&out_dir)?;
    let work = WorkDir::new_in(&out_dir)?;

    match req.kind {
        PreviewKind::Frame => {
            render_frame(deps, req, &file, &det.ext, det.category, &out_dir, &work)
        }
        PreviewKind::Waveform => {
            render_waveform(deps, req, &file, &det.ext, det.category, &out_dir, &work)
        }
        PreviewKind::Pages => {
            render_pages(deps, req, &file, &det.ext, det.category, &out_dir, &work)
        }
    }
}

fn render_frame(
    deps: &Deps,
    req: &PreviewRequest,
    file: &Path,
    ext: &str,
    cat: Category,
    out_dir: &Path,
    work: &WorkDir,
) -> JobResult<PreviewOut> {
    let w = req.width.unwrap_or(1600);
    match cat {
        Category::Image => {
            if ext == "heic" && !deps.has("libheif") {
                return Err(JobError::missing("libheif", "libheif"));
            }
            if ext == "svg" && !deps.has("rsvg-convert") && !deps.magick_can("SVG", false) {
                return Err(JobError::missing("rsvg-convert", "librsvg"));
            }
            require(deps, "magick")?;
            let (src, src_ext) = image::decode_source(deps, file, ext, work)?;
            let h_bound = req.height.unwrap_or(w);
            image::magick(deps, work)?
                .arg(image::ispec(work, &src, &src_ext, true)?)
                .args(["-auto-orient", "-thumbnail", &format!("{w}x{h_bound}>")])
                .arg(image::ospec("png", "frame.png"))
                .run_ok()?;
            let tmp = work.join("frame.png");
            let (pw, ph) = png_dimensions(&tmp)
                .ok_or_else(|| JobError::tool_failed("failed to read preview PNG dimensions"))?;
            let dst = commit_preview(&tmp, out_dir, file, "frame")?;
            Ok(PreviewOut {
                kind: PreviewKind::Frame,
                files: vec![path_str(&dst)],
                width: pw,
                height: ph,
            })
        }
        Category::Video | Category::Gif => {
            let ff = require(deps, "ffmpeg")?;
            let probe = ffprobe(deps, file);
            if let Some(p) = &probe {
                if !p.has_video && cat != Category::Gif {
                    return Err(JobError::invalid_input("the file has no video stream"));
                }
            }
            let mut at = req.at.unwrap_or(0.0).max(0.0);
            if let Some(d) = probe.as_ref().and_then(|p| p.duration) {
                if at >= d {
                    at = (d - 0.05).max(0.0);
                }
            }
            let scale = match req.height {
                Some(h) => format!(
                    "scale='min({w},iw)':'min({h},ih)':force_original_aspect_ratio=decrease"
                ),
                None => format!("scale='min({w},iw)':-1"),
            };
            let tmp = work.join("frame.png");
            let extract = |seek_flag: &str, seek_val: &str| -> JobResult<()> {
                let _ = std::fs::remove_file(&tmp);
                Cmd::new(ff)
                    .args([
                        "-hide_banner",
                        "-nostdin",
                        "-y",
                        "-loglevel",
                        "error",
                        seek_flag,
                        seek_val,
                        "-i",
                    ])
                    .arg_cat("file:", file)
                    .args([
                        "-frames:v",
                        "1",
                        "-vf",
                        &scale,
                        "-c:v",
                        "png",
                        "-f",
                        "image2pipe",
                    ])
                    .arg_cat("file:", &tmp)
                    .run_ok()?;
                Ok(())
            };
            extract("-ss", &format!("{at:.3}"))?;
            if png_dimensions(&tmp).is_none() && at > 0.0 {
                // Seek landed past the last frame's PTS near EOF; grab the tail frame.
                let _ = extract("-sseof", "-0.25");
                if png_dimensions(&tmp).is_none() {
                    extract("-ss", "0")?;
                }
            }
            let (pw, ph) = png_dimensions(&tmp)
                .ok_or_else(|| JobError::tool_failed("ffmpeg produced no preview frame"))?;
            let dst = commit_preview(&tmp, out_dir, file, "frame")?;
            Ok(PreviewOut {
                kind: PreviewKind::Frame,
                files: vec![path_str(&dst)],
                width: pw,
                height: ph,
            })
        }
        _ => Err(JobError::invalid_input(format!(
            "frame preview requires an image, video or GIF (got .{ext})"
        ))),
    }
}

fn render_waveform(
    deps: &Deps,
    req: &PreviewRequest,
    file: &Path,
    ext: &str,
    cat: Category,
    out_dir: &Path,
    work: &WorkDir,
) -> JobResult<PreviewOut> {
    if !matches!(cat, Category::Audio | Category::Video) {
        return Err(JobError::invalid_input(format!(
            "waveform preview requires an audio or video file (got .{ext})"
        )));
    }
    let ff = require(deps, "ffmpeg")?;
    if let Some(p) = ffprobe(deps, file) {
        if !p.has_audio {
            return Err(JobError::invalid_input("the file has no audio stream"));
        }
    }
    let w = req.width.unwrap_or(1200);
    let h = req.height.unwrap_or(160);
    let color = req.color.as_deref().unwrap_or("#30543b");
    let filter =
        format!("[0:a:0]aformat=channel_layouts=mono,showwavespic=s={w}x{h}:colors={color}");
    let tmp = work.join("waveform.png");
    let out = Cmd::new(ff)
        .args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-i"])
        .arg_cat("file:", file)
        .args([
            "-filter_complex",
            &filter,
            "-frames:v",
            "1",
            "-c:v",
            "png",
            "-f",
            "image2pipe",
        ])
        .arg_cat("file:", &tmp)
        .run()?;
    if !out.status.success() {
        if out.stderr.contains("matches no streams") {
            return Err(JobError::invalid_input("the file has no audio stream"));
        }
        return Err(failure("ffmpeg", &out));
    }
    let (pw, ph) = png_dimensions(&tmp)
        .ok_or_else(|| JobError::tool_failed("ffmpeg produced no waveform image"))?;
    let dst = commit_preview(&tmp, out_dir, file, "waveform")?;
    Ok(PreviewOut {
        kind: PreviewKind::Waveform,
        files: vec![path_str(&dst)],
        width: pw,
        height: ph,
    })
}

fn render_pages(
    deps: &Deps,
    req: &PreviewRequest,
    file: &Path,
    ext: &str,
    cat: Category,
    out_dir: &Path,
    work: &WorkDir,
) -> JobResult<PreviewOut> {
    if cat != Category::Pdf {
        return Err(JobError::invalid_input(format!(
            "pages preview requires a PDF file (got .{ext})"
        )));
    }
    let ppm = require(deps, "pdftoppm")?;
    let w = req.width.unwrap_or(240).to_string();
    let count = pdf_pages(deps, file);
    let mut out_files = Vec::new();
    let mut first_dims: Option<(u32, u32)> = None;

    if let Some(spec) = &req.pages {
        let pages = parse_pages(spec, count)?;
        let mut cache: HashMap<u32, PathBuf> = HashMap::new();
        for (idx, &page) in pages.iter().enumerate() {
            check_cancel()?;
            let rendered = if let Some(cached) = cache.get(&page) {
                cached.clone()
            } else {
                let p_str = page.to_string();
                let prefix = work.join(&format!("src-{page}"));
                Cmd::new(ppm)
                    .args([
                        "-png",
                        "-scale-to-x",
                        &w,
                        "-scale-to-y",
                        "-1",
                        "-f",
                        &p_str,
                        "-l",
                        &p_str,
                        "-singlefile",
                    ])
                    .arg(file)
                    .arg(&prefix)
                    .run_ok()?;
                let png = work.join(&format!("src-{page}.png"));
                if !png.exists() {
                    return Err(JobError::tool_failed(format!(
                        "pdftoppm produced no output for page {page}"
                    )));
                }
                cache.insert(page, png.clone());
                png
            };
            let copy_tmp = work.join(&format!("out-{idx}.png"));
            std::fs::copy(&rendered, &copy_tmp)?;
            if first_dims.is_none() {
                first_dims = png_dimensions(&copy_tmp);
            }
            let dst = commit_preview(&copy_tmp, out_dir, file, &format!("page-{:03}", idx + 1))?;
            out_files.push(path_str(&dst));
        }
    } else {
        Cmd::new(ppm)
            .args(["-png", "-scale-to-x", &w, "-scale-to-y", "-1"])
            .arg(file)
            .arg(work.join("p"))
            .run_ok()?;
        let rendered = files_in(work.path())?;
        if rendered.is_empty() {
            return Err(JobError::tool_failed("pdftoppm produced no pages"));
        }
        for (idx, src) in rendered.into_iter().enumerate() {
            check_cancel()?;
            if first_dims.is_none() {
                first_dims = png_dimensions(&src);
            }
            let dst = commit_preview(&src, out_dir, file, &format!("page-{:03}", idx + 1))?;
            out_files.push(path_str(&dst));
        }
    }

    let (pw, ph) =
        first_dims.ok_or_else(|| JobError::tool_failed("failed to read page PNG dimensions"))?;
    Ok(PreviewOut {
        kind: PreviewKind::Pages,
        files: out_files,
        width: pw,
        height: ph,
    })
}

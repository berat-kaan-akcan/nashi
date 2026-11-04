//! Images: ImageMagick, with rsvg-convert for SVG, ffmpeg for AVIF and
//! libheif tools for HEIC when ImageMagick lacks those delegates.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Ctx, Input, Produced};
use crate::deps::{require, Deps};
use crate::events::{JobError, JobResult};
use crate::fsops::{Staged, WorkDir};
use crate::process::{failure, Cmd};

fn im_format(ext: &str) -> &str {
    match ext {
        "jpg" => "JPEG",
        "png" => "PNG",
        "webp" => "WEBP",
        "tiff" => "TIFF",
        "bmp" => "BMP",
        "heic" => "HEIC",
        "avif" => "AVIF",
        "pdf" => "PDF",
        "gif" => "GIF",
        "svg" => "SVG",
        _ => "PNG",
    }
}

/// ffmpeg encoder/muxer args for still images ImageMagick may lack.
fn ff_encoder(ext: &str) -> Option<&'static [&'static str]> {
    Some(match ext {
        "avif" => &["-c:v", "libaom-av1", "-still-picture", "1", "-f", "avif"],
        "tiff" => &[
            "-c:v",
            "tiff",
            "-compression_algo",
            "deflate",
            "-f",
            "image2pipe",
        ],
        "bmp" => &["-c:v", "bmp", "-f", "image2pipe"],
        "png" => &["-c:v", "png", "-f", "image2pipe"],
        "jpg" => &["-c:v", "mjpeg", "-q:v", "2", "-f", "image2pipe"],
        "webp" => &["-c:v", "libwebp", "-f", "image2pipe"],
        _ => return None,
    })
}

/// Restrictive ImageMagick policy: no MVG/MSL/TEXT/URL/EPHEMERAL/... coders,
/// no external delegates, no `@file` indirection.
pub const MAGICK_POLICY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<policymap>
  <policy domain="delegate" rights="none" pattern="*"/>
  <policy domain="coder" rights="none" pattern="{MVG,MSL,MSVG,TEXT,TXT,LABEL,CAPTION,PANGO,URL,HTTP,HTTPS,FTP,EPHEMERAL,SHOW,WIN,X,VID,INLINE,PS,PS2,PS3,EPS,EPI,EPSF,EPSI,EPT,XPS,PCL,SCAN,PLASMA,FRACTAL}"/>
  <policy domain="path" rights="none" pattern="@*"/>
</policymap>
"#;

/// Directory holding [`MAGICK_POLICY`] inside `work` (for MAGICK_CONFIGURE_PATH).
pub fn policy_dir(work: &WorkDir) -> std::io::Result<PathBuf> {
    let d = work.join(".im");
    if !d.join("policy.xml").exists() {
        std::fs::create_dir_all(&d)?;
        std::fs::write(d.join("policy.xml"), MAGICK_POLICY)?;
    }
    Ok(d)
}

/// Run an ImageMagick-based program inside `work` under the restrictive policy.
fn im_cmd(prog: &Path, work: &WorkDir) -> JobResult<Cmd> {
    Ok(Cmd::new(prog)
        .cwd(work.path())
        .env("MAGICK_CONFIGURE_PATH", policy_dir(work)?))
}

/// `magick` (IM7) or `convert` (IM6), confined to `work`.
pub fn magick(deps: &Deps, work: &WorkDir) -> JobResult<Cmd> {
    im_cmd(require(deps, "magick")?, work)
}

/// `magick montage` (IM7) or `montage` (IM6).
fn montage(deps: &Deps, work: &WorkDir) -> JobResult<Cmd> {
    if deps.is_magick6() {
        im_cmd(require(deps, "montage")?, work)
    } else {
        Ok(magick(deps, work)?.arg("montage"))
    }
}

/// Formats that can hold several frames/pages.
fn multi_frame(ext: &str) -> bool {
    matches!(ext, "gif" | "webp" | "tiff")
}

static INPUT_N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Place `p` in `work` under a plain name (hard link, else copy) so that no
/// part of the user's path (`%d`, `[...]`, `@`, `name:`) is interpreted.
/// Returns the name relative to `work`. SVGs get a directory of their own so
/// a renderer resolving relative references only sees that one file.
fn place(p: &Path, ext: &str, work: &WorkDir) -> JobResult<String> {
    if p.parent() == Some(work.path()) {
        if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
            if n.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
            {
                return Ok(n.to_string());
            }
        }
    }
    let n = INPUT_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let rel = if ext == "svg" {
        std::fs::create_dir_all(work.join(&format!("svg{n}")))?;
        format!("svg{n}/input.svg")
    } else {
        format!("input-{n}.{ext}")
    };
    let dst = work.join(&rel);
    if std::fs::hard_link(p, &dst).is_err() {
        std::fs::copy(p, &dst)?;
    }
    Ok(rel)
}

/// Input spec: explicit coder + relative name (+ `[0]` for the first frame).
pub(crate) fn ispec(work: &WorkDir, p: &Path, ext: &str, first_frame: bool) -> JobResult<OsString> {
    let rel = place(p, ext, work)?;
    let mut s = format!("{}:{rel}", im_format(ext));
    if first_frame {
        s.push_str("[0]");
    }
    Ok(s.into())
}

/// Output spec: explicit coder + plain relative name inside the work dir.
pub(crate) fn ospec(ext: &str, name: &str) -> OsString {
    format!("{}:{name}", im_format(ext)).into()
}

fn flatten_args(target: &str) -> Vec<&'static str> {
    if matches!(target, "jpg" | "bmp") {
        vec!["-background", "white", "-alpha", "remove", "-alpha", "off"]
    } else {
        vec![]
    }
}

/// Decode `path` (of format `ext`) into a raster source ImageMagick can read,
/// rendering SVG/HEIC/AVIF/TIFF through helpers when needed. Returns `(path, ext)`.
pub(crate) fn decode_source(
    deps: &Deps,
    path: &Path,
    ext: &str,
    work: &WorkDir,
) -> JobResult<(PathBuf, String)> {
    match ext {
        "svg" => {
            if let Some(rsvg) = deps.path("rsvg-convert") {
                let rel = place(path, "svg", work)?;
                let out = work.join("render.png");
                Cmd::new(rsvg)
                    .args(["-f", "png", "-o"])
                    .arg(&out)
                    .arg(work.join(&rel))
                    .run_ok()?;
                return Ok((out, "png".into()));
            }
        }
        "heic" if !deps.magick_can("HEIC", false) => {
            if let Some(dec) = deps.path("heif-dec") {
                let out = work.join("decoded.png");
                Cmd::new(dec).arg("-o").arg(&out).arg(path).run_ok()?;
                return Ok((out, "png".into()));
            }
            return Err(JobError::missing("libheif", "libheif"));
        }
        e if ff_encoder(e).is_some() && !deps.magick_can(im_format(e), false) => {
            // ImageMagick lacks this coder (e.g. TIFF/AVIF): decode with ffmpeg.
            let ff = require(deps, "ffmpeg")?;
            let out = work.join("decoded.png");
            Cmd::new(ff)
                .args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-i"])
                .arg_cat("file:", path)
                .args(["-frames:v", "1", "-c:v", "png", "-f", "image2pipe"])
                .arg_cat("file:", &out)
                .run_ok()?;
            return Ok((out, "png".into()));
        }
        _ => {}
    }
    Ok((path.to_path_buf(), ext.to_string()))
}

fn readable_source(ctx: &Ctx, input: &Input, work: &WorkDir) -> JobResult<(PathBuf, String)> {
    decode_source(ctx.deps, &input.path, input.ext(), work)
}

/// Write `src` (of format `src_ext`) as `target` into `staged`.
#[allow(clippy::too_many_arguments)]
fn write_as(
    ctx: &Ctx,
    src: &Path,
    src_ext: &str,
    target: &str,
    extra: &[OsString],
    first_only: bool,
    staged: &Staged,
    work: &WorkDir,
) -> JobResult<()> {
    let d = ctx.deps;
    // Keep every frame when both sides can hold several (animated GIF/WebP,
    // multi-page TIFF) and for PDF output; otherwise use the first frame.
    let first = first_only || !(target == "pdf" || (multi_frame(src_ext) && multi_frame(target)));
    match target {
        "heic" if !d.magick_can("HEIC", true) => {
            let enc = d
                .path("heif-enc")
                .ok_or_else(|| JobError::missing("libheif", "libheif"))?;
            magick(d, work)?
                .arg(ispec(work, src, src_ext, true)?)
                .args(extra)
                .arg(ospec("png", "pre.png"))
                .run_ok()?;
            let q = ctx.opts.quality.unwrap_or(80).to_string();
            let out = work.join("out.heic");
            Cmd::new(enc)
                .args(["-q", &q, "-o"])
                .arg(&out)
                .arg(work.join("pre.png"))
                .run_ok()?;
            staged.take_from(&out)?;
        }
        t if ff_encoder(t).is_some() && !d.magick_can(im_format(t), true) => {
            // ImageMagick lacks this writer: normalise through ImageMagick when
            // present (applies `extra`), then encode with ffmpeg.
            let ff = require(d, "ffmpeg")?;
            let mut source = src.to_path_buf();
            if d.has("magick") {
                magick(d, work)?
                    .arg(ispec(work, src, src_ext, true)?)
                    .args(extra)
                    .arg(ospec("png", "pre.png"))
                    .run_ok()?;
                source = work.join("pre.png");
            }
            let mut enc: Vec<String> = ff_encoder(t)
                .unwrap()
                .iter()
                .map(|x| x.to_string())
                .collect();
            if t == "avif" {
                let crf = ctx.opts.quality.map(|q| 63 - q * 63 / 100).unwrap_or(30);
                enc.extend(["-crf".to_string(), crf.to_string()]);
            }
            Cmd::new(ff)
                .args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-i"])
                .arg_cat("file:", &source)
                .args(["-frames:v", "1"])
                .args(enc)
                .arg_cat("file:", staged.path())
                .run_ok()?;
        }
        _ => {
            let name = format!("out.{target}");
            magick(d, work)?
                .arg(ispec(work, src, src_ext, first)?)
                .args(flatten_args(target))
                .args(extra)
                .arg(ospec(target, &name))
                .run_ok()?;
            staged.take_from(&work.join(&name))?;
        }
    }
    Ok(())
}

fn oriented_size_in_work(
    deps: &Deps,
    work: &WorkDir,
    src: &Path,
    src_ext: &str,
) -> Option<(u32, u32)> {
    if let Ok(cmd) = magick(deps, work) {
        if let Ok(spec) = ispec(work, src, src_ext, true) {
            if let Ok(out) = cmd
                .arg(spec)
                .args(["-auto-orient", "-format", "%w %h\n", "INFO:"])
                .run_ok()
            {
                let mut it = out.stdout.split_whitespace();
                if let (Some(w), Some(h)) = (
                    it.next().and_then(|s| s.parse().ok()),
                    it.next().and_then(|s| s.parse().ok()),
                ) {
                    return Some((w, h));
                }
            }
        }
    }
    if src_ext == "png" {
        return crate::detect::png_dimensions(src);
    }
    None
}

/// Auto-oriented image dimensions via ImageMagick (confined, explicit coder).
pub fn identify_size(deps: &Deps, path: &Path, ext: &str) -> Option<(u32, u32)> {
    let work = WorkDir::system().ok()?;
    let (src, src_ext) = decode_source(deps, path, ext, &work).ok()?;
    oriented_size_in_work(deps, &work, &src, &src_ext)
}

/// Convert an image to another image format or PDF.
pub fn convert(ctx: &Ctx, input: &Input, target: &str) -> JobResult<PathBuf> {
    ctx.progress(None, "Converting");
    let staged = input.stage("", target)?;
    let d = ctx.deps;
    // SVG straight to PNG/PDF with rsvg-convert when available.
    let work = WorkDir::new_in(&input.out_dir)?;
    if input.ext() == "svg" && (target == "png" || target == "pdf") {
        if let Some(rsvg) = d.path("rsvg-convert") {
            let rel = place(&input.path, "svg", &work)?;
            Cmd::new(rsvg)
                .args(["-f", target, "-o"])
                .arg(staged.path())
                .arg(work.join(&rel))
                .run_ok()?;
            return Ok(staged.commit()?);
        }
    }
    let (src, src_ext) = readable_source(ctx, input, &work)?;
    // Not every viewer honours an orientation tag, so turn the pixels themselves.
    let mut extra: Vec<OsString> = vec!["-auto-orient".into()];
    if matches!(target, "jpg" | "webp" | "heic" | "avif") && d.magick_can(im_format(target), true) {
        extra.push("-quality".into());
        extra.push(ctx.opts.quality.unwrap_or(90).to_string().into());
    }
    write_as(ctx, &src, &src_ext, target, &extra, false, &staged, &work)?;
    Ok(staged.commit()?)
}

/// Apply ImageMagick operations, keeping the format, into `stem<suffix>.ext`.
fn process(ctx: &Ctx, input: &Input, suffix: &str, ops: Vec<OsString>) -> JobResult<Produced> {
    ctx.progress(None, "Processing");
    let ext = input.ext();
    let out_ext = if ext == "svg" { "png" } else { ext };
    let staged = input.stage(suffix, out_ext)?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let (src, src_ext) = readable_source(ctx, input, &work)?;
    write_as(ctx, &src, &src_ext, out_ext, &ops, false, &staged, &work)?;
    Ok(staged.commit()?.into())
}

fn os(v: &[String]) -> Vec<OsString> {
    v.iter().map(OsString::from).collect()
}

/// `-resize` operations for the size options of `compress`.
fn compress_resize(o: &crate::options::Options) -> JobResult<Vec<String>> {
    let mut ops = Vec::new();
    if let Some(edge) = o.max_edge {
        if !(1..=32000).contains(&edge) {
            return Err(JobError::invalid_options(
                "max_edge must be between 1 and 32000",
            ));
        }
        ops.extend(["-resize".to_string(), format!("{edge}x{edge}>")]);
    }
    if let Some(scale) = o.image_scale {
        if !scale.is_finite() || scale <= 0.0 || scale > 1.0 {
            return Err(JobError::invalid_options(
                "image_scale must be greater than 0 and at most 1",
            ));
        }
        ops.extend(["-resize".to_string(), format!("{}%", scale * 100.0)]);
    }
    if let Some(rs) = o.resize.as_ref().filter(|r| r.w.is_some() || r.h.is_some()) {
        let w = rs.w.map(|v| v.to_string()).unwrap_or_default();
        let h = rs.h.map(|v| v.to_string()).unwrap_or_default();
        let exact = if rs.w.is_some() && rs.h.is_some() {
            "!"
        } else {
            ""
        };
        ops.extend(["-resize".to_string(), format!("{w}x{h}{exact}")]);
    }
    Ok(ops)
}

fn compress(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    let o = ctx.opts;
    let d = ctx.deps;
    let ext = input.ext();
    let resize = compress_resize(o)?;
    let target = super::compress_target(o, input)?;
    let encode = |quality: u32| -> Vec<String> {
        match ext {
            "png" => vec![
                "-strip".into(),
                "-define".into(),
                "png:compression-level=9".into(),
                "-define".into(),
                "png:compression-filter=5".into(),
            ],
            "tiff" => vec!["-compress".into(), "zip".into()],
            _ => vec!["-quality".into(), quality.to_string()],
        }
    };
    let mut quality = o.quality.unwrap_or(82);
    let Some(target) = target else {
        let mut ops = encode(quality);
        ops.extend(resize);
        return process(ctx, input, "-compressed", os(&ops));
    };
    let out_ext = if ext == "svg" { "png" } else { ext };
    if resize.is_empty() && ext != "svg" && std::fs::metadata(&input.path)?.len() <= target {
        return super::keep_original(input, ext);
    }
    ctx.progress(None, "Compressing");
    let staged = input.stage("-compressed", out_ext)?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let (src, src_ext) = readable_source(ctx, input, &work)?;
    // Quality only reaches lossy formats that ImageMagick itself writes.
    let lossy = matches!(out_ext, "jpg" | "webp" | "heic" | "avif")
        && d.magick_can(im_format(out_ext), true);
    let mut scale = 1.0f64;
    for _ in 0..24 {
        let mut ops = encode(quality);
        ops.extend(resize.iter().cloned());
        if scale < 1.0 {
            ops.extend(["-resize".to_string(), format!("{}%", scale * 100.0)]);
        }
        write_as(
            ctx,
            &src,
            &src_ext,
            out_ext,
            &os(&ops),
            false,
            &staged,
            &work,
        )?;
        if std::fs::metadata(staged.path())?.len() <= target {
            return Ok(staged.commit()?.into());
        }
        // Lower the quality first, then shrink; the shrink is always from the
        // starting size, so rounding does not compound and distort the shape.
        if lossy && quality > 15 {
            quality = (quality * 72 / 100).max(15);
        } else {
            scale *= 0.8;
        }
    }
    Err(super::target_unreachable())
}

/// Single-input image tools.
pub fn tool(ctx: &Ctx, tool: &str, input: &Input) -> JobResult<Produced> {
    let o = ctx.opts;
    match tool {
        "compress" => compress(ctx, input),
        "cropImage" => {
            let r = o
                .rect
                .as_ref()
                .ok_or_else(|| JobError::invalid_options("cropImage needs a rect option"))?;
            let geo = format!(
                "{}x{}+{}+{}",
                r.w.round(),
                r.h.round(),
                r.x.round(),
                r.y.round()
            );
            process(
                ctx,
                input,
                "-cropped",
                os(&["-auto-orient".into(), "-crop".into(), geo, "+repage".into()]),
            )
        }
        "editImage" => {
            let mut ops: Vec<String> = vec!["-auto-orient".into()];
            if let Some(r) = o.rotate.filter(|r| *r != 0.0) {
                ops.extend([
                    "-background".into(),
                    "white".into(),
                    "-rotate".into(),
                    r.to_string(),
                ]);
            }
            match o.flip.as_deref() {
                Some("horizontal") => ops.push("-flop".into()),
                Some("vertical") => ops.push("-flip".into()),
                _ => {}
            }
            if let Some(rs) = &o.resize {
                if rs.w.is_some() || rs.h.is_some() {
                    let g = format!(
                        "{}x{}",
                        rs.w.map(|v| v.to_string()).unwrap_or_default(),
                        rs.h.map(|v| v.to_string()).unwrap_or_default()
                    );
                    let g = if rs.w.is_some() && rs.h.is_some() {
                        g + "!"
                    } else {
                        g
                    };
                    ops.extend(["-resize".into(), g]);
                }
            }
            if ops.len() == 1 {
                return Err(JobError::invalid_options(
                    "editImage needs rotate, flip or resize",
                ));
            }
            process(ctx, input, "-edited", os(&ops))
        }
        "frameImage" => {
            let b = o.border.clone().unwrap_or_default();
            process(
                ctx,
                input,
                "-framed",
                os(&[
                    "-auto-orient".into(),
                    "-bordercolor".into(),
                    b.color,
                    "-border".into(),
                    b.px.to_string(),
                ]),
            )
        }
        "redactImage" => redact_image(ctx, input),
        "annotateImage" => annotate_image(ctx, input),
        "readQRCode" => read_qr(ctx, input),
        _ => Err(JobError::unsupported(format!(
            "{tool} is not an image tool"
        ))),
    }
}

fn redact_image(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    use crate::options::RedactStyle;

    let rects = ctx
        .opts
        .rects
        .as_ref()
        .filter(|r| !r.is_empty())
        .ok_or_else(|| JobError::invalid_options("redactImage needs rects"))?;
    let styles: Vec<RedactStyle> = rects
        .iter()
        .map(|r| r.redact_style())
        .collect::<JobResult<_>>()?;
    let needs_comp = styles.iter().any(|s| *s != RedactStyle::Black);

    ctx.progress(None, "Redacting");
    let ext = input.ext();
    let out_ext = if ext == "svg" { "png" } else { ext };
    let staged = input.stage("-redacted", out_ext)?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let (src, src_ext) = readable_source(ctx, input, &work)?;
    let dims = if needs_comp {
        oriented_size_in_work(ctx.deps, &work, &src, &src_ext)
    } else {
        None
    };

    let mut ops: Vec<String> = vec!["-auto-orient".into(), "-gravity".into(), "NorthWest".into()];
    for (r, style) in rects.iter().zip(styles) {
        match style {
            RedactStyle::Black => {
                ops.extend([
                    "-fill".into(),
                    "black".into(),
                    "-draw".into(),
                    format!(
                        "rectangle {},{} {},{}",
                        r.x.round(),
                        r.y.round(),
                        (r.x + r.w).round(),
                        (r.y + r.h).round()
                    ),
                ]);
            }
            RedactStyle::Blur | RedactStyle::Pixelate => {
                let x = r.x.round() as u32;
                let y = r.y.round() as u32;
                let mut cw = (r.w.round() as u32).max(1);
                let mut ch = (r.h.round() as u32).max(1);
                if let Some((iw, ih)) = dims {
                    if x >= iw || y >= ih {
                        continue;
                    }
                    cw = cw.min(iw - x).max(1);
                    ch = ch.min(ih - y).max(1);
                }
                ops.extend([
                    "(".into(),
                    "+clone".into(),
                    "-crop".into(),
                    format!("{cw}x{ch}+{x}+{y}"),
                    "+repage".into(),
                ]);
                if style == RedactStyle::Blur {
                    ops.extend(["-blur".into(), "0x18".into()]);
                } else {
                    let pw = (cw / 12).max(1);
                    let ph = (ch / 12).max(1);
                    ops.extend([
                        "-scale".into(),
                        format!("{pw}x{ph}!"),
                        "-sample".into(),
                        format!("{cw}x{ch}!"),
                    ]);
                }
                ops.extend([
                    ")".into(),
                    "-geometry".into(),
                    format!("+{x}+{y}"),
                    "-composite".into(),
                    "+geometry".into(),
                ]);
            }
        }
    }
    write_as(
        ctx,
        &src,
        &src_ext,
        out_ext,
        &os(&ops),
        needs_comp,
        &staged,
        &work,
    )?;
    Ok(staged.commit()?.into())
}

fn annotate_image(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    use std::os::unix::fs::MetadataExt;

    let ov_str = ctx
        .opts
        .overlay
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| JobError::invalid_options("annotateImage needs an overlay PNG path"))?;
    let ov_path = Path::new(ov_str);
    if !ov_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        return Err(JobError::invalid_options("overlay must be a .png file"));
    }
    let ov_meta = std::fs::symlink_metadata(ov_path)
        .map_err(|e| JobError::invalid_options(format!("overlay: {e}")))?;
    if !ov_meta.file_type().is_file() {
        return Err(JobError::invalid_options(
            "overlay must be an existing regular file",
        ));
    }
    let in_meta = std::fs::metadata(&input.path)
        .map_err(|e| JobError::invalid_input(format!("{}: {e}", input.path.display())))?;
    let canon_in = std::fs::canonicalize(&input.path)
        .map_err(|e| JobError::invalid_input(format!("{}: {e}", input.path.display())))?;
    let canon_ov = std::fs::canonicalize(ov_path)
        .map_err(|e| JobError::invalid_options(format!("overlay: {e}")))?;
    if canon_in == canon_ov || (in_meta.dev() == ov_meta.dev() && in_meta.ino() == ov_meta.ino()) {
        return Err(JobError::invalid_options(
            "overlay must not be the same file as the input",
        ));
    }
    let (ov_w, ov_h) = crate::detect::png_dimensions(ov_path)
        .ok_or_else(|| JobError::invalid_options("overlay is not a valid PNG file"))?;

    ctx.progress(None, "Annotating");
    let ext = input.ext();
    let out_ext = if ext == "svg" { "png" } else { ext };
    let staged = input.stage("-annotated", out_ext)?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let (src, src_ext) = readable_source(ctx, input, &work)?;
    let (w, h) = oriented_size_in_work(ctx.deps, &work, &src, &src_ext)
        .ok_or_else(|| JobError::tool_failed("could not determine image dimensions"))?;
    let ov_spec = ispec(&work, ov_path, "png", true)?;
    let mut ops: Vec<OsString> = vec!["-auto-orient".into()];
    if (ov_w, ov_h) != (w, h) {
        ops.push("(".into());
        ops.push(ov_spec);
        ops.push("-resize".into());
        ops.push(format!("{w}x{h}!").into());
        ops.push(")".into());
    } else {
        ops.push(ov_spec);
    }
    ops.extend([
        "-gravity".into(),
        "NorthWest".into(),
        "-compose".into(),
        "Over".into(),
        "-composite".into(),
        "+geometry".into(),
    ]);
    write_as(ctx, &src, &src_ext, out_ext, &ops, true, &staged, &work)?;
    Ok(staged.commit()?.into())
}

/// Strip metadata (exiftool, else `magick -strip`).
pub fn remove_metadata(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    ctx.progress(None, "Removing metadata");
    if let Some(exif) = ctx.deps.path("exiftool") {
        let staged = input.stage("-clean", input.ext())?;
        let work = WorkDir::new_in(&input.out_dir)?;
        let out = work.join(&format!("clean.{}", input.ext()));
        Cmd::new(exif)
            .args(["-q", "-q", "-all=", "--icc_profile:all", "-o"])
            .arg(&out)
            .arg(&input.path)
            .run_ok()?;
        staged.take_from(&out)?;
        return Ok(staged.commit()?.into());
    }
    process(ctx, input, "-clean", vec!["-strip".into()])
}

/// All images into one PDF (`images.pdf`).
pub fn create_pdf(ctx: &Ctx, inputs: &[Input], out_dir: &Path) -> JobResult<PathBuf> {
    ctx.progress(None, "Creating PDF");
    let staged = Staged::file(out_dir, "images", "pdf")?;
    let work = WorkDir::new_in(out_dir)?;
    let mut cmd = magick(ctx.deps, &work)?;
    for i in inputs {
        let (src, ext) = readable_source(ctx, i, &work)?;
        cmd = cmd.arg(ispec(&work, &src, &ext, true)?);
    }
    cmd.args([
        "-auto-orient",
        "-background",
        "white",
        "-alpha",
        "remove",
        "-alpha",
        "off",
    ])
    .arg(ospec("pdf", "out.pdf"))
    .run_ok()?;
    staged.take_from(&work.join("out.pdf"))?;
    Ok(staged.commit()?)
}

/// Grid collage (`collage.png`).
pub fn collage(ctx: &Ctx, inputs: &[Input], out_dir: &Path) -> JobResult<PathBuf> {
    ctx.progress(None, "Creating collage");
    let staged = Staged::file(out_dir, "collage", "png")?;
    let work = WorkDir::new_in(out_dir)?;
    let cols = (inputs.len() as f64).sqrt().ceil().max(1.0) as usize;
    let mut cmd = montage(ctx.deps, &work)?;
    for i in inputs {
        let (src, ext) = readable_source(ctx, i, &work)?;
        cmd = cmd.arg(ispec(&work, &src, &ext, true)?);
    }
    cmd.args([
        "-auto-orient",
        "-tile",
        &format!("{cols}x"),
        "-geometry",
        "600x600>+8+8",
        "-background",
        "white",
    ])
    .arg(ospec("png", "out.png"))
    .run_ok()?;
    staged.take_from(&work.join("out.png"))?;
    Ok(staged.commit()?)
}

/// Decode QR/barcodes with zbarimg into `stem-qr.txt`.
fn read_qr(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    ctx.progress(None, "Reading QR codes");
    let zbar = require(ctx.deps, "zbarimg")?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let (src, ext) = readable_source(ctx, input, &work)?;
    // zbarimg decodes through ImageMagick: same confinement and coder prefix.
    let out = im_cmd(zbar, &work)?
        .args(["--quiet", "--raw"])
        .arg(ispec(&work, &src, &ext, true)?)
        .run()?;
    match out.status.code() {
        Some(0) => {}
        Some(4) => return Err(JobError::invalid_input("no QR code or barcode found")),
        _ => return Err(failure("zbarimg", &out)),
    }
    let text = out.stdout.trim_end().to_string();
    let staged = input.stage("-qr", "txt")?;
    std::fs::write(staged.path(), format!("{text}\n"))?;
    let path = staged.commit()?;
    Ok(Produced {
        paths: vec![path],
        message: Some(text),
    })
}

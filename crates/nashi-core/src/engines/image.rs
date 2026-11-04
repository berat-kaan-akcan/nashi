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

/// ImageMagick operations for picture adjustments. Each is limited to the colour
/// channels, since on alpha they would reveal transparent areas or fade opaque ones.
pub fn adjust_ops(a: &crate::options::Adjust) -> JobResult<Vec<String>> {
    let checked = |value: Option<f64>, name: &str, low: f64, high: f64, neutral: f64| match value {
        Some(v) if !v.is_finite() || v < low || v > high => Err(JobError::invalid_options(
            format!("{name} must be between {low} and {high}"),
        )),
        Some(v) if v != neutral => Ok(Some(v)),
        _ => Ok(None),
    };
    let mut colour: Vec<String> = Vec::new();
    let mut push = |args: &[String]| colour.extend_from_slice(args);
    if let Some(stops) = checked(a.exposure, "exposure", -5.0, 5.0, 0.0)? {
        push(&[
            "-evaluate".into(),
            "Multiply".into(),
            2f64.powf(stops).to_string(),
        ]);
    }
    let brightness = checked(a.brightness, "brightness", -100.0, 100.0, 0.0)?;
    let contrast = checked(a.contrast, "contrast", -100.0, 100.0, 0.0)?;
    if brightness.is_some() || contrast.is_some() {
        push(&[
            "-brightness-contrast".into(),
            format!("{}x{}", brightness.unwrap_or(0.0), contrast.unwrap_or(0.0)),
        ]);
    }
    if let Some(g) = checked(a.gamma, "gamma", 0.1, 5.0, 1.0)? {
        push(&["-gamma".into(), g.to_string()]);
    }
    let saturation = checked(a.saturation, "saturation", -100.0, 100.0, 0.0)?;
    let hue = checked(a.hue, "hue", -100.0, 100.0, 0.0)?;
    if saturation.is_some() || hue.is_some() {
        push(&[
            "-modulate".into(),
            format!(
                "100,{},{}",
                100.0 + saturation.unwrap_or(0.0),
                100.0 + hue.unwrap_or(0.0)
            ),
        ]);
    }
    if let Some(amount) = checked(a.sharpen, "sharpen", 0.0, 100.0, 0.0)? {
        push(&["-sharpen".into(), format!("0x{}", amount / 25.0)]);
    }
    if let Some(amount) = checked(a.blur, "blur", 0.0, 100.0, 0.0)? {
        push(&["-blur".into(), format!("0x{}", amount / 5.0)]);
    }
    if let Some(amount) = checked(a.grain, "grain", 0.0, 100.0, 0.0)? {
        push(&[
            "-attenuate".into(),
            (amount / 100.0).to_string(),
            "+noise".into(),
            "Gaussian".into(),
        ]);
    }
    match a.filter.as_deref() {
        None | Some("") => {}
        // Gray pixels, not a gray colorspace: the output format may not offer one.
        Some("grayscale") => push(&["-modulate".into(), "100,0,100".into()]),
        // A fixed matrix: -sepia-tone normalizes the result, so its look depends on the image.
        Some("sepia") => push(&[
            "-color-matrix".into(),
            "0.393 0.769 0.189 0.349 0.686 0.168 0.272 0.534 0.131".into(),
        ]),
        Some(other) => {
            return Err(JobError::invalid_options(format!(
                "unknown filter '{other}'"
            )))
        }
    }
    let mut ops = Vec::new();
    if !colour.is_empty() {
        ops.extend(["-channel".to_string(), "RGB".to_string()]);
        ops.append(&mut colour);
        ops.push("+channel".to_string());
    }
    if let Some(warmth) = checked(a.temperature, "temperature", -100.0, 100.0, 0.0)? {
        let shift = warmth / 100.0 * 0.3;
        for (channel, factor) in [("R", 1.0 + shift), ("B", 1.0 - shift)] {
            ops.extend([
                "-channel".to_string(),
                channel.to_string(),
                "-evaluate".to_string(),
                "Multiply".to_string(),
                factor.to_string(),
            ]);
        }
        ops.push("+channel".to_string());
    }
    Ok(ops)
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
            if let Some(adjust) = &o.adjust {
                ops.extend(adjust_ops(adjust)?);
            }
            if ops.len() == 1 {
                return Err(JobError::invalid_options(
                    "editImage needs rotate, flip, resize or an adjustment",
                ));
            }
            process(ctx, input, "-edited", os(&ops))
        }
        "frameImage" if o.frame.is_some() => frame_image(ctx, input),
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
/// Largest canvas a layout may ask for.
const MAX_CANVAS_SIDE: f64 = 32000.0;
const MAX_CANVAS_AREA: f64 = 100_000_000.0;

fn checked_canvas(w: f64, h: f64) -> JobResult<(u32, u32)> {
    if w < 1.0 || h < 1.0 || w > MAX_CANVAS_SIDE || h > MAX_CANVAS_SIDE || w * h > MAX_CANVAS_AREA {
        return Err(JobError::invalid_options(
            "the canvas would be too large or too small",
        ));
    }
    Ok((w.round() as u32, h.round() as u32))
}

fn checked_color(color: Option<&str>, default: &str) -> JobResult<String> {
    let color = color.unwrap_or(default);
    if crate::options::valid_color(color) {
        Ok(color.to_string())
    } else {
        Err(JobError::invalid_options(format!(
            "'{color}' is not a colour"
        )))
    }
}

/// Operations that cut an image of this size to rounded corners, keeping its alpha.
fn rounded_ops(w: u32, h: u32, radius: f64) -> Vec<String> {
    if radius <= 0.0 {
        return Vec::new();
    }
    let r = radius.min(f64::from(w.min(h)) / 2.0);
    [
        "(",
        "-size",
        &format!("{w}x{h}"),
        "xc:none",
        "-fill",
        "white",
        "-draw",
        &format!("roundrectangle 0,0,{},{},{r},{r}", w - 1, h - 1),
        ")",
        "-alpha",
        "set",
        "-compose",
        "DstIn",
        "-composite",
        "-compose",
        "Over",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect()
}

fn frame_image(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    let f = ctx.opts.frame.clone().unwrap_or_default();
    let d = ctx.deps;
    let unit = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
    let scale = f.image_scale.unwrap_or(1.0);
    if !scale.is_finite() || scale <= 0.0 || scale > 1.0 {
        return Err(JobError::invalid_options(
            "image_scale must be greater than 0 and at most 1",
        ));
    }
    let (px, py) = f.position.as_ref().map_or((0.5, 0.5), |p| (p.x, p.y));
    if !unit(px) || !unit(py) {
        return Err(JobError::invalid_options(
            "position must be between 0 and 1",
        ));
    }
    if let Some(a) = f.aspect {
        if !a.is_finite() || !(0.1..=10.0).contains(&a) {
            return Err(JobError::invalid_options(
                "aspect must be between 0.1 and 10",
            ));
        }
    }
    let blur = f.background_blur.unwrap_or(0.0);
    let shadow = f.shadow.unwrap_or(0.0);
    let opacity = f.shadow_opacity.unwrap_or(30.0);
    let radius = f.corner_radius.unwrap_or(0.0);
    if !blur.is_finite() || !(0.0..=100.0).contains(&blur) {
        return Err(JobError::invalid_options(
            "background_blur must be between 0 and 100",
        ));
    }
    if !shadow.is_finite() || !(0.0..=200.0).contains(&shadow) {
        return Err(JobError::invalid_options(
            "shadow must be between 0 and 200",
        ));
    }
    if !opacity.is_finite() || !(0.0..=100.0).contains(&opacity) {
        return Err(JobError::invalid_options(
            "shadow_opacity must be between 0 and 100",
        ));
    }
    if !radius.is_finite() || radius < 0.0 {
        return Err(JobError::invalid_options(
            "corner_radius must not be negative",
        ));
    }
    ctx.progress(None, "Framing");
    let ext = input.ext();
    let out_ext = if ext == "svg" { "png" } else { ext };
    let work = WorkDir::new_in(&input.out_dir)?;
    let (src, src_ext) = readable_source(ctx, input, &work)?;
    let (w, h) = oriented_size_in_work(d, &work, &src, &src_ext)
        .ok_or_else(|| JobError::invalid_input("could not read the image size"))?;

    let padding = f64::from(f.padding.unwrap_or(40));
    let mut cw = f64::from(w) + 2.0 * padding;
    let mut ch = f64::from(h) + 2.0 * padding;
    if let Some(a) = f.aspect {
        if cw / ch < a {
            cw = (ch * a).round();
        } else {
            ch = (cw / a).round();
        }
    }
    let (cw, ch) = checked_canvas(cw, ch)?;
    let iw = ((f64::from(w) * scale).round() as u32).max(1);
    let ih = ((f64::from(h) * scale).round() as u32).max(1);
    // Whole-pixel origin: half a pixel off the grid would blur the whole image.
    let x = (f64::from(cw - iw.min(cw)) * px).round() as u32;
    let y = (f64::from(ch - ih.min(ch)) * py).round() as u32;

    let color = checked_color(f.color.as_deref(), "#ffffff")?;
    let canvas = format!("{cw}x{ch}");
    let mut cmd = magick(d, &work)?;
    cmd = match f.backdrop.as_deref().unwrap_or("solid") {
        "solid" => cmd.args(["-size", &canvas]).arg(format!("xc:{color}")),
        "gradient" => {
            let second = checked_color(f.color2.as_deref(), "#000000")?;
            let angle = f.angle.unwrap_or(0.0);
            if !angle.is_finite() {
                return Err(JobError::invalid_options("angle must be a number"));
            }
            cmd.args(["-size", &canvas])
                .args(["-define", &format!("gradient:angle={angle}")])
                .arg(format!("gradient:{color}-{second}"))
        }
        "image" => {
            let path = f.background_image.as_deref().ok_or_else(|| {
                JobError::invalid_options("backdrop image needs background_image")
            })?;
            let meta = std::fs::metadata(path).ok().filter(|m| m.is_file());
            if meta.is_none() {
                return Err(JobError::invalid_options(
                    "the background image could not be read",
                ));
            }
            let det = crate::detect::detect(path);
            if det.category != crate::catalog::Category::Image {
                return Err(JobError::invalid_options("the background must be an image"));
            }
            let (back, back_ext) = decode_source(d, path, &det.ext, &work)?;
            let mut c = cmd
                .arg("(")
                .arg(ispec(&work, &back, &back_ext, true)?)
                .args(["-auto-orient", "-resize", &format!("{canvas}^")])
                .args(["-gravity", "center", "-extent", &canvas, "+gravity"]);
            if blur > 0.0 {
                c = c.args(["-blur", &format!("0x{}", blur / 2.0)]);
            }
            c.args(["+repage", ")"])
        }
        other => {
            return Err(JobError::invalid_options(format!(
                "unknown backdrop '{other}'"
            )))
        }
    };
    // Kept under a name so the shadow and the image come from the same pixels.
    cmd = cmd
        .arg("(")
        .arg(ispec(&work, &src, &src_ext, true)?)
        .args(["-auto-orient", "-resize", &format!("{iw}x{ih}!")])
        .args(rounded_ops(iw, ih, radius))
        .args(["-write", "mpr:nashi", "+delete", ")"]);
    if shadow > 0.0 && opacity > 0.0 {
        cmd = cmd.args([
            "(",
            "mpr:nashi",
            "-background",
            "black",
            "-shadow",
            &format!("{opacity}x{shadow}+0+{}", (shadow / 2.0).round()),
            "-repage",
            &format!("+{x}+{y}!"),
            ")",
        ]);
    }
    let name = format!("out.{out_ext}");
    cmd.args(["(", "mpr:nashi", "-repage", &format!("+{x}+{y}"), ")"])
        .args(["-background", "none", "-flatten"])
        .arg(ospec(out_ext, &name))
        .run_ok()?;
    let staged = input.stage("-framed", out_ext)?;
    staged.take_from(&work.join(&name))?;
    Ok(staged.commit()?.into())
}

/// Canvas size, then `[x, y, w, h]` per cell.
type CollageCells = ((u32, u32), Vec<[u32; 4]>);

/// Where each cell of a collage goes.
fn collage_cells(c: &crate::options::Collage, n: usize) -> JobResult<CollageCells> {
    let aspect = c.aspect.unwrap_or(1.0);
    if !aspect.is_finite() || !(0.1..=10.0).contains(&aspect) {
        return Err(JobError::invalid_options(
            "aspect must be between 0.1 and 10",
        ));
    }
    let width = f64::from(c.width.unwrap_or(2048));
    let gap = f64::from(c.spacing.unwrap_or(0));
    let pad = f64::from(c.padding.unwrap_or(0));
    let cells_from = |cols: usize, rows: usize| -> JobResult<CollageCells> {
        let cell_w = (width - 2.0 * pad - gap * (cols as f64 - 1.0)) / cols as f64;
        if cell_w < 1.0 {
            return Err(JobError::invalid_options(
                "the spacing and padding leave no room for the images",
            ));
        }
        let cell_h = (cell_w / aspect).round().max(1.0);
        let height = 2.0 * pad + rows as f64 * cell_h + gap * (rows as f64 - 1.0);
        let canvas = checked_canvas(width, height)?;
        let cells = (0..n)
            .map(|i| {
                let (col, row) = ((i % cols) as f64, (i / cols) as f64);
                let x = (pad + col * (cell_w + gap)).round();
                let next = (pad + (col + 1.0) * (cell_w + gap) - gap).round();
                [
                    x as u32,
                    (pad + row * (cell_h + gap)).round() as u32,
                    (next - x) as u32,
                    cell_h as u32,
                ]
            })
            .collect();
        Ok((canvas, cells))
    };
    match c.layout.as_deref().unwrap_or("grid") {
        "grid" => {
            let cols = match c.columns {
                Some(0) => return Err(JobError::invalid_options("columns must be at least 1")),
                Some(cols) => (cols as usize).min(n),
                None => (n as f64).sqrt().ceil() as usize,
            };
            cells_from(cols, n.div_ceil(cols))
        }
        "row" => cells_from(n, 1),
        "column" => cells_from(1, n),
        "featured" if n < 2 => cells_from(1, 1),
        "featured" => {
            // The first image takes two thirds of the width, the others stack beside it.
            let inner = width - 2.0 * pad - gap;
            let big_w = (inner * 2.0 / 3.0).round();
            let small_w = inner - big_w;
            let big_h = (big_w / aspect).round();
            let rest = n as f64 - 1.0;
            let small_h = ((big_h - gap * (rest - 1.0)) / rest).floor();
            if big_w < 1.0 || small_w < 1.0 || small_h < 1.0 {
                return Err(JobError::invalid_options(
                    "the spacing and padding leave no room for the images",
                ));
            }
            let canvas = checked_canvas(width, 2.0 * pad + big_h)?;
            let mut cells = vec![[pad as u32, pad as u32, big_w as u32, big_h as u32]];
            for i in 0..n - 1 {
                cells.push([
                    (pad + big_w + gap) as u32,
                    (pad + i as f64 * (small_h + gap)) as u32,
                    small_w as u32,
                    small_h as u32,
                ]);
            }
            Ok((canvas, cells))
        }
        other => Err(JobError::invalid_options(format!(
            "unknown layout '{other}'"
        ))),
    }
}

fn laid_out_collage(
    ctx: &Ctx,
    inputs: &[Input],
    out_dir: &Path,
    c: &crate::options::Collage,
) -> JobResult<PathBuf> {
    let fill = match c.fit.as_deref().unwrap_or("fill") {
        "fill" => true,
        "fit" => false,
        other => return Err(JobError::invalid_options(format!("unknown fit '{other}'"))),
    };
    let radius = c.corner_radius.unwrap_or(0.0);
    if !radius.is_finite() || radius < 0.0 {
        return Err(JobError::invalid_options(
            "corner_radius must not be negative",
        ));
    }
    let n = inputs.len();
    let order: Vec<usize> = match &c.order {
        Some(order) => {
            let mut sorted = order.clone();
            sorted.sort_unstable();
            if sorted != (0..n).collect::<Vec<_>>() {
                return Err(JobError::invalid_options("order must name each image once"));
            }
            order.clone()
        }
        None => (0..n).collect(),
    };
    let color = checked_color(c.color.as_deref(), "#ffffff")?;
    let ((cw, ch), cells) = collage_cells(c, n)?;
    ctx.progress(None, "Creating collage");
    let staged = Staged::file(out_dir, "collage", "png")?;
    let work = WorkDir::new_in(out_dir)?;
    let mut cmd = magick(ctx.deps, &work)?
        .args(["-size", &format!("{cw}x{ch}")])
        .arg(format!("xc:{color}"));
    for (slot, index) in order.iter().enumerate() {
        let [x, y, w, h] = cells[slot];
        let (src, ext) = readable_source(ctx, &inputs[*index], &work)?;
        let geometry = format!("{w}x{h}");
        cmd = cmd
            .arg("(")
            .arg(ispec(&work, &src, &ext, true)?)
            .arg("-auto-orient");
        cmd = if fill {
            cmd.args(["-resize", &format!("{geometry}^")])
        } else {
            cmd.args(["-resize", &geometry, "-background", "none"])
        };
        cmd = cmd
            .args(["-gravity", "center", "-extent", &geometry, "+gravity"])
            .args(rounded_ops(w, h, radius))
            .args(["-repage", &format!("+{x}+{y}"), ")"]);
    }
    cmd.args(["-background", "none", "-flatten"])
        .arg(ospec("png", "out.png"))
        .run_ok()?;
    staged.take_from(&work.join("out.png"))?;
    Ok(staged.commit()?)
}

pub fn collage(ctx: &Ctx, inputs: &[Input], out_dir: &Path) -> JobResult<PathBuf> {
    if let Some(layout) = &ctx.opts.collage {
        return laid_out_collage(ctx, inputs, out_dir, layout);
    }
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

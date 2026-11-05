//! PDF: poppler (pdftoppm/pdftotext/pdfunite/pdfseparate), qpdf, Ghostscript,
//! LibreOffice for PDF → docx/pptx.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{office, Ctx, Input, Produced};
use crate::deps::require;
use crate::detect::pdf_pages;
use crate::events::{JobError, JobResult};
use crate::fsops::{files_in, Staged, WorkDir};
use crate::options::parse_pages;
use crate::process::Cmd;

/// Render a PDF to images: one page → `stem.ext`, several → `stem-pages/`.
pub fn to_images(
    ctx: &Ctx,
    pdf: &Path,
    stem: &str,
    out_dir: &Path,
    fmt: &str,
) -> JobResult<Produced> {
    ctx.progress(None, "Rendering pages");
    let ppm = require(ctx.deps, "pdftoppm")?;
    let pages = pdf_pages(ctx.deps, pdf).unwrap_or(2);
    let fmt_args: &[&str] = if fmt == "jpg" {
        &["-jpeg", "-jpegopt", "quality=90"]
    } else {
        &["-png"]
    };
    let dpi = ctx.opts.dpi.unwrap_or(150);
    if !(36..=1200).contains(&dpi) {
        return Err(JobError::invalid_options("dpi must be between 36 and 1200"));
    }
    // Sizes need pdfinfo, which this route can do without.
    let mut base = Cmd::new(ppm)
        .arg("-cropbox")
        .args(fmt_args)
        .arg("-r")
        .arg(dpi.to_string());
    if ctx.deps.has("pdfinfo") {
        let sizes = shown_page_sizes(ctx, pdf)?;
        check_render_size(&sizes, dpi)?;
        base = base.args(["-f", "1", "-l", &sizes.len().to_string()]);
    }
    if pages <= 1 {
        let staged = Staged::file(out_dir, stem, fmt)?;
        let work = WorkDir::new_in(out_dir)?;
        let prefix = work.join("page");
        base.arg("-singlefile").arg(pdf).arg(&prefix).run_ok()?;
        let produced = work.join(&format!("page.{fmt}"));
        staged.take_from(&produced)?;
        return Ok(staged.commit()?.into());
    }
    let staged = Staged::dir(out_dir, &format!("{stem}-pages"))?;
    base.arg(pdf).arg(staged.path().join(stem)).run_ok()?;
    if files_in(staged.path())?.is_empty() {
        return Err(JobError::tool_failed("pdftoppm produced no pages"));
    }
    Ok(staged.commit()?.into())
}

fn pdftotext(ctx: &Ctx, pdf: &Path, out: &Path) -> JobResult<()> {
    let p = require(ctx.deps, "pdftotext")?;
    Cmd::new(p)
        .args(["-enc", "UTF-8"])
        .arg(pdf)
        .arg(out)
        .run_ok()?;
    // pdftotext succeeds on a scan, leaving only form feeds.
    if String::from_utf8_lossy(&std::fs::read(out)?)
        .trim()
        .is_empty()
    {
        return Err(JobError::invalid_input(
            "This PDF has no text to extract (it may be a scan)",
        ));
    }
    Ok(())
}

/// Limits for keeping pages as pictures: the pixels of one page and of the whole
/// file, checked before rendering, and the JPEGs that are then held in memory.
const MAX_PAGE_PIXELS: f64 = 100_000_000.0;
const MAX_DOCUMENT_PIXELS: f64 = 20_000_000_000.0;
const MAX_PAGE_IMAGES_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn too_large_to_render() -> JobError {
    JobError::invalid_input("This PDF is too large to turn into pictures. Try a lower resolution.")
}

fn check_render_size(sizes: &[(f64, f64)], dpi: u32) -> JobResult<()> {
    let scale = f64::from(dpi) / 72.0;
    let pixels = |(w, h): &(f64, f64)| w * scale * h * scale;
    let total: f64 = sizes.iter().map(pixels).sum();
    // Written so that a size that is not a number fails too.
    let fits = |value: f64, limit: f64| value > 0.0 && value <= limit;
    if !fits(total, MAX_DOCUMENT_PIXELS) || !sizes.iter().all(|s| fits(pixels(s), MAX_PAGE_PIXELS))
    {
        return Err(too_large_to_render());
    }
    Ok(())
}

/// Render each page as a JPEG, with the size it is shown at.
fn rendered_pages(ctx: &Ctx, pdf: &Path, work: &WorkDir) -> JobResult<Vec<super::pagedoc::Page>> {
    ctx.progress(None, "Rendering pages");
    let ppm = require(ctx.deps, "pdftoppm")?;
    let sizes = shown_page_sizes(ctx, pdf)?;
    let dpi = ctx.opts.dpi.unwrap_or(150);
    if !(36..=1200).contains(&dpi) {
        return Err(JobError::invalid_options("dpi must be between 36 and 1200"));
    }
    check_render_size(&sizes, dpi)?;
    // Render what pdfinfo measured: the visible box of the measured pages.
    Cmd::new(ppm)
        .args(["-cropbox", "-jpeg", "-jpegopt", "quality=90", "-r"])
        .arg(dpi.to_string())
        .args(["-f", "1", "-l", &sizes.len().to_string()])
        .arg(pdf)
        .arg(work.join("page"))
        .run_ok()?;
    let number = |p: &PathBuf| -> Option<u32> {
        p.file_stem()?.to_str()?.strip_prefix("page-")?.parse().ok()
    };
    let mut files: Vec<PathBuf> = files_in(work.path())?
        .into_iter()
        .filter(|p| number(p).is_some())
        .collect();
    files.sort_by_key(|p| number(p));
    if files.len() != sizes.len() {
        return Err(JobError::tool_failed("pdftoppm did not render every page"));
    }
    let mut total = 0u64;
    for file in &files {
        total = total.saturating_add(std::fs::metadata(file)?.len());
    }
    if total > MAX_PAGE_IMAGES_BYTES {
        return Err(too_large_to_render());
    }
    files
        .iter()
        .zip(sizes)
        .map(|(file, (width_pt, height_pt))| {
            crate::process::check_cancel()?;
            Ok(super::pagedoc::Page {
                jpeg: std::fs::read(file)?,
                width_pt,
                height_pt,
            })
        })
        .collect()
}

/// Convert a PDF.
pub fn convert(ctx: &Ctx, input: &Input, target: &str) -> JobResult<Produced> {
    let layout = ctx.opts.pdf_layout.as_deref();
    if layout.is_some_and(|l| l != "editable" && l != "pages") {
        return Err(JobError::invalid_options(
            "pdf_layout must be editable or pages",
        ));
    }
    if layout == Some("pages") && matches!(target, "epub" | "docx") {
        let staged = input.stage("", target)?;
        let work = WorkDir::new_in(&input.out_dir)?;
        let pages = rendered_pages(ctx, &input.path, &work)?;
        if target == "epub" {
            super::pagedoc::write_epub(staged.path(), input.stem(), &pages)?;
        } else {
            super::pagedoc::write_docx(staged.path(), &pages)?;
        }
        return Ok(staged.commit()?.into());
    }
    match target {
        "jpg" | "png" => to_images(ctx, &input.path, input.stem(), &input.out_dir, target),
        "txt" => {
            ctx.progress(None, "Extracting text");
            let staged = input.stage("", "txt")?;
            pdftotext(ctx, &input.path, staged.path())?;
            Ok(staged.commit_allow_empty()?.into())
        }
        "md" | "epub" => {
            ctx.progress(None, "Extracting text");
            let staged = input.stage("", target)?;
            let work = WorkDir::new_in(&input.out_dir)?;
            let txt = work.join("text.txt");
            pdftotext(ctx, &input.path, &txt)?;
            let text = String::from_utf8_lossy(&std::fs::read(&txt)?).into_owned();
            if target == "md" {
                std::fs::write(staged.path(), text_to_markdown(&text))?;
            } else {
                let body = super::epub::text_to_xhtml_body(&text);
                super::epub::write_epub(
                    staged.path(),
                    input.stem(),
                    &body,
                    &super::epub::Language::default(),
                )?;
            }
            Ok(staged.commit_allow_empty()?.into())
        }
        "docx" => office::soffice_to_output(
            ctx,
            input,
            &input.path,
            "docx:MS Word 2007 XML",
            Some("writer_pdf_import"),
            "docx",
        ),
        "pptx" => office::soffice_to_output(
            ctx,
            input,
            &input.path,
            "pptx:Impress MS PowerPoint 2007 XML",
            Some("impress_pdf_import"),
            "pptx",
        ),
        _ => Err(JobError::unsupported(format!(
            "PDF to .{target} is not supported"
        ))),
    }
}

/// pdftotext output as Markdown: form feeds become horizontal rules.
fn text_to_markdown(text: &str) -> String {
    let mut out = String::new();
    for (i, page) in text.split('\u{c}').enumerate() {
        let page = page.trim_matches('\n');
        if page.trim().is_empty() {
            continue;
        }
        if i > 0 && !out.is_empty() {
            out.push_str("\n\n---\n\n");
        }
        for line in page.lines() {
            // Escape characters that would start Markdown syntax.
            let t = line.trim_end();
            if t.starts_with('#')
                || t.starts_with('>')
                || t.starts_with("- ")
                || t.starts_with("* ")
            {
                out.push('\\');
            }
            out.push_str(t);
            out.push('\n');
        }
    }
    out
}

fn gs_base(ctx: &Ctx) -> JobResult<Cmd> {
    Ok(Cmd::new(require(ctx.deps, "gs")?).args([
        "-dSAFER",
        "-dBATCH",
        "-dNOPAUSE",
        "-dQUIET",
        "-sDEVICE=pdfwrite",
    ]))
}

/// `-sOutputFile=` with `%` escaped (Ghostscript treats it as a page pattern).
fn gs_out(p: &Path) -> OsString {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let mut v = b"-sOutputFile=".to_vec();
    for b in p.as_os_str().as_bytes() {
        if *b == b'%' {
            v.push(b'%');
        }
        v.push(*b);
    }
    OsString::from_vec(v)
}

fn compress(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    ctx.progress(None, "Compressing");
    let original = std::fs::metadata(&input.path)?.len();
    let target = super::compress_target(ctx.opts, input)?;
    if target.is_some_and(|t| original <= t) {
        return super::keep_original(input, "pdf");
    }
    let staged = input.stage("-compressed", "pdf")?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let tmp = work.join("out.pdf");
    let rewrite = |preset: &str, resolution: Option<u32>| -> JobResult<u64> {
        let mut cmd = gs_base(ctx)?
            .arg(format!("-dPDFSETTINGS=/{preset}"))
            .arg("-dCompatibilityLevel=1.5");
        if let Some(dpi) = resolution {
            for kind in ["Color", "Gray", "Mono"] {
                cmd = cmd.args([
                    format!("-dDownsample{kind}Images=true"),
                    format!("-d{kind}ImageDownsampleThreshold=1.0"),
                    format!("-d{kind}ImageResolution={dpi}"),
                ]);
            }
        }
        cmd.arg(gs_out(&tmp)).arg("-f").arg(&input.path).run_ok()?;
        Ok(std::fs::metadata(&tmp)?.len())
    };
    if let Some(target) = target {
        // Text stays selectable at every step; only the images get coarser.
        for (preset, resolution) in [
            ("printer", None),
            ("ebook", None),
            ("screen", None),
            ("screen", Some(50)),
            ("screen", Some(36)),
        ] {
            if rewrite(preset, resolution)? <= target {
                staged.take_from(&tmp)?;
                return Ok(staged.commit()?.into());
            }
        }
        return Err(super::target_unreachable());
    }
    let preset = ctx.opts.pdf_preset.as_deref().unwrap_or("ebook");
    // Rewriting an already small PDF can make it larger.
    let kept = rewrite(preset, None)? >= original;
    if kept {
        std::fs::copy(&input.path, &tmp)?;
    }
    staged.take_from(&tmp)?;
    Ok(Produced {
        paths: vec![staged.commit()?],
        message: kept.then(|| super::ALREADY_OPTIMIZED.into()),
    })
}

/// Single-input PDF tools.
pub fn tool(ctx: &Ctx, tool: &str, input: &Input) -> JobResult<Produced> {
    match tool {
        "compress" => compress(ctx, input),
        "removeMetadata" => remove_metadata(ctx, input),
        "splitPDF" => split(ctx, input),
        "organizePDF" => organize(ctx, input),
        _ => Err(JobError::unsupported(format!("{tool} is not a PDF tool"))),
    }
}

fn remove_metadata(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    ctx.progress(None, "Removing metadata");
    let staged = input.stage("-clean", "pdf")?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let d = ctx.deps;
    let cleaned = if let Some(exif) = d.path("exiftool") {
        let tmp = work.join("clean.pdf");
        Cmd::new(exif)
            .args(["-q", "-q", "-all:all=", "-o"])
            .arg(&tmp)
            .arg(&input.path)
            .run_ok()?;
        tmp
    } else {
        let out = work.join("blank.pdf");
        gs_blank_info(ctx, &input.path, &out)?;
        out
    };
    // exiftool edits PDFs incrementally (old metadata stays recoverable);
    // rewrite the file so the removal is real.
    let Some(qpdf) = d.path("qpdf") else {
        if d.has("exiftool") && d.has("gs") {
            let out = work.join("blank.pdf");
            gs_blank_info(ctx, &cleaned, &out)?;
            staged.take_from(&out)?;
        } else {
            staged.take_from(&cleaned)?;
        }
        return Ok(Produced {
            paths: vec![staged.commit()?],
            message: Some(
                "Comment authors and dates may remain in this PDF. Install qpdf to remove them."
                    .into(),
            ),
        });
    };
    let mut rewrite = Cmd::new(qpdf).arg("--linearize");
    if let Some(update) = annotation_update(qpdf, &cleaned, &work)? {
        let mut flag = OsString::from("--update-from-json=");
        flag.push(&update);
        rewrite = rewrite.arg(flag);
    }
    let out = rewrite.arg(&cleaned).arg(staged.path()).run()?;
    // qpdf exit 3 = warnings, output still written.
    if !matches!(out.status.code(), Some(0) | Some(3)) {
        return Err(crate::process::failure("qpdf", &out));
    }
    Ok(staged.commit()?.into())
}

/// Write a qpdf update that drops annotation authors and dates. `None` when
/// the PDF has nothing to remove.
fn annotation_update(qpdf: &Path, pdf: &Path, work: &WorkDir) -> JobResult<Option<PathBuf>> {
    let json = work.join("objects.json");
    let out = Cmd::new(qpdf)
        .args([
            "--json-output=2",
            "--json-key=qpdf",
            "--json-stream-data=none",
        ])
        .arg(pdf)
        .arg(&json)
        .run()?;
    if !matches!(out.status.code(), Some(0) | Some(3)) {
        return Err(crate::process::failure("qpdf", &out));
    }
    let bad = || JobError::tool_failed("qpdf wrote unreadable PDF object data");
    let root: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&json)?).map_err(|_| bad())?;
    let parts = root["qpdf"].as_array().ok_or_else(bad)?;
    let (header, objects) = match parts.as_slice() {
        [h, o] => (h, o.as_object().ok_or_else(bad)?),
        _ => return Err(bad()),
    };
    let mut changed = serde_json::Map::new();
    for (key, object) in objects {
        if !key.starts_with("obj:") {
            continue;
        }
        let mut value = object["value"].clone();
        if scrub_annotations(&mut value) {
            changed.insert(key.clone(), serde_json::json!({ "value": value }));
        }
    }
    if changed.is_empty() {
        return Ok(None);
    }
    let path = work.join("update.json");
    let update = serde_json::json!({ "qpdf": [header, changed] });
    std::fs::write(&path, update.to_string())?;
    Ok(Some(path))
}

/// Remove authors and dates from every annotation in `value`, including
/// annotations written inline. A form field's /T is its name, so it stays.
fn scrub_annotations(value: &mut serde_json::Value) -> bool {
    let mut changed = false;
    match value {
        serde_json::Value::Object(dict) => {
            if dict.contains_key("/Rect") {
                if let Some(subtype) = dict.get("/Subtype").and_then(|s| s.as_str()) {
                    let mut keys = vec!["/M", "/CreationDate"];
                    if subtype != "/Widget" {
                        keys.push("/T");
                    }
                    for key in keys {
                        changed |= dict.remove(key).is_some();
                    }
                }
            }
            for child in dict.values_mut() {
                changed |= scrub_annotations(child);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                changed |= scrub_annotations(child);
            }
        }
        _ => {}
    }
    changed
}

fn gs_blank_info(ctx: &Ctx, src: &Path, dst: &Path) -> JobResult<()> {
    gs_base(ctx)?
        .arg(gs_out(dst))
        .arg("-f")
        .arg(src)
        .arg("-c")
        .arg("[ /Title () /Author () /Subject () /Keywords () /Creator () /Producer () /DOCINFO pdfmark")
        .run_ok()?;
    Ok(())
}

fn split(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    ctx.progress(None, "Splitting");
    let d = ctx.deps;
    let count = pdf_pages(d, &input.path);
    let pages: Vec<u32> = match &ctx.opts.pages {
        Some(spec) => parse_pages(spec, count)?,
        None => (1..=count
            .ok_or_else(|| JobError::invalid_input("could not read the page count"))?)
            .collect(),
    };
    let part_size = ctx.opts.pages_per_file.unwrap_or(1);
    if part_size == 0 {
        return Err(JobError::invalid_options(
            "pages_per_file must be at least 1",
        ));
    }
    let staged = input.stage_dir("-pages")?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let stem = input.stem();
    let parts = pages.len().div_ceil(part_size as usize);
    for (n, part) in pages.chunks(part_size as usize).enumerate() {
        crate::process::check_cancel()?;
        ctx.progress(Some(n as f64 / parts as f64), "Splitting");
        // Tools write to a fixed, %-free name; then move under the real name.
        let tmp = work.join(&format!("p{n}.pdf"));
        let name = match part {
            [only] => {
                extract_page(ctx, &input.path, *only, &tmp)?;
                format!("{stem}-{only}.pdf")
            }
            _ => {
                extract_part(ctx, &input.path, part, &tmp, &work)?;
                format!("{stem}-{}-{}.pdf", part[0], part[part.len() - 1])
            }
        };
        std::fs::rename(&tmp, staged.path().join(name))?;
    }
    Ok(staged.commit()?.into())
}

/// Several pages of `src` as one PDF.
fn extract_part(ctx: &Ctx, src: &Path, pages: &[u32], out: &Path, work: &WorkDir) -> JobResult<()> {
    if let Some(qpdf) = ctx.deps.path("qpdf") {
        let list = pages
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let r = Cmd::new(qpdf)
            .arg("--empty")
            .arg("--pages")
            .arg(src)
            .arg(&list)
            .arg("--")
            .arg(out)
            .run()?;
        if !matches!(r.status.code(), Some(0) | Some(3)) {
            return Err(crate::process::failure("qpdf", &r));
        }
        return Ok(());
    }
    let mut files = Vec::new();
    for page in pages {
        let one = work.join(&format!("one-{page}.pdf"));
        extract_page(ctx, src, *page, &one)?;
        files.push(one);
    }
    let paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    join_pdfs(ctx, &paths, out, work)
}

/// Each page's size as it is shown, in points, rotation applied.
fn shown_page_sizes(ctx: &Ctx, path: &Path) -> JobResult<Vec<(f64, f64)>> {
    let count = pdf_pages(ctx.deps, path)
        .ok_or_else(|| JobError::invalid_input("could not read the page count"))?;
    let info = Cmd::new(require(ctx.deps, "pdfinfo")?)
        .args(["-f", "1", "-l", &count.to_string()])
        .arg(path)
        .run_ok()?
        .stdout;
    let mut sizes = Vec::new();
    let mut turned = Vec::new();
    for line in info.lines().filter(|l| l.starts_with("Page ")) {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        let mut words = value.split_whitespace();
        if label.ends_with(" size") {
            let w = words.next().and_then(|v| v.parse::<f64>().ok());
            let h = words.nth(1).and_then(|v| v.parse::<f64>().ok());
            if let (Some(w), Some(h)) = (w, h) {
                sizes.push((w, h));
            }
        } else if label.ends_with(" rot") {
            let deg = words
                .next()
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);
            turned.push(deg.rem_euclid(180) == 90);
        }
    }
    if sizes.len() != count as usize || turned.len() != sizes.len() {
        return Err(JobError::invalid_input("could not read the page sizes"));
    }
    Ok(sizes
        .into_iter()
        .zip(turned)
        .map(|((w, h), t)| if t { (h, w) } else { (w, h) })
        .collect())
}

/// Copy `src` to `out` with every page scaled to the width of the narrowest.
/// Pages already that wide stay untouched; Ghostscript rewrites the others.
fn write_uniform_width(ctx: &Ctx, src: &Path, out: &Path, work: &WorkDir) -> JobResult<()> {
    let sizes = shown_page_sizes(ctx, src)?;
    let narrowest = sizes.iter().map(|s| s.0).fold(f64::INFINITY, f64::min);
    if !narrowest.is_finite() || narrowest <= 0.0 {
        return Err(JobError::invalid_input("The PDF has no valid page sizes."));
    }
    let mut files = Vec::new();
    for (n, (w, h)) in sizes.iter().enumerate() {
        crate::process::check_cancel()?;
        let page = n as u32 + 1;
        let one = work.join(&format!("uniform-{page}.pdf"));
        extract_page(ctx, src, page, &one)?;
        if (w - narrowest).abs() < 0.5 {
            files.push(one);
            continue;
        }
        let scaled = work.join(&format!("scaled-{page}.pdf"));
        gs_base(ctx)?
            .args(["-dFIXEDMEDIA", "-dPDFFitPage"])
            .arg(format!("-dDEVICEWIDTHPOINTS={}", narrowest.round()))
            .arg(format!(
                "-dDEVICEHEIGHTPOINTS={}",
                (h * narrowest / w).round().max(1.0)
            ))
            .arg(gs_out(&scaled))
            .arg("-f")
            .arg(&one)
            .run_ok()?;
        files.push(scaled);
    }
    let paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    join_pdfs(ctx, &paths, out, work)
}

fn extract_page(ctx: &Ctx, src: &Path, page: u32, out: &Path) -> JobResult<()> {
    let d = ctx.deps;
    let p = page.to_string();
    if let Some(qpdf) = d.path("qpdf") {
        let r = Cmd::new(qpdf)
            .arg("--empty")
            .arg("--pages")
            .arg(src)
            .arg(&p)
            .arg("--")
            .arg(out)
            .run()?;
        if !matches!(r.status.code(), Some(0) | Some(3)) {
            return Err(crate::process::failure("qpdf", &r));
        }
    } else if let Some(sep) = d.path("pdfseparate") {
        // The output argument is a printf-style pattern: run inside the
        // output's directory with a plain relative name.
        let dir = out.parent().unwrap_or(Path::new("."));
        let tmp_name = "nashi-page.pdf";
        Cmd::new(sep)
            .args(["-f", &p, "-l", &p])
            .arg(src)
            .arg(tmp_name)
            .cwd(dir)
            .run_ok()?;
        std::fs::rename(dir.join(tmp_name), out)?;
    } else {
        gs_base(ctx)?
            .arg(format!("-dFirstPage={p}"))
            .arg(format!("-dLastPage={p}"))
            .arg(gs_out(out))
            .arg("-f")
            .arg(src)
            .run_ok()?;
    }
    Ok(())
}

fn organize(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    let spec = ctx.opts.pages.as_deref().ok_or_else(|| {
        JobError::invalid_options("organizePDF needs a pages option (new page order)")
    })?;
    let d = ctx.deps;
    if ctx.opts.uniform_width == Some(true) {
        require(d, "gs")?;
    }
    let pages = parse_pages(spec, pdf_pages(d, &input.path))?;
    let rotations = match &ctx.opts.rotations {
        Some(r) => Some(crate::options::normalize_rotations(r, pages.len())?),
        None => None,
    };
    let has_rotations = rotations
        .as_ref()
        .is_some_and(|r| r.iter().any(|&deg| deg != 0));
    ctx.progress(None, "Reordering pages");
    let staged = input.stage("-organized", "pdf")?;
    let list = pages
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if let Some(qpdf) = d.path("qpdf") {
        let work = WorkDir::new_in(&input.out_dir)?;
        let first_dst = if has_rotations {
            work.join("reordered.pdf")
        } else {
            staged.path().to_path_buf()
        };
        let r = Cmd::new(qpdf)
            .arg("--empty")
            .arg("--pages")
            .arg(&input.path)
            .arg(&list)
            .arg("--")
            .arg(&first_dst)
            .run()?;
        if !matches!(r.status.code(), Some(0) | Some(3)) {
            return Err(crate::process::failure("qpdf", &r));
        }
        if let Some(rots) = rotations.filter(|_| has_rotations) {
            let mut cmd = Cmd::new(qpdf).arg(&first_dst);
            for (idx, deg) in rots.iter().enumerate() {
                if *deg != 0 {
                    cmd = cmd.arg(format!("--rotate=+{deg}:{}", idx + 1));
                }
            }
            let r2 = cmd.arg(staged.path()).run()?;
            if !matches!(r2.status.code(), Some(0) | Some(3)) {
                return Err(crate::process::failure("qpdf", &r2));
            }
        }
    } else if has_rotations {
        require(d, "qpdf")?;
    } else {
        let unite = require(d, "pdfunite")?;
        let work = WorkDir::new_in(&input.out_dir)?;
        let mut parts = Vec::new();
        for (n, page) in pages.iter().enumerate() {
            let out = work.join(&format!("p{n}.pdf"));
            extract_page(ctx, &input.path, *page, &out)?;
            parts.push(out);
        }
        Cmd::new(unite).args(&parts).arg(staged.path()).run_ok()?;
    }
    if ctx.opts.uniform_width == Some(true) {
        let work = WorkDir::new_in(&input.out_dir)?;
        let ordered = work.join("ordered.pdf");
        std::fs::rename(staged.path(), &ordered)?;
        write_uniform_width(ctx, &ordered, staged.path(), &work)?;
    }
    Ok(staged.commit()?.into())
}

/// Join PDFs into one with the first tool available (qpdf, pdfunite, gs).
fn join_pdfs(ctx: &Ctx, paths: &[&Path], out: &Path, work: &WorkDir) -> JobResult<()> {
    let d = ctx.deps;
    if let Some(qpdf) = d.path("qpdf") {
        let r = Cmd::new(qpdf)
            .arg("--empty")
            .arg("--pages")
            .args(paths)
            .arg("--")
            .arg(out)
            .run()?;
        if !matches!(r.status.code(), Some(0) | Some(3)) {
            return Err(crate::process::failure("qpdf", &r));
        }
    } else if let Some(unite) = d.path("pdfunite") {
        Cmd::new(unite).args(paths).arg(out).run_ok()?;
    } else {
        // Ghostscript reads the output name as a pattern: write to a plain one first.
        let tmp = work.join("joined.pdf");
        gs_base(ctx)?
            .arg(gs_out(&tmp))
            .arg("-f")
            .args(paths)
            .run_ok()?;
        std::fs::rename(&tmp, out)?;
    }
    Ok(())
}

/// Merge PDFs into `merged.pdf`.
pub fn merge(ctx: &Ctx, inputs: &[Input], out_dir: &Path) -> JobResult<PathBuf> {
    ctx.progress(None, "Merging");
    let uniform = ctx.opts.uniform_width == Some(true);
    if uniform {
        require(ctx.deps, "gs")?;
    }
    let staged = Staged::file(out_dir, "merged", "pdf")?;
    let paths: Vec<&Path> = inputs.iter().map(|i| i.path.as_path()).collect();
    let work = WorkDir::new_in(out_dir)?;
    if uniform {
        let joined = work.join("all.pdf");
        join_pdfs(ctx, &paths, &joined, &work)?;
        write_uniform_width(ctx, &joined, staged.path(), &work)?;
    } else {
        join_pdfs(ctx, &paths, staged.path(), &work)?;
    }
    Ok(staged.commit()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gs_output_escapes_percent() {
        assert_eq!(
            gs_out(Path::new("/a%d/b%")),
            OsString::from("-sOutputFile=/a%%d/b%%")
        );
    }

    #[test]
    fn markdown_from_text() {
        let md = text_to_markdown("# not a heading\nline\n\u{c}page two\n");
        assert_eq!(md, "\\# not a heading\nline\n\n\n---\n\npage two\n");
    }
}

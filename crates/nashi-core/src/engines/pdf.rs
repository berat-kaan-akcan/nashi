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
    let base = Cmd::new(ppm).args(fmt_args).args(["-r", "150"]);
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
    Ok(())
}

/// Convert a PDF.
pub fn convert(ctx: &Ctx, input: &Input, target: &str) -> JobResult<Produced> {
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
                super::epub::write_epub(staged.path(), input.stem(), &body)?;
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

/// Single-input PDF tools.
pub fn tool(ctx: &Ctx, tool: &str, input: &Input) -> JobResult<Produced> {
    match tool {
        "compress" => {
            ctx.progress(None, "Compressing");
            let preset = ctx.opts.pdf_preset.as_deref().unwrap_or("ebook");
            let staged = input.stage("-compressed", "pdf")?;
            let work = WorkDir::new_in(&input.out_dir)?;
            let tmp = work.join("out.pdf");
            gs_base(ctx)?
                .arg(format!("-dPDFSETTINGS=/{preset}"))
                .arg("-dCompatibilityLevel=1.5")
                .arg(gs_out(&tmp))
                .arg("-f")
                .arg(&input.path)
                .run_ok()?;
            staged.take_from(&tmp)?;
            Ok(staged.commit()?.into())
        }
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
    if let Some(exif) = d.path("exiftool") {
        let tmp = work.join("clean.pdf");
        Cmd::new(exif)
            .args(["-q", "-q", "-all:all=", "-o"])
            .arg(&tmp)
            .arg(&input.path)
            .run_ok()?;
        // exiftool edits PDFs incrementally (old metadata stays recoverable);
        // rewrite the file so the removal is real.
        if let Some(qpdf) = d.path("qpdf") {
            let out = Cmd::new(qpdf)
                .arg("--linearize")
                .arg(&tmp)
                .arg(staged.path())
                .run()?;
            // qpdf exit 3 = warnings, output still written.
            if !matches!(out.status.code(), Some(0) | Some(3)) {
                return Err(crate::process::failure("qpdf", &out));
            }
        } else if d.has("gs") {
            let out = work.join("blank.pdf");
            gs_blank_info(ctx, &tmp, &out)?;
            staged.take_from(&out)?;
        } else {
            staged.take_from(&tmp)?;
        }
    } else {
        let out = work.join("blank.pdf");
        gs_blank_info(ctx, &input.path, &out)?;
        staged.take_from(&out)?;
    }
    Ok(staged.commit()?.into())
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
    let staged = input.stage_dir("-pages")?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let stem = input.stem();
    for (n, page) in pages.iter().enumerate() {
        crate::process::check_cancel()?;
        ctx.progress(Some(n as f64 / pages.len() as f64), "Splitting");
        // Tools write to a fixed, %-free name; then move under the real name.
        let tmp = work.join(&format!("p{n}.pdf"));
        extract_page(ctx, &input.path, *page, &tmp)?;
        std::fs::rename(&tmp, staged.path().join(format!("{stem}-{page}.pdf")))?;
    }
    Ok(staged.commit()?.into())
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
    let pages = parse_pages(spec, pdf_pages(d, &input.path))?;
    ctx.progress(None, "Reordering pages");
    let staged = input.stage("-organized", "pdf")?;
    let list = pages
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if let Some(qpdf) = d.path("qpdf") {
        let r = Cmd::new(qpdf)
            .arg("--empty")
            .arg("--pages")
            .arg(&input.path)
            .arg(&list)
            .arg("--")
            .arg(staged.path())
            .run()?;
        if !matches!(r.status.code(), Some(0) | Some(3)) {
            return Err(crate::process::failure("qpdf", &r));
        }
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
    Ok(staged.commit()?.into())
}

/// Merge PDFs into `merged.pdf` (qpdf, pdfunite, gs).
pub fn merge(ctx: &Ctx, inputs: &[Input], out_dir: &Path) -> JobResult<PathBuf> {
    ctx.progress(None, "Merging");
    let staged = Staged::file(out_dir, "merged", "pdf")?;
    let d = ctx.deps;
    let paths: Vec<&Path> = inputs.iter().map(|i| i.path.as_path()).collect();
    if let Some(qpdf) = d.path("qpdf") {
        let r = Cmd::new(qpdf)
            .arg("--empty")
            .arg("--pages")
            .args(&paths)
            .arg("--")
            .arg(staged.path())
            .run()?;
        if !matches!(r.status.code(), Some(0) | Some(3)) {
            return Err(crate::process::failure("qpdf", &r));
        }
    } else if let Some(unite) = d.path("pdfunite") {
        Cmd::new(unite).args(&paths).arg(staged.path()).run_ok()?;
    } else {
        let work = WorkDir::new_in(out_dir)?;
        let tmp = work.join("merged.pdf");
        gs_base(ctx)?
            .arg(gs_out(&tmp))
            .arg("-f")
            .args(&paths)
            .run_ok()?;
        staged.take_from(&tmp)?;
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

//! Documents, presentations, spreadsheets and text via LibreOffice (headless,
//! private per-job profile so it works while LibreOffice is open), pandoc for
//! EPUB when present, and native handling for txt/md/textclipping/epub.

use std::path::{Path, PathBuf};

use super::{epub, pdf, textclip, Ctx, Input, Produced};
use crate::catalog::{category_of, Category};
use crate::deps::require;
use crate::events::{JobError, JobResult};
use crate::fsops::{single_file_in, WorkDir};
use crate::process::Cmd;

/// `file://` URL for a local path (percent-encoding unsafe bytes).
pub fn file_url(p: &Path) -> String {
    let mut s = String::from("file://");
    for b in p.to_string_lossy().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                s.push(*b as char)
            }
            _ => s.push_str(&format!("%{b:02X}")),
        }
    }
    s
}

/// LibreOffice `--convert-to` filter for a target extension.
fn lo_filter(target: &str) -> Option<&'static str> {
    Some(match target {
        "pdf" => "pdf",
        "docx" => "docx:MS Word 2007 XML",
        "txt" => "txt:Text (encoded):UTF8",
        "md" => "md",
        "html" => "html:XHTML Writer File:UTF8",
        "xlsx" => "xlsx:Calc MS Excel 2007 XML",
        "ods" => "ods",
        "csv" => "csv:Text - txt - csv (StarCalc):44,34,76",
        "tsv" => "csv:Text - txt - csv (StarCalc):9,34,76",
        "pptx" => "pptx:Impress MS PowerPoint 2007 XML",
        _ => return None,
    })
}

/// LibreOffice `--infilter` for a source extension.
fn lo_infilter(src: &str) -> Option<&'static str> {
    match src {
        "csv" => Some("CSV:44,34,76"),
        "tsv" => Some("CSV:9,34,76"),
        "txt" => Some("Text (encoded):UTF8"),
        "html" => Some("HTML (StarWriter)"),
        _ => None,
    }
}

/// Per-job LibreOffice settings: macros off (security level "very high"),
/// never update links on load (Writer: 2 = never, Calc: 1 = never), block
/// untrusted referer links, no active content / OLE automation.
pub const LO_REGISTRY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<oor:items xmlns:oor="http://openoffice.org/2001/registry" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="MacroSecurityLevel" oor:op="fuse"><value>3</value></prop></item>
<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="DisableMacrosExecution" oor:op="fuse"><value>true</value></prop></item>
<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="BlockUntrustedRefererLinks" oor:op="fuse"><value>true</value></prop></item>
<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="DisableActiveContent" oor:op="fuse"><value>true</value></prop></item>
<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="DisableOLEAutomation" oor:op="fuse"><value>true</value></prop></item>
<item oor:path="/org.openoffice.Office.Writer/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>2</value></prop></item>
<item oor:path="/org.openoffice.Office.Calc/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>1</value></prop></item>
</oor:items>
"#;

/// Seed a fresh profile directory with [`LO_REGISTRY`].
pub fn write_hardened_profile(profile: &Path) -> std::io::Result<()> {
    let user = profile.join("user");
    std::fs::create_dir_all(&user)?;
    std::fs::write(user.join("registrymodifications.xcu"), LO_REGISTRY)
}

/// Run `soffice --convert-to` into `work/out/` and return the produced file.
pub fn soffice(
    ctx: &Ctx,
    src: &Path,
    convert_to: &str,
    infilter: Option<&str>,
    work: &WorkDir,
) -> JobResult<PathBuf> {
    let so = require(ctx.deps, "soffice")?;
    let profile = WorkDir::system()?;
    write_hardened_profile(profile.path())?;
    let outdir = work.join("out");
    std::fs::create_dir_all(&outdir)?;
    let mut cmd = Cmd::new(so)
        .arg(format!(
            "-env:UserInstallation={}",
            file_url(profile.path())
        ))
        .args([
            "--headless",
            "--invisible",
            "--norestore",
            "--nolockcheck",
            "--nodefault",
            "--nologo",
        ]);
    if let Some(f) = infilter {
        cmd = cmd.arg(format!("--infilter={f}"));
    }
    let out = cmd
        .arg("--convert-to")
        .arg(convert_to)
        .arg("--outdir")
        .arg(&outdir)
        .arg(src)
        .env("SAL_USE_VCLPLUGIN", "svp")
        .run()?;
    match single_file_in(&outdir) {
        Ok(p) => Ok(p),
        Err(_) => {
            let msg = out
                .stdout
                .lines()
                .chain(out.stderr.lines())
                .map(str::trim)
                .find(|l| l.starts_with("Error"))
                .map(str::to_string)
                .unwrap_or_else(|| {
                    format!(
                        "LibreOffice produced no output (exit {:?})",
                        out.status.code()
                    )
                });
            Err(JobError::tool_failed(format!("soffice: {msg}")))
        }
    }
}

/// soffice conversion straight into `stem.<out_ext>` next to the input.
pub fn soffice_to_output(
    ctx: &Ctx,
    input: &Input,
    src: &Path,
    convert_to: &str,
    infilter: Option<&str>,
    out_ext: &str,
) -> JobResult<Produced> {
    ctx.progress(None, "Converting with LibreOffice");
    let staged = input.stage("", out_ext)?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let produced = soffice(ctx, src, convert_to, infilter, &work)?;
    staged.take_from(&produced)?;
    Ok(staged.commit()?.into())
}

/// Pandoc conversion (used for EPUB when available).
///
/// `--sandbox` (pandoc >= 2.15) stops a crafted input from embedding other local
/// files (e.g. `![](~/.ssh/id_rsa)` into an EPUB) or fetching URLs.
fn pandoc(ctx: &Ctx, src: &Path, from: &str, to: &str, out: &Path) -> JobResult<()> {
    let p = require(ctx.deps, "pandoc")?;
    Cmd::new(p)
        .arg("--sandbox")
        .args(["-f", from, "-t", to, "-o"])
        .arg(out)
        .arg(src)
        .run_ok()?;
    Ok(())
}

fn pandoc_format(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "md" => "markdown",
        "docx" => "docx",
        "html" => "html",
        "epub" => "epub",
        "txt" => "plain",
        "rtf" => "rtf",
        _ => return None,
    })
}

/// Normalise special sources to something LibreOffice understands.
/// Returns `(path, ext)` of the effective source.
fn effective_source(ctx: &Ctx, input: &Input, work: &WorkDir) -> JobResult<(PathBuf, String)> {
    match input.ext() {
        "textclipping" => {
            let data = std::fs::read(&input.path)?;
            let text = textclip::extract_text(&data).ok_or_else(|| {
                JobError::invalid_input(
                    "no text found in the clipping (resource-fork clippings are not supported)",
                )
            })?;
            let p = work.join("clipping.txt");
            std::fs::write(&p, text)?;
            Ok((p, "txt".into()))
        }
        "rtfd" => {
            if input.path.is_dir() {
                let rtf = input.path.join("TXT.rtf");
                let regular = std::fs::symlink_metadata(&rtf).is_ok_and(|m| m.is_file());
                if regular {
                    return Ok((rtf, "rtf".into()));
                }
                return Err(JobError::invalid_input("RTFD bundle has no TXT.rtf"));
            }
            // A bundle flattened into a zip archive.
            let dest = work.join("rtfd");
            std::fs::create_dir_all(&dest)?;
            super::archive::extract_into(
                ctx,
                &input.path,
                "zip",
                &dest,
                super::archive::Links::RemoveAll,
            )
            .map_err(|_| JobError::invalid_input("not an RTFD bundle"))?;
            find_named(&dest, "TXT.rtf")
                .map(|p| (p, "rtf".into()))
                .ok_or_else(|| JobError::invalid_input("RTFD bundle has no TXT.rtf"))
        }
        "epub" => {
            if ctx.deps.has("pandoc") {
                return Ok((input.path.clone(), "epub".into()));
            }
            let html = epub::epub_to_html(ctx, &input.path, work)?;
            Ok((html, "html".into()))
        }
        _ => Ok((input.path.clone(), input.ext().to_string())),
    }
}

fn find_named(dir: &Path, name: &str) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        let ft = std::fs::symlink_metadata(&p).ok()?.file_type();
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            if let Some(f) = find_named(&p, name) {
                return Some(f);
            }
        } else if p.file_name().is_some_and(|n| n == name) {
            return Some(p);
        }
    }
    None
}

/// Convert a document/presentation/spreadsheet/text file.
pub fn convert(ctx: &Ctx, input: &Input, target: &str) -> JobResult<Produced> {
    ctx.progress(None, "Converting");
    let work = WorkDir::new_in(&input.out_dir)?;
    let (src, src_ext) = effective_source(ctx, input, &work)?;

    // Plain text is valid Markdown: copy natively.
    if src_ext == "txt" && target == "md" {
        let staged = input.stage("", "md")?;
        std::fs::copy(&src, staged.path())?;
        return Ok(staged.commit_allow_empty()?.into());
    }
    if src_ext == "txt" && target == "txt" {
        let staged = input.stage("", "txt")?;
        std::fs::copy(&src, staged.path())?;
        return Ok(staged.commit_allow_empty()?.into());
    }
    // EPUB source via pandoc.
    if src_ext == "epub" {
        let to = match target {
            "md" => "gfm",
            "txt" => "plain",
            "html" => "html",
            "docx" => "docx",
            _ => "html",
        };
        if target == "pdf" {
            let html = work.join("book.html");
            pandoc(ctx, &src, "epub", "html", &html)?;
            return soffice_to_output(ctx, input, &html, "pdf", lo_infilter("html"), "pdf");
        }
        let staged = input.stage("", target)?;
        let tmp = work.join(&format!("out.{target}"));
        pandoc(ctx, &src, "epub", to, &tmp)?;
        staged.take_from(&tmp)?;
        return Ok(staged.commit()?.into());
    }
    // EPUB from HTML (native epub source path) is a direct copy.
    if src_ext == "html" && target == "html" {
        let staged = input.stage("", "html")?;
        std::fs::copy(&src, staged.path())?;
        return Ok(staged.commit()?.into());
    }
    match target {
        "epub" => {
            let staged = input.stage("", "epub")?;
            if src_ext == "txt" {
                let text = String::from_utf8_lossy(&std::fs::read(&src)?).into_owned();
                epub::write_epub(
                    staged.path(),
                    input.stem(),
                    &epub::text_to_xhtml_body(&text),
                )?;
                return Ok(staged.commit()?.into());
            }
            if let (true, Some(from)) = (ctx.deps.has("pandoc"), pandoc_format(&src_ext)) {
                let tmp = work.join("out.epub");
                pandoc(ctx, &src, from, "epub", &tmp)?;
                staged.take_from(&tmp)?;
                return Ok(staged.commit()?.into());
            }
            // LibreOffice → XHTML, packaged natively as EPUB.
            let xhtml = soffice(
                ctx,
                &src,
                "html:XHTML Writer File:UTF8",
                lo_infilter(&src_ext),
                &work,
            )?;
            let html = String::from_utf8_lossy(&std::fs::read(&xhtml)?).into_owned();
            epub::write_epub(staged.path(), input.stem(), &epub::html_body(&html))?;
            Ok(staged.commit()?.into())
        }
        "jpg" | "png" => {
            ctx.progress(None, "Converting with LibreOffice");
            let pdf_path = soffice(ctx, &src, "pdf", lo_infilter(&src_ext), &work)?;
            pdf::to_images(ctx, &pdf_path, input.stem(), &input.out_dir, target)
        }
        "pdf" if src_ext == "html" => soffice_to_output(
            ctx,
            input,
            &src,
            "pdf:writer_pdf_Export",
            lo_infilter("html"),
            "pdf",
        ),
        _ => {
            let filter = lo_filter(target).ok_or_else(|| {
                JobError::unsupported(format!(".{} to .{target} is not supported", input.ext()))
            })?;
            if category_of(&src_ext) == Category::Spreadsheet && target == "txt" {
                return Err(JobError::unsupported(
                    "spreadsheet to text is not supported",
                ));
            }
            soffice_to_output(ctx, input, &src, filter, lo_infilter(&src_ext), target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(
            file_url(Path::new("/tmp/a b/ü")),
            "file:///tmp/a%20b/%C3%BC"
        );
    }
}

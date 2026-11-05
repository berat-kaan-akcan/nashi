//! PDF to EPUB and DOCX as page images, which keeps the layout and also works
//! for scanned PDFs. Skipped (not failed) when an external tool is missing.

mod common;
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn zip_names(p: &Path) -> Vec<String> {
    let o = Command::new("unzip").args(["-Z1"]).arg(p).output().unwrap();
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(String::from)
        .collect()
}

fn zip_text(p: &Path, name: &str) -> String {
    let o = Command::new("unzip")
        .arg("-p")
        .arg(p)
        .arg(name)
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn converted(input: &Path, target: &str, options: &str) -> Run {
    run(&["convert", "--json", target, "--options", options, s(input)])
}

/// A wide and a tall page holding only pictures, as a scan does.
fn scanned(dir: &Path) -> PathBuf {
    let p = dir.join("scan.pdf");
    tool(
        "magick",
        &[
            "-size",
            "600x300",
            "xc:red",
            "-size",
            "200x400",
            "xc:blue",
            "-units",
            "PixelsPerInch",
            "-density",
            "72",
            s(&p),
        ],
    );
    p
}

#[test]
fn epub_keeps_each_page_in_a_fixed_layout() {
    need!("magick", "pdftoppm", "pdfinfo", "unzip");
    let d = tempfile::tempdir().unwrap();
    let f = scanned(d.path());
    let before = hash(&f);
    let r = converted(&f, "epub", r#"{"pdf_layout":"pages"}"#);
    r.ok();
    let out = r.one();
    let names = zip_names(&out);
    assert_eq!(names[0], "mimetype");
    assert_eq!(
        names.iter().filter(|n| n.ends_with(".jpg")).count(),
        2,
        "{names:?}"
    );
    let opf = zip_text(&out, "OEBPS/content.opf");
    assert!(opf.contains("pre-paginated"), "{opf}");
    assert!(opf.contains("version=\"3.0\""), "{opf}");
    assert!(opf.contains("dcterms:modified"), "{opf}");
    assert!(opf.contains("properties=\"nav\""), "{opf}");
    assert!(opf.find("idref=\"p1\"").unwrap() < opf.find("idref=\"p2\"").unwrap());
    let page = zip_text(&out, "OEBPS/p2.xhtml");
    assert!(page.contains("width=200, height=400"), "{page}");
    assert_eq!(hash(&f), before);
}

#[test]
fn docx_keeps_each_page_at_its_own_size() {
    need!("magick", "pdftoppm", "pdfinfo", "unzip");
    let d = tempfile::tempdir().unwrap();
    let f = scanned(d.path());
    let r = converted(&f, "docx", r#"{"pdf_layout":"pages"}"#);
    r.ok();
    let out = r.one();
    let names = zip_names(&out);
    assert_eq!(
        names
            .iter()
            .filter(|n| n.starts_with("word/media/"))
            .count(),
        2,
        "{names:?}"
    );
    let doc = zip_text(&out, "word/document.xml");
    // Twentieths of a point for the page, and EMU (12700 per point) for the picture.
    assert!(doc.contains(r#"w:w="12000" w:h="6000""#), "{doc}");
    assert!(doc.contains(r#"w:w="4000" w:h="8000""#), "{doc}");
    assert!(doc.contains(r#"cx="7620000" cy="3810000""#), "{doc}");
    assert!(doc.contains(r#"w:orient="landscape""#), "{doc}");
}

#[test]
fn a_docx_of_pages_opens_with_the_same_page_count() {
    need!("magick", "pdftoppm", "pdfinfo", "soffice");
    let d = tempfile::tempdir().unwrap();
    let f = scanned(d.path());
    let r = converted(&f, "docx", r#"{"pdf_layout":"pages"}"#);
    r.ok();
    let back = d.path().join("back");
    std::fs::create_dir(&back).unwrap();
    let again = run(&[
        "convert",
        "--json",
        "pdf",
        "--out-dir",
        s(&back),
        s(&r.one()),
    ]);
    again.ok();
    let info = Command::new("pdfinfo").arg(again.one()).output().unwrap();
    let text = String::from_utf8_lossy(&info.stdout).into_owned();
    assert!(
        text.lines()
            .any(|l| l.starts_with("Pages:") && l.ends_with(" 2")),
        "{text}"
    );
}

#[test]
fn the_default_stays_editable_text() {
    need!("magick", "pdftotext", "unzip");
    let d = tempfile::tempdir().unwrap();
    let f = pdf(d.path(), "text.pdf", 2);
    let r = converted(&f, "epub", "{}");
    r.ok();
    assert!(zip_names(&r.one()).iter().all(|n| !n.ends_with(".jpg")));
    let same = converted(&f, "epub", r#"{"pdf_layout":"editable"}"#);
    same.ok();
    assert!(zip_names(&same.one()).iter().all(|n| !n.ends_with(".jpg")));
    // A scan has no text, so only the page layout can convert it.
    let scan = scanned(d.path());
    assert_eq!(
        converted(&scan, "epub", "{}").error_codes(),
        ["invalid_input"]
    );
    assert_eq!(
        converted(&f, "epub", r#"{"pdf_layout":"x"}"#).error_codes(),
        ["invalid_options"]
    );
}

#[test]
fn page_layout_needs_pdftoppm_and_says_so() {
    need!("magick", "pdfinfo");
    let d = tempfile::tempdir().unwrap();
    let f = scanned(d.path());
    let r = run_env(
        &[
            "convert",
            "--json",
            "epub",
            "--options",
            r#"{"pdf_layout":"pages"}"#,
            s(&f),
        ],
        &[("NASHI_DISABLE_TOOLS", "pdftoppm")],
    );
    assert_eq!(r.error_codes(), ["missing_dependency"]);
    assert!(r.outputs().is_empty());
}

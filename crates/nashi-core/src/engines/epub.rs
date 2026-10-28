//! Native EPUB 2 packaging (single XHTML chapter) and a simple EPUB reader
//! that flattens the spine to one HTML file (used when pandoc is missing).

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use super::{archive, Ctx};
use crate::events::{JobError, JobResult};
use crate::fsops::WorkDir;

pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

/// Plain text → XHTML body: blank lines separate paragraphs.
pub fn text_to_xhtml_body(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\u{c}', "\n\n");
    let mut out = String::new();
    for para in text.split("\n\n") {
        let p = para.trim();
        if p.is_empty() {
            continue;
        }
        out.push_str("<p>");
        out.push_str(&escape(p).replace('\n', "<br/>"));
        out.push_str("</p>\n");
    }
    out
}

/// Inner `<body>` markup of an (X)HTML document.
pub fn html_body(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let start = lower
        .find("<body")
        .and_then(|i| lower[i..].find('>').map(|j| i + j + 1))
        .unwrap_or(0);
    let end = lower.rfind("</body>").unwrap_or(html.len());
    let body = if start <= end {
        &html[start..end]
    } else {
        html
    };
    // Drop <img> tags: images are not packaged by the minimal writer.
    strip_tag(body, "img")
}

fn strip_tag(s: &str, tag: &str) -> String {
    let mut out = String::new();
    let lower = s.to_ascii_lowercase();
    let open = format!("<{tag}");
    let mut i = 0;
    while let Some(pos) = lower[i..].find(&open) {
        let a = i + pos;
        out.push_str(&s[i..a]);
        match lower[a..].find('>') {
            Some(e) => i = a + e + 1,
            None => {
                i = s.len();
                break;
            }
        }
    }
    out.push_str(&s[i..]);
    out
}

/// Write an EPUB 2 file with one chapter.
pub fn write_epub(path: &Path, title: &str, body_xhtml: &str) -> std::io::Result<()> {
    let t = escape(title);
    let id = format!("urn:nashi:{:x}", {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        title.hash(&mut h);
        body_xhtml.len().hash(&mut h);
        h.finish()
    });
    let container = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>
"#;
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="bookid" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{t}</dc:title><dc:identifier id="bookid">{id}</dc:identifier><dc:language>und</dc:language>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="c1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx"><itemref idref="c1"/></spine>
</package>
"#
    );
    let ncx = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head><meta name="dtb:uid" content="{id}"/></head>
  <docTitle><text>{t}</text></docTitle>
  <navMap><navPoint id="n1" playOrder="1"><navLabel><text>{t}</text></navLabel><content src="chapter.xhtml"/></navPoint></navMap>
</ncx>
"#
    );
    let chapter = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{t}</title></head><body>\n{body_xhtml}\n</body></html>\n"
    );
    let f = BufWriter::new(File::create(path)?);
    super::zipw::write_zip(
        f,
        &[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", container.as_bytes()),
            ("OEBPS/content.opf", opf.as_bytes()),
            ("OEBPS/toc.ncx", ncx.as_bytes()),
            ("OEBPS/chapter.xhtml", chapter.as_bytes()),
        ],
    )
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=");
    let i = tag.find(&key)? + key.len();
    let q = tag[i..].chars().next()?;
    if q != '"' && q != '\'' {
        return None;
    }
    let rest = &tag[i + 1..];
    Some(rest[..rest.find(q)?].to_string())
}

fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let mut v = Vec::new();
    let mut i = 0;
    while let Some(p) = xml[i..].find(&open) {
        let a = i + p;
        let Some(e) = xml[a..].find('>') else { break };
        v.push(&xml[a..a + e + 1]);
        i = a + e + 1;
    }
    v
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Flatten an EPUB's spine into one HTML file inside `work` (the file sits in
/// the OPF directory so relative image links still resolve).
pub fn epub_to_html(ctx: &Ctx, epub: &Path, work: &WorkDir) -> JobResult<PathBuf> {
    let dir = work.join("epub");
    std::fs::create_dir_all(&dir)?;
    archive::extract_into(ctx, epub, "zip", &dir, archive::Links::RemoveAll)?;
    let canon = dir.canonicalize()?;
    // Only regular files that really live under the extraction directory.
    let read_inside = |p: &Path| -> Option<String> {
        let c = p.canonicalize().ok()?;
        if !c.starts_with(&canon) || !std::fs::symlink_metadata(&c).ok()?.is_file() {
            return None;
        }
        std::fs::read_to_string(c).ok()
    };
    let bad = || JobError::invalid_input("not a valid EPUB file");
    let container =
        std::fs::read_to_string(dir.join("META-INF/container.xml")).map_err(|_| bad())?;
    let opf_rel = tags(&container, "rootfile")
        .into_iter()
        .find_map(|t| attr(t, "full-path"))
        .ok_or_else(bad)?;
    let opf_path = safe_join(&dir, &opf_rel).ok_or_else(bad)?;
    let opf = read_inside(&opf_path).ok_or_else(bad)?;
    let base = opf_path.parent().unwrap_or(&dir).to_path_buf();
    let manifest: Vec<(String, String)> = tags(&opf, "item")
        .into_iter()
        .filter_map(|t| Some((attr(t, "id")?, attr(t, "href")?)))
        .collect();
    let mut body = String::new();
    for r in tags(&opf, "itemref") {
        let Some(idref) = attr(r, "idref") else {
            continue;
        };
        let Some((_, href)) = manifest.iter().find(|(id, _)| *id == idref) else {
            continue;
        };
        let href = url_decode(href.split('#').next().unwrap_or(""));
        let Some(p) = safe_join(&base, &href) else {
            continue;
        };
        if let Some(x) = read_inside(&p) {
            body.push_str(&html_body(&x));
            body.push('\n');
        }
    }
    if body.trim().is_empty() {
        return Err(bad());
    }
    let out = base.join(".nashi-book.html");
    std::fs::write(
        &out,
        format!("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"/></head><body>\n{body}</body></html>\n"),
    )?;
    Ok(out)
}

/// Join a relative archive path without escaping `base`.
fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut p = base.to_path_buf();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            x => p.push(x),
        }
    }
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_and_text() {
        assert_eq!(
            text_to_xhtml_body("a <b>\nc\n\nd"),
            "<p>a &lt;b&gt;<br/>c</p>\n<p>d</p>\n"
        );
        assert_eq!(
            html_body("<html><BODY class=x><p>hi<img src='a.png'/></p></body></html>"),
            "<p>hi</p>"
        );
        assert_eq!(
            attr(r#"<item id="c1" href='x.html'/>"#, "href").as_deref(),
            Some("x.html")
        );
        assert!(safe_join(Path::new("/a"), "../etc").is_none());
        assert_eq!(url_decode("a%20b"), "a b");
    }
}

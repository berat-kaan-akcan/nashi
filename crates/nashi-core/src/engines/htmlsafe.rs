//! Sanitising user HTML before LibreOffice renders it to PDF.
//!
//! Same policy as upstream Nashi's HTML → PDF: no scripts or embedded
//! active content, nothing remote, and only regular files inside the page's
//! own folder may be loaded (images, stylesheets). Verified images are
//! inlined as `data:` URIs and stylesheets as `<style>` (LibreOffice renders
//! only inline images to PDF), so the sanitised copy is self-contained and
//! LibreOffice never opens another file or the network.
//!
//! This is a small byte-level tag scanner, not a full HTML parser: anything
//! it cannot classify safely is dropped rather than passed through.

use std::path::{Path, PathBuf};

/// Largest image or stylesheet that is inlined.
const MAX_INLINE: u64 = 16 * 1024 * 1024;

/// Elements removed together with their content.
const DROP_WITH_CONTENT: &[&str] = &[
    "script", "iframe", "frame", "frameset", "object", "embed", "applet", "noscript", "template",
    "audio", "video", "portal",
];
/// Elements removed (their content, if any, stays).
const DROP_TAG: &[&str] = &["base", "source", "track", "param"];
/// Attributes that load a resource.
const LOADING_ATTRS: &[&str] = &[
    "src",
    "poster",
    "background",
    "data",
    "xlink:href",
    "lowsrc",
    "dynsrc",
    "longdesc",
    "manifest",
    "codebase",
    "archive",
    "profile",
    "formaction",
    "action",
    "ping",
];

/// Sanitise `html` (bytes of a page that lives in `base_dir`).
pub fn sanitize(html: &[u8], base_dir: &Path) -> Vec<u8> {
    let base = std::fs::canonicalize(base_dir).ok();
    let mut out = Vec::with_capacity(html.len());
    let mut i = 0;
    while i < html.len() {
        if html[i] != b'<' {
            let next = memchr(b'<', &html[i..]).map_or(html.len(), |n| i + n);
            out.extend_from_slice(&html[i..next]);
            i = next;
            continue;
        }
        // Comments (including conditional comments) are dropped.
        if html[i..].starts_with(b"<!--") {
            i = find(&html[i + 4..], b"-->").map_or(html.len(), |n| i + 4 + n + 3);
            continue;
        }
        // Doctype / processing instructions pass through unchanged.
        if html[i..].starts_with(b"<!") || html[i..].starts_with(b"<?") {
            let end = memchr(b'>', &html[i..]).map_or(html.len(), |n| i + n + 1);
            if html[i..].starts_with(b"<!") && ascii_starts_ci(&html[i + 2..], b"doctype") {
                out.extend_from_slice(&html[i..end]);
            }
            i = end;
            continue;
        }
        let Some(tag) = parse_tag(&html[i..]) else {
            // A stray '<' is text.
            out.extend_from_slice(b"&lt;");
            i += 1;
            continue;
        };
        let after = i + tag.len;
        let name = tag.name.as_str();
        if tag.closing {
            if !DROP_WITH_CONTENT.contains(&name) && !DROP_TAG.contains(&name) {
                out.extend_from_slice(b"</");
                out.extend_from_slice(name.as_bytes());
                out.push(b'>');
            }
            i = after;
            continue;
        }
        // HTML ignores `/>` on non-void elements: `<script/>` and `<style/>`
        // still open raw-text content, so self_closing is not trusted here.
        if DROP_WITH_CONTENT.contains(&name) {
            i = skip_element(html, after, name);
            continue;
        }
        if DROP_TAG.contains(&name) || (name == "meta" && is_refresh(&tag)) {
            i = after;
            continue;
        }
        if name == "link" {
            if let Some(tag) = safe_link(&tag, base.as_deref()) {
                out.extend_from_slice(&tag);
            }
            i = after;
            continue;
        }
        out.extend_from_slice(&rebuild(&tag, base.as_deref()));
        i = after;
        if name == "style" {
            let end = end_tag(html, i, "style").unwrap_or(html.len());
            let clean = sanitize_css(&html[i..end], base.as_deref());
            out.extend_from_slice(&replace_ci(&clean, b"</style", b"<\\/style"));
            // The end tag is consumed here and always written cleanly, so no
            // unsanitised byte can follow an open raw-text element.
            out.extend_from_slice(b"</style>");
            i = if end < html.len() {
                memchr(b'>', &html[end..]).map_or(html.len(), |m| end + m + 1)
            } else {
                html.len()
            };
        }
    }
    out
}

struct Tag {
    name: String,
    attrs: Vec<(String, Option<Vec<u8>>)>,
    closing: bool,
    self_closing: bool,
    /// Bytes consumed, including `<` and `>`.
    len: usize,
}

fn parse_tag(s: &[u8]) -> Option<Tag> {
    let mut i = 1;
    let closing = s.get(i) == Some(&b'/');
    if closing {
        i += 1;
    }
    let start = i;
    while i < s.len() && (s[i].is_ascii_alphanumeric() || s[i] == b'-' || s[i] == b':') {
        i += 1;
    }
    if i == start || !s[start].is_ascii_alphabetic() {
        return None;
    }
    let name = String::from_utf8_lossy(&s[start..i]).to_ascii_lowercase();
    let mut attrs = Vec::new();
    let mut self_closing = false;
    loop {
        while i < s.len() && s[i].is_ascii_whitespace() {
            i += 1;
        }
        match s.get(i) {
            None => return None,
            Some(b'>') => {
                i += 1;
                break;
            }
            Some(b'/') => {
                self_closing = true;
                i += 1;
                continue;
            }
            _ => {}
        }
        let a = i;
        while i < s.len() && !s[i].is_ascii_whitespace() && !matches!(s[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let attr = String::from_utf8_lossy(&s[a..i]).to_ascii_lowercase();
        while i < s.len() && s[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = None;
        if s.get(i) == Some(&b'=') {
            i += 1;
            while i < s.len() && s[i].is_ascii_whitespace() {
                i += 1;
            }
            match s.get(i) {
                Some(&q) if q == b'"' || q == b'\'' => {
                    let end = memchr(q, &s[i + 1..])? + i + 1;
                    value = Some(s[i + 1..end].to_vec());
                    i = end + 1;
                }
                Some(_) => {
                    let v = i;
                    while i < s.len() && !s[i].is_ascii_whitespace() && s[i] != b'>' {
                        i += 1;
                    }
                    value = Some(s[v..i].to_vec());
                }
                None => return None,
            }
        }
        if !attr.is_empty() {
            self_closing = false;
            attrs.push((attr, value));
        }
    }
    Some(Tag {
        name,
        attrs,
        closing,
        self_closing,
        len: i,
    })
}

fn attr<'a>(tag: &'a Tag, name: &str) -> Option<&'a [u8]> {
    tag.attrs
        .iter()
        .find(|(n, _)| n == name)
        .and_then(|(_, v)| v.as_deref())
}

fn is_refresh(tag: &Tag) -> bool {
    attr(tag, "http-equiv").is_some_and(|v| v.eq_ignore_ascii_case(b"refresh"))
}

/// `<link rel=stylesheet href=local.css>` becomes an inline, sanitised
/// `<style>`; every other link (preload, icon, import…) is dropped.
fn safe_link(tag: &Tag, base: Option<&Path>) -> Option<Vec<u8>> {
    let rel = String::from_utf8_lossy(attr(tag, "rel")?).to_ascii_lowercase();
    if rel.split_ascii_whitespace().ne(["stylesheet"]) {
        return None;
    }
    let file = local_file(attr(tag, "href")?, base?)?;
    let css = std::fs::read(file).ok()?;
    let mut out = b"<style>".to_vec();
    let clean = sanitize_css(&css, base);
    // The stylesheet must not be able to close the element early.
    out.extend_from_slice(&replace_ci(&clean, b"</style", b"<\\/style"));
    out.extend_from_slice(b"</style>");
    Some(out)
}

fn rebuild(tag: &Tag, base: Option<&Path>) -> Vec<u8> {
    let mut out = vec![b'<'];
    out.extend_from_slice(tag.name.as_bytes());
    for (name, value) in &tag.attrs {
        if name.starts_with("on") || name == "srcset" || name == "imagesrcset" {
            continue;
        }
        let value: Option<Vec<u8>> = match (name.as_str(), value) {
            (_, None) => None,
            // The HTML parser decodes character references before CSS sees
            // the value (`u&#114;l(`, `url&lpar;`), and CSS has its own
            // escapes (`\75rl(`): such styles are dropped, not decoded.
            ("style", Some(v)) if v.contains(&b'&') || v.contains(&b'\\') => continue,
            ("style", Some(v)) => Some(sanitize_css(v, base)),
            ("href", Some(v)) => {
                // Links are not loaded when rendering; only script URLs go.
                if is_script_url(v) {
                    continue;
                }
                Some(v.clone())
            }
            (n, Some(v)) if LOADING_ATTRS.contains(&n) => match local_url(v, base) {
                Some(url) => Some(url.into_bytes()),
                None => continue,
            },
            (_, Some(v)) => Some(v.clone()),
        };
        out.push(b' ');
        out.extend_from_slice(name.as_bytes());
        if let Some(v) = value {
            out.extend_from_slice(b"=\"");
            out.extend_from_slice(&escape_attr(&v));
            out.push(b'"');
        }
    }
    if tag.self_closing && VOID.contains(&tag.name.as_str()) {
        out.push(b'/');
    }
    out.push(b'>');
    out
}

/// Elements that never have content (the only ones where `/>` is honoured).
const VOID: &[&str] = &[
    "area", "br", "col", "hr", "img", "input", "link", "meta", "wbr",
];

/// Start of the real `</name` end tag at or after `from`: like HTML, it only
/// counts when followed by whitespace, `/`, `>` or the end of input.
fn end_tag(html: &[u8], from: usize, name: &str) -> Option<usize> {
    let close = format!("</{name}");
    let mut j = from;
    while let Some(n) = find_ci(&html[j..], close.as_bytes()) {
        let k = j + n + close.len();
        if k >= html.len() || matches!(html[k], b'>' | b'/' | b' ' | b'\t' | b'\n' | b'\r' | 0x0c) {
            return Some(j + n);
        }
        j = k;
    }
    None
}

/// Rewrites `url(...)` to verified local files and disables `@import`,
/// `image-set()` and `expression()`.
fn sanitize_css(css: &[u8], base: Option<&Path>) -> Vec<u8> {
    // CSS escapes could spell `url(` or `@import` (`\75rl(`); drop them all.
    let css: Vec<u8> = css.iter().copied().filter(|b| *b != b'\\').collect();
    let css = css.as_slice();
    let mut out = Vec::with_capacity(css.len());
    let mut i = 0;
    while i < css.len() {
        let rest = &css[i..];
        if ascii_starts_ci(rest, b"@import") {
            out.extend_from_slice(b"@nashi-blocked");
            i += 7;
        } else if ascii_starts_ci(rest, b"image-set(") || ascii_starts_ci(rest, b"expression(") {
            out.extend_from_slice(b"none(");
            i += memchr(b'(', rest).unwrap_or(0) + 1;
        } else if ascii_starts_ci(rest, b"url(") {
            let close = memchr(b')', rest).unwrap_or(rest.len());
            let raw = trim(&rest[4..close]);
            let raw = raw
                .strip_prefix(b"\"")
                .and_then(|r| r.strip_suffix(b"\""))
                .or_else(|| raw.strip_prefix(b"'").and_then(|r| r.strip_suffix(b"'")))
                .unwrap_or(raw);
            out.extend_from_slice(b"url(\"");
            if let Some(url) = local_url(raw, base) {
                out.extend_from_slice(url.as_bytes());
            }
            out.extend_from_slice(b"\")");
            i += (close + 1).min(rest.len());
        } else {
            out.push(css[i]);
            i += 1;
        }
    }
    out
}

/// A reference that may be loaded, as a `data:` URL: an inline raster image,
/// or a raster image file inside `base`.
fn local_url(raw: &[u8], base: Option<&Path>) -> Option<String> {
    let raw = decode_entities(trim(raw));
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with(b"data:image/") {
        let mime = &lower[5..lower.iter().position(|b| *b == b';' || *b == b',')?];
        let ok = raster_mime(mime).is_some() && !raw.contains(&b'"') && !raw.contains(&b'\'');
        return ok.then(|| String::from_utf8_lossy(&raw).into_owned());
    }
    let file = local_file(&raw, base?)?;
    let ext = file.extension()?.to_str()?.to_ascii_lowercase();
    let mime = raster_mime(format!("image/{ext}").as_bytes())?;
    let bytes = std::fs::read(&file).ok()?;
    Some(format!("data:{mime};base64,{}", base64(&bytes)))
}

fn raster_mime(mime: &[u8]) -> Option<&'static str> {
    Some(match mime {
        b"image/png" => "image/png",
        b"image/jpeg" | b"image/jpg" => "image/jpeg",
        b"image/gif" => "image/gif",
        b"image/webp" => "image/webp",
        b"image/bmp" => "image/bmp",
        _ => return None,
    })
}

/// A relative path to a regular file inside `base` (symlinks resolved).
fn local_file(raw: &[u8], base: &Path) -> Option<PathBuf> {
    let raw = decode_entities(trim(raw));
    // Drop query and fragment, then refuse schemes and absolute paths.
    let path = raw.split(|b| *b == b'?' || *b == b'#').next()?;
    if path.is_empty()
        || path.starts_with(b"/")
        || path.starts_with(b"\\")
        || path.contains(&b':')
        || path.contains(&b'\\')
    {
        return None;
    }
    let decoded = percent_decode(path)?;
    use std::os::unix::ffi::OsStrExt;
    let candidate: PathBuf = base.join(std::ffi::OsStr::from_bytes(&decoded));
    let real = std::fs::canonicalize(&candidate).ok()?;
    let meta = std::fs::metadata(&real).ok()?;
    (real.starts_with(base) && meta.is_file() && meta.len() <= MAX_INLINE).then_some(real)
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for (k, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if k <= chunk.len() {
                out.push(T[(n >> shift & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn replace_ci(hay: &[u8], needle: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(hay.len());
    let mut i = 0;
    while i < hay.len() {
        if ascii_starts_ci(&hay[i..], needle) {
            out.extend_from_slice(with);
            i += needle.len();
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    out
}

fn is_script_url(v: &[u8]) -> bool {
    let v: Vec<u8> = decode_entities(trim(v))
        .into_iter()
        .filter(|b| !b.is_ascii_whitespace() && !b.is_ascii_control())
        .collect();
    let l = v.to_ascii_lowercase();
    l.starts_with(b"javascript:") || l.starts_with(b"vbscript:") || l.starts_with(b"data:")
}

fn percent_decode(s: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'%' {
            let hex = std::str::from_utf8(s.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    (!out.contains(&0)).then_some(out)
}

/// Decodes the few character references that can hide a URL scheme.
fn decode_entities(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'&' {
            if let Some(semi) = memchr(b';', &s[i..]).filter(|n| *n <= 10) {
                let ent = &s[i + 1..i + semi];
                let ch = match ent {
                    b"amp" => Some(b'&'),
                    b"colon" => Some(b':'),
                    b"sol" => Some(b'/'),
                    b"quot" => Some(b'"'),
                    b"apos" => Some(b'\''),
                    _ if ent.first() == Some(&b'#') => {
                        let num = &ent[1..];
                        let v = if num.first().is_some_and(|c| *c == b'x' || *c == b'X') {
                            u32::from_str_radix(std::str::from_utf8(&num[1..]).unwrap_or(""), 16)
                        } else {
                            std::str::from_utf8(num).unwrap_or("").parse()
                        };
                        v.ok()
                            .and_then(|v| u8::try_from(v).ok())
                            .filter(u8::is_ascii)
                    }
                    _ => None,
                };
                if let Some(c) = ch {
                    out.push(c);
                    i += semi + 1;
                    continue;
                }
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

fn escape_attr(v: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len());
    for &b in v {
        match b {
            b'"' => out.extend_from_slice(b"&quot;"),
            b'<' => out.extend_from_slice(b"&lt;"),
            b'>' => out.extend_from_slice(b"&gt;"),
            _ => out.push(b),
        }
    }
    out
}

/// Index after the element's matching close tag (or end of input).
fn skip_element(html: &[u8], from: usize, name: &str) -> usize {
    match end_tag(html, from, name) {
        Some(at) => memchr(b'>', &html[at..]).map_or(html.len(), |m| at + m + 1),
        None => html.len(),
    }
}

fn trim(s: &[u8]) -> &[u8] {
    let start = s
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(start, |e| e + 1);
    &s[start..end]
}

fn memchr(c: u8, s: &[u8]) -> Option<usize> {
    s.iter().position(|b| *b == c)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn find_ci(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle))
}

fn ascii_starts_ci(s: &[u8], prefix: &[u8]) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(html: &str, dir: &Path) -> String {
        String::from_utf8(sanitize(html.as_bytes(), dir)).unwrap()
    }

    #[test]
    fn drops_scripts_handlers_and_active_content() {
        let d = tempfile::tempdir().unwrap();
        let out = run(
            r#"<!doctype html><p onclick="x()">Hi<script>alert(1)</script><SCRIPT src=a.js></SCRIPT>
<iframe src="https://e.x"></iframe><object data=x></object><a href="javascript:alert(1)">j</a>
<a href="jav&#x61;script:x">k</a><a href="https://example.org">ok</a><!-- <img src=http://x> --></p>"#,
            d.path(),
        );
        assert!(out.starts_with("<!doctype html>"));
        for bad in [
            "script",
            "alert",
            "iframe",
            "object",
            "onclick",
            "javascript",
            "http://x",
        ] {
            assert!(!out.to_lowercase().contains(bad), "{bad} survived: {out}");
        }
        assert!(out.contains(r#"<a href="https://example.org">ok</a>"#));
        assert!(out.contains("<p>Hi"));
    }

    #[test]
    fn remote_and_outside_resources_are_blocked() {
        let root = tempfile::tempdir().unwrap();
        let page = root.path().join("site");
        std::fs::create_dir(&page).unwrap();
        std::fs::write(root.path().join("secret.png"), b"x").unwrap();
        std::fs::write(page.join("ok.png"), b"x").unwrap();
        std::os::unix::fs::symlink(root.path().join("secret.png"), page.join("link.png")).unwrap();
        let out = run(
            r#"<img src="http://127.0.0.1/a.png"><img src="//cdn/x.png"><img src="/etc/passwd">
<img src="../secret.png"><img src="link.png"><img src="%2e%2e/secret.png"><img src="ok.png?v=1">
<img srcset="ok.png 2x"><div style="background:url(http://e/x.png)"></div>
<link rel=stylesheet href="https://cdn/x.css"><link rel=preload href=ok.png><base href="http://e/">
<meta http-equiv="refresh" content="0;url=http://e"><style>@import url(x.css); p{background:url('ok.png')}</style>"#,
            &page,
        );
        assert!(!out.contains("http://") && !out.contains("//cdn"), "{out}");
        assert!(!out.contains("secret") && !out.contains("passwd"), "{out}");
        assert!(!out.contains("srcset") && !out.contains("<base") && !out.contains("refresh"));
        assert!(!out.contains("@import"));
        // ok.png ("x") is inlined for <img> and the style's url().
        assert_eq!(
            out.matches("data:image/png;base64,eA==").count(),
            2,
            "{out}"
        );
        assert!(!out.contains("<link"), "{out}");
    }

    #[test]
    fn local_stylesheet_and_data_images_are_kept() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("my style.css"),
            b"p{color:red}</style><script>x</script>",
        )
        .unwrap();
        std::fs::write(d.path().join("logo.svg"), b"<svg/>").unwrap();
        let out = run(
            r#"<link rel="stylesheet" href="my%20style.css"><img src="data:image/png;base64,AAAA"><img src="data:image/svg+xml,<svg>"><img src="logo.svg">"#,
            d.path(),
        );
        assert!(
            out.starts_with("<style>p{color:red}<\\/style><script>x</script></style>"),
            "{out}"
        );
        assert!(out.contains("data:image/png;base64,AAAA"));
        assert!(!out.contains("svg"), "{out}");
    }

    #[test]
    fn encoded_css_cannot_smuggle_urls() {
        let d = tempfile::tempdir().unwrap();
        let out = run(
            r#"<p style="background:u&#114;l(http://e/a.png)">a</p><p style="background:url&lpar;http://e/b.png)">b</p>
<p style="background:\75rl(http://e/c.png)">c</p><style>p{background:\75rl(http://e/d.png)} @\69mport "http://e/e.css";</style>"#,
            d.path(),
        );
        // Remote URLs may survive only as inert text: no escapes, no url()/@import.
        assert!(!out.contains('\\') && !out.contains("@import"), "{out}");
        assert!(
            !out.to_lowercase().contains("url(\"http") && !out.contains("url(http"),
            "{out}"
        );
        assert!(!out.contains("style=\""), "{out}");
        assert!(
            out.contains("<p>a</p>") && out.contains("<p>b</p>") && out.contains("<p>c</p>"),
            "{out}"
        );
    }

    #[test]
    fn self_closing_raw_text_and_fake_end_tags() {
        let d = tempfile::tempdir().unwrap();
        let out = run(
            r#"<style/>p{background:url(http://e/a.png)}</style><script/>evil()</script>
<style>p{}</styles>q{background:url(http://e/b.png)}</style><p>ok</p><br/>"#,
            d.path(),
        );
        assert!(!out.contains("http://e/") && !out.contains("evil"), "{out}");
        assert!(
            !out.contains("<style/>") && out.contains("<br/>") && out.contains("<p>ok</p>"),
            "{out}"
        );
        // An end tag the tag parser cannot read must still close the style.
        let out = run(
            r#"<style>p{}</style x="a>q{background:url(http://e/c.png)}"#,
            d.path(),
        );
        // The rest is inert body text, outside the (closed) style element.
        assert!(out.starts_with("<style>p{}</style>q{"), "{out}");
        let out = run("<style>p{background:url(http://e/d.png)}", d.path());
        assert!(
            out.ends_with("</style>") && !out.contains("http://e/"),
            "{out}"
        );
    }

    #[test]
    fn base64_matches_rfc4648() {
        for (i, o) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(i.as_bytes()), o);
        }
    }

    #[test]
    fn plain_text_and_stray_brackets_survive() {
        let d = tempfile::tempdir().unwrap();
        let out = run("<p>1 < 2 &amp; 3 > 2</p>", d.path());
        assert_eq!(out, "<p>1 &lt; 2 &amp; 3 > 2</p>");
    }
}

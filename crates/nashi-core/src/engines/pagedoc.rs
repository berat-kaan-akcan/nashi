//! EPUB and DOCX files that hold each page of a PDF as a picture. The layout stays
//! exactly as it was, and scanned PDFs with no text can be converted too.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use super::epub::escape;
use super::zipw::write_zip;

/// One rendered page: a JPEG and the size it is shown at, in points.
pub struct Page {
    pub jpeg: Vec<u8>,
    pub width_pt: f64,
    pub height_pt: f64,
}

/// Write a fixed-layout EPUB 3 with one document per page.
pub fn write_epub(path: &Path, title: &str, pages: &[Page]) -> std::io::Result<()> {
    let t = escape(title);
    let id = format!(
        "urn:nashi:pages:{:x}",
        pages.iter().map(|p| p.jpeg.len()).sum::<usize>()
    );
    let container = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>
"#;
    let mut manifest = String::new();
    let mut spine = String::new();
    let mut links = String::new();
    let mut documents = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let n = i + 1;
        let (w, h) = (page.width_pt.round() as u32, page.height_pt.round() as u32);
        manifest.push_str(&format!(
            "    <item id=\"p{n}\" href=\"p{n}.xhtml\" media-type=\"application/xhtml+xml\"/>\n    <item id=\"i{n}\" href=\"p{n}.jpg\" media-type=\"image/jpeg\"/>\n"
        ));
        spine.push_str(&format!("<itemref idref=\"p{n}\"/>"));
        links.push_str(&format!("<li><a href=\"p{n}.xhtml\">{n}</a></li>"));
        documents.push(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{t} {n}</title><meta name=\"viewport\" content=\"width={w}, height={h}\"/><style>html,body{{margin:0;padding:0}}img{{display:block;width:{w}px;height:{h}px}}</style></head><body><img src=\"p{n}.jpg\" alt=\"{n}\"/></body></html>\n"
        ));
    }
    // The date is required; a fixed one keeps the same PDF giving the same file.
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="bookid" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{t}</dc:title><dc:identifier id="bookid">{id}</dc:identifier><dc:language>und</dc:language>
    <meta property="dcterms:modified">2000-01-01T00:00:00Z</meta>
    <meta property="rendition:layout">pre-paginated</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
{manifest}  </manifest>
  <spine>{spine}</spine>
</package>
"#
    );
    let nav = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><title>{t}</title></head><body><nav epub:type=\"toc\"><ol>{links}</ol></nav></body></html>\n"
    );
    let names: Vec<(String, String)> = (1..=pages.len())
        .map(|n| (format!("OEBPS/p{n}.xhtml"), format!("OEBPS/p{n}.jpg")))
        .collect();
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
        ("OEBPS/nav.xhtml", nav.as_bytes()),
    ];
    for (i, (doc, image)) in names.iter().enumerate() {
        entries.push((doc, documents[i].as_bytes()));
        entries.push((image, &pages[i].jpeg));
    }
    write_zip(BufWriter::new(File::create(path)?), &entries)
}

/// Section properties for a page with no margins.
fn section(page: &Page) -> String {
    let (w, h) = (
        (page.width_pt * 20.0).round() as u64,
        (page.height_pt * 20.0).round() as u64,
    );
    let orient = if w > h {
        r#" w:orient="landscape""#
    } else {
        ""
    };
    format!(
        r#"<w:sectPr><w:pgSz w:w="{w}" w:h="{h}"{orient}/><w:pgMar w:top="0" w:right="0" w:bottom="0" w:left="0" w:header="0" w:footer="0" w:gutter="0"/></w:sectPr>"#
    )
}

/// Write a DOCX with each page as a picture filling a section of its own size.
pub fn write_docx(path: &Path, pages: &[Page]) -> std::io::Result<()> {
    let mut body = String::new();
    let mut rels = String::new();
    for (i, page) in pages.iter().enumerate() {
        let n = i + 1;
        let (cx, cy) = (
            (page.width_pt * 12700.0).round() as u64,
            (page.height_pt * 12700.0).round() as u64,
        );
        // Word keeps a section's properties in its last paragraph, and those of the
        // last section at the end of the body.
        let inner = if n < pages.len() {
            section(page)
        } else {
            String::new()
        };
        body.push_str(&format!(
            r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/>{inner}</w:pPr><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{n}" name="Page {n}"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{n}" name="Page {n}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rId{n}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
        ));
        rels.push_str(&format!(
            r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/p{n}.jpg"/>"#
        ));
    }
    if let Some(last) = pages.last() {
        body.push_str(&section(last));
    }
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>{body}</w:body></w:document>
"#
    );
    let types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="jpg" ContentType="image/jpeg"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>
"#;
    let root = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>
"#;
    let doc_rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}</Relationships>\n"
    );
    let names: Vec<String> = (1..=pages.len())
        .map(|n| format!("word/media/p{n}.jpg"))
        .collect();
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", root.as_bytes()),
        ("word/document.xml", document.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
    ];
    for (i, name) in names.iter().enumerate() {
        entries.push((name, &pages[i].jpeg));
    }
    write_zip(BufWriter::new(File::create(path)?), &entries)
}

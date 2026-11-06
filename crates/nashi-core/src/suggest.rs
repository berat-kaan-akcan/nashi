//! Suggested redaction areas (`nashi-cli suggest`): read the text in a picture
//! or a video frame with `tesseract` and mark what looks like personal data,
//! and find faces with OpenCV (through `python3`).
//!
//! Only the kind and the place of a match leave this module, never the text.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::catalog::Category;
use crate::deps::{require, Deps};
use crate::detect::detect;
use crate::events::{JobError, JobResult};
use crate::fsops::WorkDir;
use crate::preview::{self, PreviewKind, PreviewRequest};
use crate::process::{check_cancel, failure, Cmd};

/// What `suggest` can look for, in the order they are tried.
pub const KINDS: &[&str] = &["iban", "email", "url", "card", "id", "phone", "face"];
/// The kinds that need `tesseract`; the rest need OpenCV.
const TEXT_KINDS: &[&str] = &["iban", "email", "url", "card", "id", "phone"];

/// Runs under `python3 -I -c`; argument: a PNG. Prints `[[x, y, w, h], ...]`.
/// Exit 3 when OpenCV's face model files cannot be found.
const FACE_SCRIPT: &str = r#"
import sys, json, os
import cv2
name = "haarcascade_frontalface_default.xml"
dirs = ["/usr/share/opencv5/haarcascades", "/usr/share/opencv4/haarcascades",
        "/usr/share/OpenCV/haarcascades", "/usr/local/share/opencv4/haarcascades"]
found = [d for d in dirs if os.path.isfile(os.path.join(d, name))]
if not found:
    sys.stderr.write("OpenCV face model files not found\n")
    sys.exit(3)
img = cv2.imread(sys.argv[1], cv2.IMREAD_GRAYSCALE)
if img is None:
    sys.stderr.write("cannot read the picture\n")
    sys.exit(2)
h, w = img.shape
c = cv2.CascadeClassifier(os.path.join(found[0], name))
m = max(24, int(min(w, h) * 0.03))
r = c.detectMultiScale(cv2.equalizeHist(img), scaleFactor=1.1, minNeighbors=5, minSize=(m, m))
print(json.dumps([[int(x), int(y), int(a), int(b)] for x, y, a, b in r]))
"#;

const DEFAULT_LANG: &str = "tur+eng";
/// Space added around a match so that the whole of the glyphs is covered.
const PAD: i64 = 3;

#[derive(Debug, Clone)]
pub struct SuggestRequest {
    pub file: PathBuf,
    /// Video only: the second to read.
    pub at: Option<f64>,
    /// Comma-separated subset of [`KINDS`]; all of them when absent.
    pub kinds: Option<String>,
    /// Tesseract languages joined with `+`; the ones that are installed are used.
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SuggestRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestOut {
    /// Size of the picture the rectangles refer to (after orientation).
    pub width: u32,
    pub height: u32,
    pub language: String,
    pub rects: Vec<SuggestRect>,
}

/// One word the OCR found, with its place and its line.
#[derive(Debug, Clone)]
pub(crate) struct Word {
    line: (u32, u32, u32),
    text: String,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
}

pub fn run(deps: &Deps, req: &SuggestRequest) -> JobResult<SuggestOut> {
    check_cancel()?;
    let explicit = req.kinds.as_deref().is_some_and(|k| !k.trim().is_empty());
    let mut kinds = parse_kinds(req.kinds.as_deref())?;
    if !explicit {
        // Without a list, look for whatever the installed tools can find.
        kinds.retain(|k| {
            if *k == "face" {
                deps.has("opencv")
            } else {
                deps.has("tesseract")
            }
        });
    }
    let read_text = kinds.iter().any(|k| TEXT_KINDS.contains(k));
    let find_faces = kinds.contains(&"face");
    let tess = if read_text || kinds.is_empty() {
        Some(require(deps, "tesseract")?)
    } else {
        None
    };
    let python = if find_faces {
        Some(require(deps, "opencv")?)
    } else {
        None
    };
    let file = std::path::absolute(&req.file).unwrap_or_else(|_| req.file.clone());
    let meta = std::fs::metadata(&file)
        .map_err(|e| JobError::invalid_input(format!("{}: {e}", file.display())))?;
    if meta.is_dir() {
        return Err(JobError::invalid_input(format!(
            "{} is a directory",
            file.display()
        )));
    }
    match detect(&file).category {
        Category::Image | Category::Video | Category::Gif => {}
        _ => {
            return Err(JobError::invalid_input(
                "suggest reads pictures and videos only",
            ))
        }
    }
    let language = match tess.filter(|_| read_text) {
        Some(t) => pick_languages(t, req.lang.as_deref())?,
        None => String::new(),
    };

    let work = WorkDir::system()?;
    // The largest size the preview allows: the frame is not scaled down, so
    // the rectangles are in the pixels the redact tools work in.
    let frame = preview::run(
        deps,
        &PreviewRequest {
            file: file.clone(),
            kind: PreviewKind::Frame,
            out_dir: work.path().to_path_buf(),
            at: req.at,
            width: Some(16384),
            height: Some(16384),
            pages: None,
            color: None,
            adjust: None,
        },
    )?;
    let png = frame
        .files
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| JobError::tool_failed("no frame to read"))?;
    check_cancel()?;
    let mut rects = Vec::new();
    if let Some(tess) = tess.filter(|_| read_text) {
        let out = Cmd::new(tess)
            .arg(&png)
            .arg("stdout")
            .args(["-l", &language])
            .arg("tsv")
            .env("OMP_THREAD_LIMIT", "1")
            .run()?;
        if !out.status.success() {
            return Err(failure("tesseract", &out));
        }
        let words = parse_tsv(&out.stdout);
        rects = find(&words, &kinds, frame.width, frame.height);
    }
    if let Some(python) = python {
        check_cancel()?;
        let out = Cmd::new(python)
            .args(["-I", "-c", FACE_SCRIPT])
            .arg(&png)
            .run()?;
        if !out.status.success() {
            return Err(failure("OpenCV", &out));
        }
        rects.extend(parse_faces(&out.stdout, frame.width, frame.height)?);
    }
    Ok(SuggestOut {
        width: frame.width,
        height: frame.height,
        language,
        rects,
    })
}

/// Face boxes from the helper script, widened a little so hair and chin are covered.
fn parse_faces(json: &str, pw: u32, ph: u32) -> JobResult<Vec<SuggestRect>> {
    let boxes: Vec<[i64; 4]> = serde_json::from_str(json.trim())
        .map_err(|_| JobError::tool_failed("OpenCV gave an unreadable answer"))?;
    Ok(boxes
        .into_iter()
        .map(|[x, y, w, h]| {
            let (gx, gy) = (w * 12 / 100, h * 12 / 100);
            let word = Word {
                line: (0, 0, 0),
                text: String::new(),
                x: x - gx,
                y: y - gy,
                w: w + 2 * gx,
                h: h + 2 * gy,
            };
            boxed(&[word], "face", pw, ph)
        })
        .collect())
}

fn parse_kinds(list: Option<&str>) -> JobResult<Vec<&'static str>> {
    let Some(list) = list.map(str::trim).filter(|l| !l.is_empty()) else {
        return Ok(KINDS.to_vec());
    };
    let mut kinds = Vec::new();
    for name in list.split(',') {
        let name = name.trim();
        match KINDS.iter().find(|k| **k == name) {
            Some(k) if !kinds.contains(k) => kinds.push(*k),
            Some(_) => {}
            None => {
                return Err(JobError::invalid_options(format!(
                    "unknown kind '{name}'; use {}",
                    KINDS.join(", ")
                )))
            }
        }
    }
    Ok(KINDS
        .iter()
        .copied()
        .filter(|k| kinds.contains(k))
        .collect())
}

/// The requested languages that are installed, joined for tesseract.
fn pick_languages(tess: &Path, wanted: Option<&str>) -> JobResult<String> {
    let listing = Cmd::new(tess).arg("--list-langs").run()?;
    // Older versions print the list on stderr.
    let all = format!("{}\n{}", listing.stdout, listing.stderr);
    let installed: Vec<&str> = all
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains(' ') && !l.contains(':') && *l != "osd")
        .collect();
    let requested = wanted.unwrap_or(DEFAULT_LANG);
    let mut picked = Vec::new();
    for lang in requested.split('+') {
        if lang.is_empty() || !lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(JobError::invalid_options(format!(
                "invalid language '{lang}'"
            )));
        }
        if installed.contains(&lang) && !picked.contains(&lang) {
            picked.push(lang);
        }
    }
    if picked.is_empty() {
        if wanted.is_some() {
            return Err(JobError::invalid_options(format!(
                "no OCR data for '{requested}' is installed"
            )));
        }
        match installed.first() {
            Some(first) => picked.push(first),
            None => {
                return Err(JobError::missing(
                    "tesseract-data-eng",
                    "tesseract-data-eng",
                ))
            }
        }
    }
    Ok(picked.join("+"))
}

/// Words of tesseract's TSV output, in reading order.
pub(crate) fn parse_tsv(tsv: &str) -> Vec<Word> {
    tsv.lines()
        .skip(1)
        .filter_map(|row| {
            let f: Vec<&str> = row.splitn(12, '\t').collect();
            if f.len() < 12 || f[0] != "5" {
                return None;
            }
            let text = f[11].trim();
            let conf: f64 = f[10].parse().ok()?;
            if text.is_empty() || conf < 0.0 {
                return None;
            }
            Some(Word {
                line: (f[2].parse().ok()?, f[3].parse().ok()?, f[4].parse().ok()?),
                text: text.to_string(),
                x: f[6].parse().ok()?,
                y: f[7].parse().ok()?,
                w: f[8].parse().ok()?,
                h: f[9].parse().ok()?,
            })
        })
        .collect()
}

/// Rectangles for everything in `words` that looks like one of `kinds`.
pub(crate) fn find(words: &[Word], kinds: &[&'static str], pw: u32, ph: u32) -> Vec<SuggestRect> {
    let mut rects = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let mut end = start;
        while end < words.len() && words[end].line == words[start].line {
            end += 1;
        }
        let line = &words[start..end];
        for (first, last, kind) in line_matches(line, kinds) {
            rects.push(boxed(&line[first..=last], kind, pw, ph));
        }
        start = end;
    }
    rects
}

fn boxed(words: &[Word], kind: &'static str, pw: u32, ph: u32) -> SuggestRect {
    let x0 = words.iter().map(|w| w.x).min().unwrap_or(0) - PAD;
    let y0 = words.iter().map(|w| w.y).min().unwrap_or(0) - PAD;
    let x1 = words.iter().map(|w| w.x + w.w).max().unwrap_or(0) + PAD;
    let y1 = words.iter().map(|w| w.y + w.h).max().unwrap_or(0) + PAD;
    let (x0, y0) = (x0.clamp(0, pw as i64), y0.clamp(0, ph as i64));
    let (x1, y1) = (x1.clamp(0, pw as i64), y1.clamp(0, ph as i64));
    SuggestRect {
        x: x0 as u32,
        y: y0 as u32,
        w: (x1 - x0).max(1) as u32,
        h: (y1 - y0).max(1) as u32,
        kind,
    }
}

/// `(first word, last word, kind)` for each match in one line.
fn line_matches(line: &[Word], kinds: &[&'static str]) -> Vec<(usize, usize, &'static str)> {
    let on = |k: &str| kinds.contains(&k);
    let mut used = vec![false; line.len()];
    let mut found = Vec::new();
    if on("iban") {
        let mut i = 0;
        while i < line.len() {
            if let Some(last) = iban_at(line, i).filter(|_| !used[i]) {
                (i..=last).for_each(|k| used[k] = true);
                found.push((i, last, "iban"));
                i = last + 1;
            } else {
                i += 1;
            }
        }
    }
    for (i, word) in line.iter().enumerate() {
        if used[i] {
            continue;
        }
        let core = word
            .text
            .trim_matches(|c: char| "<>()[]{}\"',;:".contains(c));
        let kind = if on("email") && is_email(core) {
            "email"
        } else if on("url") && is_url(core) {
            "url"
        } else {
            continue;
        };
        used[i] = true;
        found.push((i, i, kind));
    }
    let mut i = 0;
    while i < line.len() {
        if used[i] || !numeric(&line[i].text) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < line.len() && !used[j + 1] && numeric(&line[j + 1].text) {
            j += 1;
        }
        let digits: String = line[i..=j]
            .iter()
            .flat_map(|w| w.text.chars())
            .filter(char::is_ascii_digit)
            .collect();
        let single = i == j;
        let kind = if on("card") && (13..=19).contains(&digits.len()) && luhn(&digits) {
            Some("card")
        } else if on("id") && single && digits.len() == 11 && turkish_id(&digits) {
            Some("id")
        } else if on("phone") && (10..=15).contains(&digits.len()) {
            Some("phone")
        } else {
            None
        };
        if let Some(kind) = kind {
            found.push((i, j, kind));
        }
        i = j + 1;
    }
    found.sort_unstable();
    found
}

/// A word made of digits and the marks used inside phone and card numbers.
fn numeric(word: &str) -> bool {
    word.chars().any(|c| c.is_ascii_digit())
        && word
            .chars()
            .all(|c| c.is_ascii_digit() || "+-().,/".contains(c))
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && local
            .chars()
            .all(|c| c.is_alphanumeric() || "._%+-".contains(c))
        && domain
            .chars()
            .all(|c| c.is_alphanumeric() || ".-".contains(c))
}

fn is_url(s: &str) -> bool {
    let l = s.to_lowercase();
    ["http://", "https://", "www."].iter().any(|p| {
        l.strip_prefix(p)
            .is_some_and(|rest| rest.chars().any(char::is_alphanumeric))
    })
}

fn luhn(digits: &str) -> bool {
    let sum: u32 = digits
        .chars()
        .rev()
        .filter_map(|c| c.to_digit(10))
        .enumerate()
        .map(|(i, d)| {
            if i % 2 == 1 {
                let x = d * 2;
                if x > 9 {
                    x - 9
                } else {
                    x
                }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// The checksums of a Turkish identity number.
fn turkish_id(digits: &str) -> bool {
    let d: Vec<i32> = digits
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|x| x as i32)
        .collect();
    if d.len() != 11 || d[0] == 0 {
        return false;
    }
    let odd: i32 = d[0] + d[2] + d[4] + d[6] + d[8];
    let even: i32 = d[1] + d[3] + d[5] + d[7];
    (odd * 7 - even).rem_euclid(10) == d[9] && d[..10].iter().sum::<i32>().rem_euclid(10) == d[10]
}

/// The last word of an IBAN that starts at `start`, if one does.
fn iban_at(line: &[Word], start: usize) -> Option<usize> {
    let head: Vec<char> = line[start].text.chars().collect();
    if head.len() < 4
        || !head[0].is_ascii_uppercase()
        || !head[1].is_ascii_uppercase()
        || !head[2].is_ascii_digit()
        || !head[3].is_ascii_digit()
    {
        return None;
    }
    let mut joined = String::new();
    for (j, word) in line.iter().enumerate().skip(start) {
        if !word.text.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        joined.push_str(&word.text.to_ascii_uppercase());
        if joined.len() > 34 {
            return None;
        }
        if joined.len() >= 15 && iban_valid(&joined) {
            return Some(j);
        }
    }
    None
}

fn iban_valid(iban: &str) -> bool {
    let b = iban.as_bytes();
    let mut rest = 0u32;
    for &c in b[4..].iter().chain(&b[..4]) {
        let v = match c {
            b'0'..=b'9' => (c - b'0') as u32,
            b'A'..=b'Z' => (c - b'A') as u32 + 10,
            _ => return false,
        };
        rest = if v > 9 { rest * 100 + v } else { rest * 10 + v } % 97;
    }
    rest == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(texts: &[&str]) -> Vec<Word> {
        texts
            .iter()
            .enumerate()
            .map(|(i, t)| Word {
                line: (1, 1, 1),
                text: (*t).into(),
                x: i as i64 * 100,
                y: 10,
                w: 90,
                h: 20,
            })
            .collect()
    }

    fn kinds_in(texts: &[&str]) -> Vec<(usize, usize, &'static str)> {
        line_matches(&line(texts), KINDS)
    }

    #[test]
    fn finds_email_and_url() {
        assert_eq!(
            kinds_in(&["mail", "ayse.k@mail.com,", "www.example.org"]),
            [(1, 1, "email"), (2, 2, "url")]
        );
        assert!(kinds_in(&["a@b", "@", "mail.com"]).is_empty());
    }

    #[test]
    fn finds_card_with_luhn_and_ignores_other_long_numbers() {
        assert_eq!(
            kinds_in(&["4111", "1111", "1111", "1111"]),
            [(0, 3, "card")]
        );
        // Right length, wrong checksum: a phone-sized number at most.
        assert!(!kinds_in(&["4111", "1111", "1111", "1112"])
            .iter()
            .any(|m| m.2 == "card"));
    }

    #[test]
    fn finds_phone_numbers_in_pieces() {
        assert_eq!(
            kinds_in(&["Tel:", "+90", "532", "123", "45", "67"]),
            [(1, 5, "phone")]
        );
        assert!(kinds_in(&["12.05.2024"]).is_empty());
    }

    #[test]
    fn finds_turkish_id_by_checksum() {
        assert_eq!(kinds_in(&["10000000146"]), [(0, 0, "id")]);
        assert!(!kinds_in(&["10000000147"]).iter().any(|m| m.2 == "id"));
    }

    #[test]
    fn finds_iban_across_words() {
        assert_eq!(
            kinds_in(&["GB82", "WEST", "1234", "5698", "7654", "32"]),
            [(0, 5, "iban")]
        );
        // A wrong checksum is no IBAN; the digits may still look like a phone number.
        assert!(!kinds_in(&["GB83", "WEST", "1234", "5698", "7654", "32"])
            .iter()
            .any(|m| m.2 == "iban"));
    }

    #[test]
    fn only_asked_kinds_are_found() {
        let words = line(&["ayse@mail.com", "+90", "532", "123", "45", "67"]);
        let m = line_matches(&words, &["email"]);
        assert_eq!(m, [(0, 0, "email")]);
        assert!(parse_kinds(Some("email,nope")).is_err());
        assert_eq!(
            parse_kinds(Some("phone,email")).unwrap(),
            ["email", "phone"]
        );
    }

    #[test]
    fn rectangles_cover_the_words_and_stay_inside_the_picture() {
        let words = line(&["x", "ayse@mail.com"]);
        let r = find(&words, KINDS, 150, 28);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].x, r[0].y), (97, 7));
        assert_eq!((r[0].x + r[0].w, r[0].y + r[0].h), (150, 28));
        assert_eq!(r[0].kind, "email");
    }

    #[test]
    fn reads_tesseract_tsv() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   4\t1\t1\t1\t1\t0\t5\t5\t50\t10\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t5\t5\t50\t10\t96.5\ta@b.co\n";
        let w = parse_tsv(tsv);
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].x, w[0].w, w[0].text.as_str()), (5, 50, "a@b.co"));
    }
}

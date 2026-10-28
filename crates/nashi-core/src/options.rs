//! `--options` JSON. Unknown keys are ignored; every field is optional.

use serde::Deserialize;

use crate::events::{JobError, JobResult};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub t0: Option<f64>,
    #[serde(default)]
    pub t1: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Resize {
    #[serde(default)]
    pub w: Option<u32>,
    #[serde(default)]
    pub h: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Border {
    #[serde(default = "default_border_px")]
    pub px: u32,
    #[serde(default = "default_border_color")]
    pub color: String,
}

fn default_border_px() -> u32 {
    40
}
fn default_border_color() -> String {
    "#ffffff".into()
}

impl Default for Border {
    fn default() -> Self {
        Border {
            px: default_border_px(),
            color: default_border_color(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Options {
    pub quality: Option<u32>,
    pub crf: Option<u32>,
    pub pdf_preset: Option<String>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub speed: Option<f64>,
    pub rect: Option<Rect>,
    pub pages: Option<String>,
    pub channels: Option<String>,
    pub fps: Option<f64>,
    pub segment_seconds: Option<f64>,
    pub rotate: Option<f64>,
    pub flip: Option<String>,
    pub resize: Option<Resize>,
    pub border: Option<Border>,
    pub rects: Option<Vec<Rect>>,
    pub ranges: Option<Vec<[f64; 2]>>,
}

impl Options {
    /// Parse `--options` JSON (`None`/empty → defaults).
    pub fn parse(json: Option<&str>) -> JobResult<Options> {
        match json.map(str::trim) {
            None | Some("") => Ok(Options::default()),
            Some(s) => {
                let o: Options = serde_json::from_str(s)
                    .map_err(|e| JobError::invalid_options(format!("--options: {e}")))?;
                o.validate()?;
                Ok(o)
            }
        }
    }

    fn validate(&self) -> JobResult<()> {
        let bad = |m: &str| Err(JobError::invalid_options(m.to_string()));
        if let Some(q) = self.quality {
            if !(1..=100).contains(&q) {
                return bad("quality must be 1..100");
            }
        }
        if let Some(c) = self.crf {
            if c > 63 {
                return bad("crf must be 0..63");
            }
        }
        if let Some(p) = &self.pdf_preset {
            if !["screen", "ebook", "printer", "prepress", "default"].contains(&p.as_str()) {
                return bad("pdf_preset must be screen, ebook, printer, prepress or default");
            }
        }
        for v in [self.start, self.end].into_iter().flatten() {
            if !v.is_finite() || v < 0.0 {
                return bad("start/end must be >= 0");
            }
        }
        if let (Some(s), Some(e)) = (self.start, self.end) {
            if e <= s {
                return bad("end must be greater than start");
            }
        }
        if let Some(s) = self.speed {
            if !(0.25..=4.0).contains(&s) {
                return bad("speed must be 0.25..4");
            }
        }
        if let Some(c) = &self.channels {
            if c != "mono" && c != "stereo" {
                return bad("channels must be mono or stereo");
            }
        }
        if let Some(f) = self.fps {
            if !(f > 0.0 && f <= 60.0) {
                return bad("fps must be in (0, 60]");
            }
        }
        if let Some(s) = self.segment_seconds {
            if !(s.is_finite() && s >= 1.0) {
                return bad("segment_seconds must be >= 1");
            }
        }
        if let Some(r) = self.rotate {
            if !r.is_finite() {
                return bad("rotate must be a number");
            }
        }
        if let Some(f) = &self.flip {
            if f != "horizontal" && f != "vertical" {
                return bad("flip must be horizontal or vertical");
            }
        }
        if let Some(b) = &self.border {
            if !valid_color(&b.color) {
                return bad("border.color must be #rgb, #rrggbb(aa) or a color name");
            }
        }
        for r in self.rect.iter().chain(self.rects.iter().flatten()) {
            if ![r.x, r.y, r.w, r.h]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0)
                || r.w <= 0.0
                || r.h <= 0.0
            {
                return bad("rect values must be >= 0 with positive w/h");
            }
        }
        for [a, b] in self.ranges.iter().flatten() {
            if !(a.is_finite() && b.is_finite() && *a >= 0.0 && b > a) {
                return bad("ranges must be [start, end] with end > start >= 0");
            }
        }
        if let Some(p) = &self.pages {
            parse_pages(p, None)?;
        }
        Ok(())
    }
}

pub fn valid_color(c: &str) -> bool {
    if let Some(hex) = c.strip_prefix('#') {
        [3, 4, 6, 8].contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit())
    } else {
        !c.is_empty() && c.len() <= 32 && c.chars().all(|c| c.is_ascii_alphabetic())
    }
}

/// Parse a page list like `1-3,5` into 1-based page numbers (order kept).
pub fn parse_pages(spec: &str, count: Option<u32>) -> JobResult<Vec<u32>> {
    let err = || JobError::invalid_options(format!("invalid page list '{spec}'"));
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => {
                let a: u32 = a.trim().parse().map_err(|_| err())?;
                let b: u32 = if b.trim().is_empty() {
                    count.ok_or_else(err)?
                } else {
                    b.trim().parse().map_err(|_| err())?
                };
                (a, b)
            }
            None => {
                let a: u32 = part.parse().map_err(|_| err())?;
                (a, a)
            }
        };
        if a == 0 || b == 0 {
            return Err(err());
        }
        if a <= b {
            out.extend(a..=b);
        } else {
            out.extend((b..=a).rev());
        }
    }
    if out.is_empty() {
        return Err(err());
    }
    if let Some(n) = count {
        if let Some(p) = out.iter().find(|p| **p > n) {
            return Err(JobError::invalid_options(format!(
                "page {p} is beyond the last page ({n})"
            )));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_and_unknown_keys() {
        let o = Options::parse(Some(r#"{"quality":50,"whatever":1}"#)).unwrap();
        assert_eq!(o.quality, Some(50));
        assert!(Options::parse(None).unwrap().crf.is_none());
        assert!(Options::parse(Some(r#"{"quality":500}"#)).is_err());
        assert!(Options::parse(Some("not json")).is_err());
        let o = Options::parse(Some(
            r#"{"rect":{"x":0,"y":0,"w":10,"h":10},"border":{"px":5}}"#,
        ))
        .unwrap();
        assert_eq!(o.border.unwrap().color, "#ffffff");
    }

    #[test]
    fn pages() {
        assert_eq!(parse_pages("1-3,5", None).unwrap(), vec![1, 2, 3, 5]);
        assert_eq!(parse_pages("3,1", Some(3)).unwrap(), vec![3, 1]);
        assert_eq!(parse_pages("2-", Some(4)).unwrap(), vec![2, 3, 4]);
        assert!(parse_pages("0", None).is_err());
        assert!(parse_pages("9", Some(3)).is_err());
        assert!(parse_pages("a", None).is_err());
    }

    #[test]
    fn colors() {
        assert!(valid_color("#fff"));
        assert!(valid_color("white"));
        assert!(!valid_color("#ggg"));
        assert!(!valid_color("red; rm"));
    }
}

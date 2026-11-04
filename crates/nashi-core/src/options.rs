//! `--options` JSON. Unknown keys are ignored; every field is optional.

use serde::Deserialize;

use crate::events::{JobError, JobResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactStyle {
    Black,
    Blur,
    Pixelate,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub t0: Option<f64>,
    #[serde(default)]
    pub t1: Option<f64>,
    #[serde(default)]
    pub style: Option<String>,
}

impl Rect {
    pub fn redact_style(&self) -> JobResult<RedactStyle> {
        match self.style.as_deref() {
            None | Some("black") => Ok(RedactStyle::Black),
            Some("blur") => Ok(RedactStyle::Blur),
            Some("pixelate") => Ok(RedactStyle::Pixelate),
            Some(other) => Err(JobError::invalid_options(format!(
                "invalid redact style '{other}' (expected black, blur or pixelate)"
            ))),
        }
    }
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

/// A point as a fraction of the free space, 0..=1 on each axis.
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// `frameImage` layout. Every field is optional.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Frame {
    /// `solid` (default), `gradient` or `image`.
    pub backdrop: Option<String>,
    pub color: Option<String>,
    pub color2: Option<String>,
    /// Gradient direction in degrees.
    pub angle: Option<f64>,
    pub background_image: Option<std::path::PathBuf>,
    /// 0..=100.
    pub background_blur: Option<f64>,
    /// Space around the image, in pixels (default 40).
    pub padding: Option<u32>,
    pub corner_radius: Option<f64>,
    /// Shadow softness in pixels, 0..=200.
    pub shadow: Option<f64>,
    /// 0..=100 (default 30).
    pub shadow_opacity: Option<f64>,
    /// Canvas width over height, 0.1..=10.
    pub aspect: Option<f64>,
    /// Image size against the original, greater than 0 up to 1.
    pub image_scale: Option<f64>,
    pub position: Option<Position>,
}

/// `createCollage` layout. Every field is optional.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Collage {
    /// `grid` (default), `row`, `column` or `featured`.
    pub layout: Option<String>,
    pub columns: Option<u32>,
    /// Width in pixels (default 2048).
    pub width: Option<u32>,
    /// Cell width over height (default 1).
    pub aspect: Option<f64>,
    pub spacing: Option<u32>,
    pub padding: Option<u32>,
    pub corner_radius: Option<f64>,
    pub color: Option<String>,
    /// `fill` (default) crops to the cell, `fit` shows the whole image.
    pub fit: Option<String>,
    /// The inputs' indices in display order.
    pub order: Option<Vec<usize>>,
}

/// `editImage` picture adjustments. A missing value leaves that control alone.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Adjust {
    /// Stops, -5..=5.
    pub exposure: Option<f64>,
    /// -100..=100.
    pub brightness: Option<f64>,
    /// -100..=100.
    pub contrast: Option<f64>,
    /// 0.1..=5, where 1 changes nothing.
    pub gamma: Option<f64>,
    /// -100..=100; -100 removes all colour.
    pub saturation: Option<f64>,
    /// -100..=100.
    pub hue: Option<f64>,
    /// -100..=100, cooler to warmer.
    pub temperature: Option<f64>,
    /// 0..=100.
    pub sharpen: Option<f64>,
    /// 0..=100.
    pub blur: Option<f64>,
    /// 0..=100.
    pub grain: Option<f64>,
    /// `grayscale` or `sepia`.
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Options {
    /// `frameImage`: canvas, background and placement.
    pub frame: Option<Frame>,
    /// `createCollage`: layout and appearance.
    pub collage: Option<Collage>,
    /// `editImage`: picture adjustments.
    pub adjust: Option<Adjust>,
    /// `removeMetadata`: set these fields instead of removing everything.
    pub metadata_set: Option<std::collections::BTreeMap<String, String>>,
    /// `removeMetadata`: remove only these fields.
    pub metadata_remove: Option<Vec<String>>,
    /// `splitVideo`: cut at these times, in seconds, in any order.
    pub split_points: Option<Vec<f64>>,
    /// `splitVideo`: cut into this many equal clips (2..=10000).
    pub equal_clips: Option<u32>,
    /// `splitVideo`: zero-based clips to leave out.
    pub discard_clips: Option<Vec<usize>>,
    /// `splitVideo`: export the kept clips as one video.
    pub join_clips: Option<bool>,
    /// `videoSnapshots`: save the frames at these times, in seconds.
    pub snapshot_times: Option<Vec<f64>>,
    /// `compress`: the largest size the output may have.
    pub target_bytes: Option<u64>,
    /// `compress`: reduce by this percentage of the input (0 < p < 100).
    pub percentage: Option<f64>,
    /// `compress` images: shrink so the longest edge is at most this many pixels.
    pub max_edge: Option<u32>,
    /// `compress` images: resize each to this fraction of its own size (0 < s <= 1).
    pub image_scale: Option<f64>,
    /// `compress` video: shrink to at most this height.
    pub max_height: Option<u32>,
    /// `compress` lossy audio: bitrate in kbit/s.
    pub audio_kbps: Option<u32>,
    /// `trimAudio`: also remove silence from both ends of the selection.
    pub trim_silence: Option<bool>,
    /// `trimAudio`: what counts as silence, in dB (default -50).
    pub silence_threshold_db: Option<f64>,
    /// `redactAudio`: `mute` (default) or `bleep`.
    pub audio_mode: Option<String>,
    /// PDF → image resolution, 36..=1200 (default 150).
    pub dpi: Option<u32>,
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
    pub overlay: Option<String>,
    pub rotations: Option<Vec<i32>>,
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

/// Normalise clockwise page rotations (multiples of 90) to `0 | 90 | 180 | 270`,
/// checking that `rotations.len() == expected_len`.
pub fn normalize_rotations(rotations: &[i32], expected_len: usize) -> JobResult<Vec<i32>> {
    if rotations.len() != expected_len {
        return Err(JobError::invalid_options(format!(
            "rotations length ({}) must match output page count ({expected_len})",
            rotations.len()
        )));
    }
    let mut out = Vec::with_capacity(rotations.len());
    for &r in rotations {
        if r % 90 != 0 {
            return Err(JobError::invalid_options(format!(
                "rotation {r} must be a multiple of 90"
            )));
        }
        out.push(r.rem_euclid(360));
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

    #[test]
    fn rotations_and_redact_styles() {
        assert_eq!(
            normalize_rotations(&[0, 90, -90, 450, -180], 5).unwrap(),
            vec![0, 90, 270, 90, 180]
        );
        assert!(normalize_rotations(&[0, 90], 3).is_err());
        assert!(normalize_rotations(&[45], 1).is_err());

        let o = Options::parse(Some(
            r#"{"overlay":"/tmp/ov.png","rotations":[0,90],"rects":[{"x":0,"y":0,"w":10,"h":10,"style":"blur"},{"x":0,"y":0,"w":5,"h":5}]}"#,
        ))
        .unwrap();
        assert_eq!(o.overlay.as_deref(), Some("/tmp/ov.png"));
        assert_eq!(o.rotations.as_deref(), Some(&[0, 90][..]));
        let rects = o.rects.unwrap();
        assert_eq!(rects[0].redact_style().unwrap(), RedactStyle::Blur);
        assert_eq!(rects[1].redact_style().unwrap(), RedactStyle::Black);

        let bad = Options::parse(Some(
            r#"{"rects":[{"x":0,"y":0,"w":5,"h":5,"style":"neon"}]}"#,
        ))
        .unwrap();
        assert!(bad.rects.unwrap()[0].redact_style().is_err());
    }
}

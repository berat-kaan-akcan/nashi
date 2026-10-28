//! SRT ↔ VTT and subtitle → plain text, natively.

use std::path::PathBuf;

use super::{Ctx, Input};
use crate::events::{JobError, JobResult};

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: Vec<String>,
}

fn parse_ts(s: &str) -> Option<u64> {
    let s = s.trim().replace(',', ".");
    let (hms, ms) = s.split_once('.').unwrap_or((&s, "0"));
    let parts: Vec<&str> = hms.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (
            h.parse::<u64>().ok()?,
            m.parse::<u64>().ok()?,
            s.parse::<u64>().ok()?,
        ),
        [m, s] => (0, m.parse::<u64>().ok()?, s.parse::<u64>().ok()?),
        _ => return None,
    };
    let mut ms_str: String = ms.chars().take(3).collect();
    while ms_str.len() < 3 {
        ms_str.push('0');
    }
    let ms: u64 = ms_str.parse().ok()?;
    Some(((h * 60 + m) * 60 + sec) * 1000 + ms)
}

fn timing(line: &str) -> Option<(u64, u64)> {
    let (a, b) = line.split_once("-->")?;
    let b = b.split_whitespace().next()?; // drop VTT cue settings
    Some((parse_ts(a)?, parse_ts(b)?))
}

/// Parse SRT or VTT (the format is detected per block).
pub fn parse(text: &str) -> Vec<Cue> {
    let text = text
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut cues = Vec::new();
    for block in text.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(ti) = lines.iter().position(|l| l.contains("-->")) else {
            continue; // header, NOTE, STYLE, REGION
        };
        if lines[..ti]
            .iter()
            .any(|l| l.starts_with("NOTE") || l.starts_with("STYLE"))
        {
            continue;
        }
        let Some((s, e)) = timing(lines[ti]) else {
            continue;
        };
        let body: Vec<String> = lines[ti + 1..].iter().map(|l| l.to_string()).collect();
        cues.push(Cue {
            start_ms: s,
            end_ms: e,
            text: body,
        });
    }
    cues
}

fn fmt_ts(ms: u64, sep: char) -> String {
    format!(
        "{:02}:{:02}:{:02}{sep}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

pub fn to_srt(cues: &[Cue]) -> String {
    let mut o = String::new();
    for (i, c) in cues.iter().enumerate() {
        o.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            fmt_ts(c.start_ms, ','),
            fmt_ts(c.end_ms, ','),
            c.text.join("\n")
        ));
    }
    o
}

pub fn to_vtt(cues: &[Cue]) -> String {
    let mut o = String::from("WEBVTT\n\n");
    for c in cues {
        o.push_str(&format!(
            "{} --> {}\n{}\n\n",
            fmt_ts(c.start_ms, '.'),
            fmt_ts(c.end_ms, '.'),
            c.text.join("\n")
        ));
    }
    o
}

fn strip_markup(s: &str) -> String {
    let mut o = String::new();
    let mut in_tag = false;
    let mut in_brace = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            '{' => in_brace = true,
            '}' if in_brace => in_brace = false,
            c if !in_tag && !in_brace => o.push(c),
            _ => {}
        }
    }
    o.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

pub fn to_txt(cues: &[Cue]) -> String {
    let mut o = String::new();
    for c in cues {
        for l in &c.text {
            let t = strip_markup(l);
            let t = t.trim();
            if !t.is_empty() {
                o.push_str(t);
                o.push('\n');
            }
        }
    }
    o
}

pub fn convert(ctx: &Ctx, input: &Input, target: &str) -> JobResult<PathBuf> {
    ctx.progress(None, "Converting");
    let raw = std::fs::read(&input.path)?;
    let text = String::from_utf8_lossy(&raw);
    let cues = parse(&text);
    if cues.is_empty() {
        return Err(JobError::invalid_input("no subtitle cues found"));
    }
    let out = match target {
        "srt" => to_srt(&cues),
        "vtt" => to_vtt(&cues),
        "txt" => to_txt(&cues),
        _ => {
            return Err(JobError::unsupported(format!(
                "subtitles to .{target} are not supported"
            )))
        }
    };
    let staged = input.stage("", target)?;
    std::fs::write(staged.path(), out)?;
    Ok(staged.commit()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "1\r\n00:00:01,000 --> 00:00:02,500\r\nHello <i>world</i>\r\n\r\n2\r\n00:01:02,003 --> 01:00:00,000\r\nLine one\r\nLine two\r\n";
    const VTT: &str = "WEBVTT\n\nNOTE a comment\n\nintro\n00:01.000 --> 00:02.500 align:start\nHello <i>world</i>\n\n01:02.003 --> 1:00:00.000\nLine one\nLine two\n";

    #[test]
    fn roundtrip() {
        let a = parse(SRT);
        let b = parse(VTT);
        assert_eq!(a, b);
        assert_eq!(a[1].start_ms, 62_003);
        let vtt = to_vtt(&a);
        assert!(vtt.starts_with("WEBVTT\n\n00:00:01.000 --> 00:00:02.500\n"));
        assert_eq!(parse(&vtt), a);
        let srt = to_srt(&b);
        assert!(srt.starts_with("1\n00:00:01,000 --> 00:00:02,500\n"));
        assert_eq!(parse(&srt), a);
        assert_eq!(to_txt(&a), "Hello world\nLine one\nLine two\n");
    }
}

//! Audio, video and GIF via ffmpeg.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::{Ctx, Input, Produced};
use crate::catalog::{category_of, Category};
use crate::deps::require;
use crate::detect::{ffprobe, Probe};
use crate::events::{ErrorCode, JobError, JobResult};
use crate::fsops::{Staged, WorkDir};
use crate::process::{Cmd, FfProgress};

/// ffmpeg muxer for an extension.
pub fn muxer(ext: &str) -> &'static str {
    match ext {
        "mp4" => "mp4",
        "mov" => "mov",
        "mkv" => "matroska",
        "webm" => "webm",
        "avi" => "avi",
        "wmv" | "wma" => "asf",
        "mp3" => "mp3",
        "m4a" => "ipod",
        "wav" => "wav",
        "flac" => "flac",
        "ogg" => "ogg",
        "opus" => "opus",
        "aiff" => "aiff",
        "gif" => "gif",
        _ => "matroska",
    }
}

/// Audio encoder args for an output extension (audio-only or video container).
pub fn audio_codec(ext: &str) -> Vec<&'static str> {
    match ext {
        "mp3" => vec!["-c:a", "libmp3lame", "-q:a", "2"],
        "m4a" => vec!["-c:a", "aac", "-b:a", "192k"],
        "wav" => vec!["-c:a", "pcm_s16le"],
        "flac" => vec!["-c:a", "flac"],
        "ogg" => vec!["-c:a", "libvorbis", "-q:a", "5"],
        "opus" | "webm" => vec!["-c:a", "libopus", "-b:a", "128k"],
        "aiff" => vec!["-c:a", "pcm_s16be"],
        "wma" | "wmv" => vec!["-c:a", "wmav2", "-b:a", "192k"],
        "avi" => vec!["-c:a", "libmp3lame", "-q:a", "4"],
        _ => vec!["-c:a", "aac", "-b:a", "192k"],
    }
}

/// The PCM codec for WAV or AIFF: the chosen depth, else the source's, as sources
/// deeper than 16 bits would otherwise lose detail on every conversion.
fn pcm_codec(ext: &str, p: &Probe, depth: Option<&str>) -> String {
    let endian = if ext == "wav" { "le" } else { "be" };
    let source = p.audio_codec.as_deref().unwrap_or("");
    let digits = |s: &str| {
        s.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .ok()
    };
    match depth {
        Some("float32") => format!("pcm_f32{endian}"),
        Some(bits) => format!("pcm_s{bits}{endian}"),
        None if source.starts_with("pcm_f") => {
            let wide = digits(&source[5..]).is_some_and(|d| d > 32);
            format!("pcm_f{}{endian}", if wide { 64 } else { 32 })
        }
        None => {
            let from_name = source
                .strip_prefix("pcm_s")
                .or_else(|| source.strip_prefix("pcm_u"))
                .and_then(digits);
            let bits = p.audio_bits.filter(|b| *b > 0).or(from_name).unwrap_or(16);
            let width = match bits {
                ..=16 => 16,
                17..=24 => 24,
                _ => 32,
            };
            format!("pcm_s{width}{endian}")
        }
    }
}

/// Encoder args for audio written to `ext`, following the file and the options.
fn audio_for(ext: &str, p: &Probe, o: &crate::options::Options) -> Vec<String> {
    match ext {
        "wav" | "aiff" => vec![
            "-c:a".to_string(),
            pcm_codec(ext, p, o.sample_depth.as_deref()),
        ],
        "flac" => {
            let level = match o.audio_compression.as_deref() {
                Some("fast") => Some("0"),
                Some("balanced") => Some("8"),
                Some("maximum") => Some("12"),
                _ => None,
            };
            let mut a = vec!["-c:a".to_string(), "flac".to_string()];
            if let Some(level) = level {
                a.extend(["-compression_level".to_string(), level.to_string()]);
            }
            a
        }
        _ => audio_at(ext, o.audio_kbps),
    }
}

/// `audio_codec` at a chosen bitrate; lossless and PCM codecs have none to set.
fn audio_at(ext: &str, kbps: Option<u32>) -> Vec<String> {
    let base = audio_codec(ext);
    match kbps {
        Some(kbps) if !matches!(base[1], "pcm_s16le" | "pcm_s16be" | "flac") => {
            vec![
                "-c:a".to_string(),
                base[1].to_string(),
                "-b:a".to_string(),
                format!("{kbps}k"),
            ]
        }
        _ => base.into_iter().map(String::from).collect(),
    }
}

/// Extra encoder args for audio an encoder cannot take as it is. `channels` is
/// `None` when the tool sets its own channel count.
fn audio_fit(ext: &str, channels: Option<u32>, rate: Option<u32>) -> Vec<&'static str> {
    match ext {
        // libopus rejects surround layouts outside Vorbis order, such as 5.1(side).
        "opus" | "webm" => match channels {
            Some(3) => vec!["-ch_layout", "3.0"],
            Some(4) => vec!["-ch_layout", "quad"],
            Some(5) => vec!["-ch_layout", "5.0"],
            Some(6) => vec!["-ch_layout", "5.1"],
            Some(7) => vec!["-ch_layout", "6.1"],
            Some(8) => vec!["-ch_layout", "7.1"],
            _ => vec![],
        },
        // WMA holds at most two channels at up to 48 kHz.
        "wma" | "wmv" => {
            let mut a = vec![];
            if channels.is_some_and(|c| c > 2) {
                a.extend(["-ac", "2"]);
            }
            if rate.is_some_and(|r| r > 48000) {
                a.extend(["-ar", "48000"]);
            }
            a
        }
        _ => vec![],
    }
}

/// The quantizer of encoders without CRF. Size doubles about every 6 CRF steps
/// and halves as the quantizer doubles; CRF 23 lands on the usual 3.
fn fixed_quantizer(crf: u32) -> u32 {
    (3.0 * 2f64.powf((f64::from(crf) - 23.0) / 6.0))
        .round()
        .clamp(1.0, 31.0) as u32
}

/// Put a height limit in front of the even-size filter of `video_codec`. Never enlarges.
fn limit_height(codec: &mut [String], max_height: Option<u32>) {
    if let Some(h) = max_height {
        let i = codec.iter().position(|x| x == "-vf").unwrap();
        codec[i + 1] = format!("scale=-2:'min({h},ih)',{}", codec[i + 1]);
    }
}

/// Video encoder args for a container; `crf` overrides the default quality.
pub fn video_codec(ext: &str, crf: Option<u32>) -> Vec<String> {
    let even = "scale=trunc(iw/2)*2:trunc(ih/2)*2".to_string();
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<String>>();
    let mut v = match ext {
        "webm" => {
            let mut a = s(&[
                "-c:v",
                "libvpx-vp9",
                "-b:v",
                "0",
                "-row-mt",
                "1",
                "-deadline",
                "good",
                "-cpu-used",
                "4",
                "-crf",
            ]);
            a.push(crf.unwrap_or(32).to_string());
            a
        }
        "avi" => {
            let mut a = s(&["-c:v", "mpeg4", "-q:v"]);
            a.push(crf.map_or(3, fixed_quantizer).to_string());
            a.extend(s(&["-vtag", "xvid"]));
            a
        }
        "wmv" => match crf {
            Some(c) => vec![
                "-c:v".to_string(),
                "wmv2".to_string(),
                "-q:v".to_string(),
                fixed_quantizer(c).to_string(),
            ],
            None => s(&["-c:v", "wmv2", "-b:v", "2500k"]),
        },
        _ => {
            let mut a = s(&["-c:v", "libx264", "-preset", "medium", "-crf"]);
            a.push(crf.unwrap_or(23).to_string());
            a
        }
    };
    v.extend(s(&["-pix_fmt", "yuv420p", "-vf"]));
    v.push(even);
    if matches!(ext, "mp4" | "mov") {
        v.extend(s(&["-movflags", "+faststart"]));
    }
    v
}

/// Album art is an attached picture stream, not a tag: map it from `input` where
/// the container stores JPEG/PNG art, otherwise drop video with `-vn`.
fn cover_args(p: &Probe, ext: &str, strip: bool, input: u8) -> Vec<String> {
    match p
        .art_stream
        .filter(|_| !strip && matches!(ext, "mp3" | "m4a" | "flac"))
    {
        Some(art) => vec![
            "-map".into(),
            format!("{input}:{art}"),
            "-c:v".into(),
            "copy".into(),
            "-disposition:v:0".into(),
            "attached_pic".into(),
        ],
        None => vec!["-vn".into()],
    }
}

/// Base ffmpeg command with progress on stdout.
fn ffmpeg(ctx: &Ctx) -> JobResult<Cmd> {
    Ok(Cmd::new(require(ctx.deps, "ffmpeg")?).args([
        "-hide_banner",
        "-nostdin",
        "-y",
        "-loglevel",
        "error",
        "-progress",
        "pipe:1",
        "-nostats",
    ]))
}

/// Run an ffmpeg command, reporting progress against `duration`.
fn run(ctx: &Ctx, cmd: Cmd, duration: Option<f64>, msg: &str) -> JobResult<()> {
    let mut prog = FfProgress::new(duration);
    ctx.progress(if duration.is_some() { Some(0.0) } else { None }, msg);
    cmd.run_ok_lines(&mut |line| {
        if let Some(f) = prog.feed(line) {
            ctx.progress(Some(f), msg);
        }
    })?;
    Ok(())
}

fn probe(ctx: &Ctx, input: &Input) -> Probe {
    ffprobe(ctx.deps, &input.path).unwrap_or(Probe {
        has_audio: true,
        has_video: matches!(input.category(), Category::Video | Category::Gif),
        ..Default::default()
    })
}

fn out(path: &Path) -> OsString {
    let mut s = OsString::from("file:");
    s.push(path.as_os_str());
    s
}

/// Convert between audio/video/gif formats.
pub fn convert(ctx: &Ctx, input: &Input, target: &str) -> JobResult<PathBuf> {
    let o = ctx.opts;
    check_encoding(o)?;
    if o.sample_depth
        .as_deref()
        .is_some_and(|d| !["16", "24", "32", "float32"].contains(&d))
    {
        return Err(JobError::invalid_options(
            "sample_depth must be 16, 24, 32 or float32",
        ));
    }
    if o.audio_compression
        .as_deref()
        .is_some_and(|c| !["fast", "balanced", "maximum"].contains(&c))
    {
        return Err(JobError::invalid_options(
            "audio_compression must be fast, balanced or maximum",
        ));
    }
    if o.gif_fps.is_some_and(|f| !(1..=60).contains(&f)) {
        return Err(JobError::invalid_options(
            "gif_fps must be between 1 and 60",
        ));
    }
    if o.gif_max_width.is_some_and(|w| !(2..=16384).contains(&w)) {
        return Err(JobError::invalid_options(
            "gif_max_width must be between 2 and 16384",
        ));
    }
    let gif_palette = match o.gif_compression.as_deref() {
        None | Some("balanced") => ("", ""),
        // Ordered dithering skips error diffusion's pass over every pixel.
        Some("fast") => ("", "=dither=bayer:bayer_scale=5"),
        // Fewer colours, no dither noise, and only the part of a frame that changed.
        Some("maximum") => (
            "=max_colors=128:stats_mode=diff",
            "=dither=none:diff_mode=rectangle",
        ),
        Some(_) => {
            return Err(JobError::invalid_options(
                "gif_compression must be fast, balanced or maximum",
            ))
        }
    };
    let p = probe(ctx, input);
    let staged = input.stage("", target)?;
    let mut cmd = ffmpeg(ctx)?.arg("-i").arg_cat("file:", &input.path);
    let tcat = category_of(target);
    let strip = o.strip_metadata == Some(true);
    if strip {
        cmd = cmd.args(["-map_metadata", "-1", "-map_chapters", "-1"]);
    }
    match tcat {
        Category::Audio => {
            if !p.has_audio {
                return Err(JobError::invalid_input("the file has no audio stream"));
            }
            cmd = cmd.args(["-sn", "-dn", "-map", "0:a:0"]);
            if !strip {
                cmd = cmd.args(["-map_metadata", "0"]);
            }
            cmd = cmd.args(cover_args(&p, target, strip, 0));
            // Other containers write file-level tags, so copy Ogg's stream tags there.
            if p.ogg_stream_tags && !strip {
                cmd = cmd.args(["-map_metadata", "0:s:a:0"]);
            }
            cmd = cmd.args(audio_for(target, &p, o)).args(audio_fit(
                target,
                p.channels,
                p.sample_rate,
            ));
        }
        Category::Gif => {
            let fps = o.gif_fps.unwrap_or(12);
            let width = o.gif_max_width.unwrap_or(640);
            cmd = cmd
                .args(["-an", "-filter_complex"])
                // GIF viewers ignore a pixel aspect, so size from the displayed shape.
                .arg(format!(
                    "[0:v:0]fps={fps},scale='min({width},iw*sar)':'ow/dar':flags=lanczos,setsar=1,split[a][b];[a]palettegen{}[p];[b][p]paletteuse{}",
                    gif_palette.0, gif_palette.1
                ))
                .args(["-loop", "0"]);
        }
        Category::Video => {
            if !p.has_video {
                return Err(JobError::invalid_input("the file has no video stream"));
            }
            cmd = cmd.args(["-map", "0:v:0"]);
            if p.has_audio && input.category() != Category::Gif {
                cmd = cmd.args(["-map", "0:a:0?"]);
                cmd = cmd.args(audio_at(target, o.audio_kbps)).args(audio_fit(
                    target,
                    p.channels,
                    p.sample_rate,
                ));
            } else {
                cmd = cmd.arg("-an");
            }
            let mut codec = video_codec(target, o.crf);
            limit_height(&mut codec, o.max_height);
            cmd = cmd.args(codec);
            if !strip {
                cmd = cmd.args(["-map_metadata", "0"]);
            }
            // MP4 and MOV write only a fixed tag list unless asked for the source's own keys.
            if !strip && p.quicktime_keys && matches!(target, "mp4" | "mov") {
                cmd = cmd.args(["-movflags", "+use_metadata_tags"]);
            }
        }
        _ => {
            return Err(JobError::unsupported(format!(
                "cannot convert media to .{target}"
            )))
        }
    }
    cmd = cmd.args(["-f", muxer(target)]).arg(out(staged.path()));
    run(ctx, cmd, p.duration, "Converting")?;
    Ok(staged.commit()?)
}

fn trim_args(ctx: &Ctx, p: &Probe) -> JobResult<(Vec<String>, Option<f64>)> {
    let (s, e) = (ctx.opts.start, ctx.opts.end);
    if s.unwrap_or(0.0) == 0.0 && e.is_none() {
        return Err(JobError::invalid_options("trim needs start and/or end"));
    }
    let start = s.unwrap_or(0.0);
    let mut a = vec!["-ss".to_string(), format!("{start:.3}")];
    if let Some(e) = e {
        a.push("-to".into());
        a.push(format!("{e:.3}"));
    }
    let dur = match (e, p.duration) {
        (Some(e), _) => Some(e - start),
        (None, Some(d)) => Some((d - start).max(0.0)),
        _ => None,
    };
    if let (Some(d), true) = (p.duration, start > 0.0) {
        if start >= d {
            return Err(JobError::invalid_options(
                "start is beyond the end of the file",
            ));
        }
    }
    Ok((a, dur))
}

fn atempo_chain(speed: f64) -> String {
    let mut parts = Vec::new();
    let mut s = speed;
    while s > 2.0 {
        parts.push("atempo=2.0".to_string());
        s /= 2.0;
    }
    while s < 0.5 {
        parts.push("atempo=0.5".to_string());
        s /= 0.5;
    }
    parts.push(format!("atempo={s:.6}"));
    parts.join(",")
}

/// The options that give `loudnorm` its first pass, from the JSON it printed.
/// `None` when a value is not finite, as for silence: one pass handles that.
fn measured_loudness(log: &str) -> Option<String> {
    let start = log.rfind('{')?;
    let end = log.rfind('}')?;
    let v: serde_json::Value = serde_json::from_str(log.get(start..=end)?).ok()?;
    let value = |key: &str| {
        let text = v[key].as_str()?;
        text.parse::<f64>().ok().filter(|n| n.is_finite())?;
        Some(text.to_string())
    };
    Some(format!(
        ":measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true",
        value("input_i")?,
        value("input_tp")?,
        value("input_lra")?,
        value("input_thresh")?,
        value("target_offset")?
    ))
}

/// Options shared by `compress` and conversions.
fn check_encoding(o: &crate::options::Options) -> JobResult<()> {
    if o.audio_kbps.is_some_and(|k| !(8..=512).contains(&k)) {
        return Err(JobError::invalid_options(
            "audio_kbps must be between 8 and 512",
        ));
    }
    if o.max_height.is_some_and(|h| !(2..=16384).contains(&h)) {
        return Err(JobError::invalid_options(
            "max_height must be between 2 and 16384",
        ));
    }
    Ok(())
}

fn compress(ctx: &Ctx, input: &Input, p: &Probe) -> JobResult<Produced> {
    let target = super::compress_target(ctx.opts, input)?;
    check_encoding(ctx.opts)?;
    if input.category() == Category::Video {
        compress_video(ctx, input, p, target)
    } else {
        compress_audio(ctx, input, p, target)
    }
}

/// Attempts at a size target; each one aims lower, since encoders overshoot.
const TARGET_ATTEMPTS: [f64; 3] = [0.95, 0.8, 0.65];

fn known_duration(p: &Probe) -> JobResult<f64> {
    p.duration
        .filter(|d| *d > 0.0)
        .ok_or_else(|| JobError::invalid_input("A size target needs media with a known duration"))
}

fn compress_video(ctx: &Ctx, input: &Input, p: &Probe, target: Option<u64>) -> JobResult<Produced> {
    let o = ctx.opts;
    let ext = input.ext().to_string();
    let source = || ffmpeg(ctx).map(|c| c.arg("-i").arg_cat("file:", &input.path));
    let fitted = |crf: Option<u32>| -> Vec<String> {
        let mut v = video_codec(&ext, crf);
        limit_height(&mut v, o.max_height);
        v
    };
    let audio_args = |kbps: u32| -> Vec<String> {
        let codec = if ext == "webm" { "libopus" } else { "aac" };
        let mut a: Vec<String> = ["-c:a", codec, "-b:a", &format!("{kbps}k")]
            .iter()
            .map(|x| x.to_string())
            .collect();
        a.extend(
            audio_fit(&ext, p.channels, p.sample_rate)
                .into_iter()
                .map(String::from),
        );
        a
    };
    let Some(target) = target else {
        let staged = input.stage("-compressed", &ext)?;
        let cmd = source()?
            .args(fitted(Some(o.crf.unwrap_or(26))))
            .args(["-map", "0:v:0", "-map", "0:a?"])
            .args(audio_args(if ext == "webm" { 96 } else { 128 }))
            .args(["-f", muxer(&ext)])
            .arg(out(staged.path()));
        run(ctx, cmd, p.duration, "Compressing")?;
        return Ok(staged.commit()?.into());
    };
    if o.max_height.is_none() && std::fs::metadata(&input.path)?.len() <= target {
        return super::keep_original(input, &ext);
    }
    let seconds = known_duration(p)?;
    let total_kbps = target as f64 * 8.0 / seconds / 1000.0;
    let audio_kbps = if p.has_audio {
        (total_kbps / 8.0).clamp(32.0, 128.0) as u32
    } else {
        0
    };
    let staged = input.stage("-compressed", &ext)?;
    let work = WorkDir::new_in(&input.out_dir)?;
    let log = work.join("pass");
    for share in TARGET_ATTEMPTS {
        let video_kbps = total_kbps * share - f64::from(audio_kbps);
        if video_kbps < 20.0 {
            break;
        }
        // Two passes keep the size close to the request without wasting bits.
        let mut video = fitted(None);
        for flag in ["-crf", "-b:v"] {
            if let Some(i) = video.iter().position(|x| x == flag) {
                video.drain(i..i + 2);
            }
        }
        video.extend(["-b:v".to_string(), format!("{}k", video_kbps as u32)]);
        let first = source()?
            .args(["-map", "0:v:0", "-an"])
            .args(video.iter())
            .args(["-pass", "1", "-passlogfile"])
            .arg(&log)
            .args(["-f", "null", "/dev/null"]);
        run(ctx, first, p.duration, "Compressing (first pass)")?;
        let mut second = source()?
            .args(["-map", "0:v:0", "-map", "0:a:0?"])
            .args(video.iter())
            .args(["-pass", "2", "-passlogfile"])
            .arg(&log);
        if p.has_audio {
            second = second.args(audio_args(audio_kbps));
        }
        let second = second.args(["-f", muxer(&ext)]).arg(out(staged.path()));
        run(ctx, second, p.duration, "Compressing (second pass)")?;
        if std::fs::metadata(staged.path())?.len() <= target {
            drop(work);
            return Ok(staged.commit()?.into());
        }
    }
    Err(super::target_unreachable())
}

fn compress_audio(ctx: &Ctx, input: &Input, p: &Probe, target: Option<u64>) -> JobResult<Produced> {
    let o = ctx.opts;
    let ext = input.ext();
    let source = || {
        ffmpeg(ctx).map(|c| {
            c.arg("-i")
                .arg_cat("file:", &input.path)
                .args(["-vn", "-map", "0:a:0"])
        })
    };
    let at_bitrate = |ext: &str, kbps: u32| -> Vec<String> {
        let codec = match ext {
            "mp3" => "libmp3lame",
            "ogg" => "libvorbis",
            "opus" => "libopus",
            "wma" => "wmav2",
            _ => "aac",
        };
        let mut a: Vec<String> = ["-c:a", codec, "-b:a", &format!("{kbps}k")]
            .iter()
            .map(|x| x.to_string())
            .collect();
        a.extend(
            audio_fit(ext, p.channels, p.sample_rate)
                .into_iter()
                .map(String::from),
        );
        a
    };
    let has_bitrate = matches!(ext, "mp3" | "m4a" | "ogg" | "opus" | "wma");
    let Some(target) = target else {
        let staged = input.stage("-compressed", ext)?;
        let a: Vec<String> = match (ext, o.audio_kbps) {
            (_, Some(kbps)) if has_bitrate => at_bitrate(ext, kbps),
            ("mp3" | "m4a", None) => at_bitrate(ext, 128),
            ("opus", None) => at_bitrate(ext, 64),
            ("ogg", None) => ["-c:a", "libvorbis", "-q:a", "3"]
                .map(String::from)
                .to_vec(),
            ("flac", _) => ["-c:a", "flac", "-compression_level", "12"]
                .map(String::from)
                .to_vec(),
            ("wav", _) => ["-c:a", "pcm_s16le", "-ar", "22050"]
                .map(String::from)
                .to_vec(),
            _ => audio_codec(ext)
                .into_iter()
                .chain(audio_fit(ext, p.channels, p.sample_rate))
                .map(String::from)
                .collect(),
        };
        let cmd = source()?
            .args(a)
            .args(["-f", muxer(ext)])
            .arg(out(staged.path()));
        run(ctx, cmd, p.duration, "Compressing")?;
        return Ok(staged.commit()?.into());
    };
    if std::fs::metadata(&input.path)?.len() <= target {
        return super::keep_original(input, ext);
    }
    // Lossless containers have no bitrate to lower.
    let out_ext = if has_bitrate { ext } else { "m4a" };
    let seconds = known_duration(p)?;
    let total_kbps = target as f64 * 8.0 / seconds / 1000.0;
    let staged = input.stage("-compressed", out_ext)?;
    for share in TARGET_ATTEMPTS {
        let kbps = (total_kbps * share).min(320.0) as u32;
        if kbps < 8 {
            break;
        }
        let cmd = source()?
            .args(at_bitrate(out_ext, kbps))
            .args(["-f", muxer(out_ext)])
            .arg(out(staged.path()));
        run(ctx, cmd, p.duration, "Compressing")?;
        if std::fs::metadata(staged.path())?.len() <= target {
            return Ok(staged.commit()?.into());
        }
    }
    Err(super::target_unreachable())
}

/// Single-input audio/video tools.
pub fn tool(ctx: &Ctx, tool: &str, input: &Input) -> JobResult<Produced> {
    let o = ctx.opts;
    let ext = input.ext().to_string();
    let p = probe(ctx, input);
    let is_video = input.category() == Category::Video;
    let cover = |input: u8| cover_args(&p, &ext, o.strip_metadata == Some(true), input);
    let ffin = |c: Cmd| c.arg("-i").arg_cat("file:", &input.path);
    // Encoder args to re-encode into the same container.
    let reencode = |vf: Option<String>, af: Option<String>, cover_input: u8| -> Vec<String> {
        let mut a: Vec<String> = Vec::new();
        if is_video {
            let mut v = video_codec(&ext, o.crf);
            if let Some(f) = vf {
                // merge with the even-size scale filter
                let idx = v.iter().position(|x| x == "-vf").unwrap();
                v[idx + 1] = format!("{f},{}", v[idx + 1]);
            }
            a.extend(v);
            a.extend(["-map".into(), "0:v:0".into()]);
            if p.has_audio {
                a.extend(["-map".into(), "0:a:0".into()]);
            }
        } else {
            a.extend(["-map".into(), "0:a:0".into()]);
            a.extend(cover(cover_input));
        }
        if p.has_audio {
            a.extend(audio_for(&ext, &p, o));
            a.extend(
                audio_fit(&ext, p.channels, p.sample_rate)
                    .into_iter()
                    .map(String::from),
            );
            if let Some(f) = af {
                a.extend(["-af".into(), f]);
            }
        }
        a
    };
    let finish = |cmd: Cmd, staged: Staged, dur: Option<f64>, msg: &str| -> JobResult<Produced> {
        let cmd = cmd.args(["-f", muxer(&ext)]).arg(out(staged.path()));
        run(ctx, cmd, dur, msg)?;
        Ok(staged.commit()?.into())
    };
    match tool {
        "compress" => compress(ctx, input, &p),
        "removeMetadata" => {
            let staged = input.stage("-clean", &ext)?;
            let maps: &[&str] = if is_video {
                &["-map", "0:v?", "-map", "0:a?", "-map", "0:s?"]
            } else {
                &["-map", "0:a"]
            };
            let cmd = ffin(ffmpeg(ctx)?)
                .args(maps)
                .args([
                    "-map_metadata",
                    "-1",
                    "-map_chapters",
                    "-1",
                    "-c",
                    "copy",
                    "-fflags",
                    "+bitexact",
                ])
                .args(["-flags:v", "+bitexact", "-flags:a", "+bitexact"]);
            finish(cmd, staged, p.duration, "Removing metadata")
        }
        "muteVideo" => {
            let staged = input.stage("-muted", &ext)?;
            let cmd = ffin(ffmpeg(ctx)?).args(["-map", "0", "-map", "-0:a", "-c", "copy"]);
            finish(cmd, staged, p.duration, "Removing audio")
        }
        "normalizeAudio" => {
            let range = |value: Option<f64>, default: f64, low: f64, high: f64, what: &str| {
                let v = value.unwrap_or(default);
                if (low..=high).contains(&v) {
                    Ok(v)
                } else {
                    Err(JobError::invalid_options(format!(
                        "{what} must be between {low} and {high}"
                    )))
                }
            };
            let lufs = range(o.target_lufs, -16.0, -70.0, -5.0, "target_lufs")?;
            let lra = range(o.loudness_range, 11.0, 1.0, 50.0, "loudness_range")?;
            let peak = range(o.true_peak, -1.5, -9.0, 0.0, "true_peak")?;
            let staged = input.stage("-normalized", &ext)?;
            let mut filter = format!("loudnorm=I={lufs}:TP={peak}:LRA={lra}");
            // With the whole file measured first, the level can be changed evenly, so
            // quiet and loud parts stay as far apart as they were.
            ctx.progress(None, "Measuring loudness");
            let measured = ffmpeg(ctx)?
                .args(["-loglevel", "info"])
                .arg("-i")
                .arg_cat("file:", &input.path)
                .args(["-vn", "-map", "0:a:0", "-af"])
                .arg(format!("{filter}:print_format=json"))
                .args(["-f", "null", "-"])
                .run_ok()?;
            if let Some(second_pass) = measured_loudness(&measured.stderr) {
                filter.push_str(&second_pass);
            }
            let rate = p.sample_rate.unwrap_or(48000).to_string();
            let cmd = ffin(ffmpeg(ctx)?)
                .args(["-map", "0:a:0"])
                .args(cover(0))
                .args(["-af", &filter, "-ar", &rate])
                .args(audio_for(&ext, &p, o))
                .args(audio_fit(&ext, p.channels, p.sample_rate));
            finish(cmd, staged, p.duration, "Normalizing")
        }
        "audioChannels" => {
            let ch = o.channels.as_deref().unwrap_or("mono");
            let staged = input.stage(&format!("-{ch}"), &ext)?;
            let n = if ch == "mono" { "1" } else { "2" };
            let cmd = ffin(ffmpeg(ctx)?)
                .args(["-map", "0:a:0"])
                .args(cover(0))
                .args(["-ac", n])
                .args(audio_for(&ext, &p, o))
                .args(audio_fit(&ext, None, p.sample_rate));
            finish(cmd, staged, p.duration, "Converting channels")
        }
        "trimVideo" | "trimAudio" => {
            let (targs, dur) = trim_args(ctx, &p)?;
            let staged = input.stage("-trimmed", &ext)?;
            let silence = if tool == "trimAudio" && o.trim_silence == Some(true) {
                let db = o.silence_threshold_db.unwrap_or(-50.0);
                if !db.is_finite() || !(-120.0..=0.0).contains(&db) {
                    return Err(JobError::invalid_options(
                        "silence_threshold_db must be between -120 and 0",
                    ));
                }
                let edge = format!(
                    "silenceremove=start_periods=1:start_duration=0.02:start_threshold={db}dB"
                );
                Some(format!("{edge},areverse,{edge},areverse"))
            } else {
                None
            };
            // The result's length is unknown until the silence has been measured.
            let dur = if silence.is_some() { None } else { dur };
            // -ss and -to sit before -i, so the filter only ever sees the selection.
            let cmd = ffmpeg(ctx)?.args(targs);
            let cmd = ffin(cmd).args(reencode(None, silence, 0));
            finish(cmd, staged, dur, "Trimming")
        }
        "changeVideoSpeed" => {
            let s = o.speed.filter(|s| (*s - 1.0).abs() > 1e-6).ok_or_else(|| {
                JobError::invalid_options("changeVideoSpeed needs a speed other than 1")
            })?;
            let staged = input.stage(&format!("-{s}x"), &ext)?;
            let cmd = ffin(ffmpeg(ctx)?).args(reencode(
                Some(format!("setpts=PTS/{s}")),
                Some(atempo_chain(s)),
                0,
            ));
            finish(cmd, staged, p.duration.map(|d| d / s), "Changing speed")
        }
        "cropVideo" => {
            let r = o
                .rect
                .as_ref()
                .ok_or_else(|| JobError::invalid_options("cropVideo needs a rect option"))?;
            let staged = input.stage("-cropped", &ext)?;
            let f = format!(
                "crop={}:{}:{}:{}",
                r.w.round(),
                r.h.round(),
                r.x.round(),
                r.y.round()
            );
            let cmd = ffin(ffmpeg(ctx)?).args(reencode(Some(f), None, 0));
            finish(cmd, staged, p.duration, "Cropping")
        }
        "redactVideo" => {
            use crate::options::RedactStyle;

            let rects = o
                .rects
                .as_ref()
                .filter(|r| !r.is_empty())
                .ok_or_else(|| JobError::invalid_options("redactVideo needs rects"))?;
            let styles: Vec<RedactStyle> = rects
                .iter()
                .map(|r| r.redact_style())
                .collect::<JobResult<_>>()?;
            let mut steps = Vec::new();
            for (i, (r, style)) in rects.iter().zip(styles).enumerate() {
                let en = match (r.t0, r.t1) {
                    (None, None) => String::new(),
                    (a, b) => format!(
                        ":enable='between(t,{},{})'",
                        a.unwrap_or(0.0),
                        b.unwrap_or(1e9)
                    ),
                };
                match style {
                    RedactStyle::Black => {
                        steps.push(format!(
                            "drawbox=x={}:y={}:w={}:h={}:color=black:t=fill{en}",
                            r.x.round(),
                            r.y.round(),
                            r.w.round().max(1.0),
                            r.h.round().max(1.0)
                        ));
                    }
                    RedactStyle::Blur | RedactStyle::Pixelate => {
                        // overlay moves an odd x or y down to even in 4:2:0 video, which
                        // would leave the far edge uncovered: grow the box to even edges.
                        let left = r.x.round().max(0.0) as u32;
                        let top = r.y.round().max(0.0) as u32;
                        let mut x = left & !1;
                        let mut y = top & !1;
                        let mut cw = ((left + (r.w.round() as u32).max(2) - x + 1) & !1).max(2);
                        let mut ch = ((top + (r.h.round() as u32).max(2) - y + 1) & !1).max(2);
                        if let (Some(vw), Some(vh)) = (p.width, p.height) {
                            if vw < 2 || vh < 2 || x >= vw || y >= vh {
                                continue;
                            }
                            // Only an odd frame size can leave a single column or row.
                            if vw - x < 2 {
                                x -= 2;
                            }
                            if vh - y < 2 {
                                y -= 2;
                            }
                            cw = cw.min(vw - x);
                            ch = ch.min(vh - y);
                        }
                        // format=yuv420p makes the patch opaque: overlay would otherwise let
                        // the original show through wherever the video is transparent.
                        if style == RedactStyle::Blur {
                            steps.push(format!(
                                "split[base{i}][tmp{i}];[tmp{i}]crop={cw}:{ch}:{x}:{y}:exact=1,boxblur=luma_radius='min(16,max(0,min(w,h)/2-1))':luma_power=2:chroma_radius='min(16,max(0,min(cw,ch)/2-1))':chroma_power=2,format=yuv420p[fg{i}];[base{i}][fg{i}]overlay=x={x}:y={y}{en}"
                            ));
                        } else {
                            steps.push(format!(
                                "split[base{i}][tmp{i}];[tmp{i}]crop={cw}:{ch}:{x}:{y}:exact=1,scale='max(1,iw/12)':'max(1,ih/12)':flags=neighbor,scale={cw}:{ch}:flags=neighbor,format=yuv420p[fg{i}];[base{i}][fg{i}]overlay=x={x}:y={y}{en}"
                            ));
                        }
                    }
                }
            }
            let f = if steps.is_empty() {
                "null".to_string()
            } else {
                steps.join(",")
            };
            let staged = input.stage("-redacted", &ext)?;
            let cmd = ffin(ffmpeg(ctx)?).args(reencode(Some(f), None, 0));
            finish(cmd, staged, p.duration, "Redacting")
        }
        "redactAudio" => {
            let ranges = o
                .ranges
                .as_ref()
                .filter(|r| !r.is_empty())
                .ok_or_else(|| JobError::invalid_options("redactAudio needs ranges"))?;
            let f = match o.audio_mode.as_deref().unwrap_or("mute") {
                "mute" => ranges
                    .iter()
                    .map(|[a, b]| format!("volume=0:enable='between(t,{a},{b})'"))
                    .collect::<Vec<_>>()
                    .join(","),
                "bleep" => {
                    let inside = ranges
                        .iter()
                        .map(|[a, b]| format!("between(t,{a},{b})"))
                        .collect::<Vec<_>>()
                        .join("+");
                    let exprs = (0..p.channels.unwrap_or(2))
                        .map(|c| format!("if({inside},0.18*sin(2*PI*1000*t),val({c}))"))
                        .collect::<Vec<_>>()
                        .join("|");
                    format!("aeval=exprs='{exprs}':c=same")
                }
                other => {
                    return Err(JobError::invalid_options(format!(
                        "unknown audio_mode '{other}'"
                    )))
                }
            };
            let staged = input.stage("-bleeped", &ext)?;
            let cmd = ffin(ffmpeg(ctx)?)
                .args(["-map", "0:a:0"])
                .args(cover(0))
                .args(["-af", &f])
                .args(audio_for(&ext, &p, o))
                .args(audio_fit(&ext, p.channels, p.sample_rate));
            finish(cmd, staged, p.duration, "Bleeping")
        }
        "audioToVideo" => {
            let staged = input.stage("-visualizer", "mp4")?;
            let cmd = ffin(ffmpeg(ctx)?)
                .args([
                    "-filter_complex",
                    "[0:a:0]showwaves=s=1280x720:mode=cline:colors=0x5ec8ff,format=yuv420p[v]",
                    "-map",
                    "[v]",
                    "-map",
                    "0:a:0",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "veryfast",
                    "-crf",
                    "23",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "192k",
                    "-shortest",
                    "-movflags",
                    "+faststart",
                    "-f",
                    "mp4",
                ])
                .arg(out(staged.path()));
            run(ctx, cmd, p.duration, "Rendering")?;
            Ok(staged.commit()?.into())
        }
        "videoSnapshots" if o.snapshot_times.is_some() => {
            let times = o.snapshot_times.as_deref().unwrap_or_default();
            if times.is_empty() {
                return Err(JobError::invalid_options("choose at least one frame"));
            }
            let inside =
                |t: &f64| t.is_finite() && *t >= 0.0 && p.duration.is_none_or(|d| *t <= d + 0.001);
            if !times.iter().all(inside) {
                return Err(JobError::invalid_options(
                    "a snapshot time is outside the video",
                ));
            }
            let mut staged = Vec::with_capacity(times.len());
            for (n, time) in times.iter().enumerate() {
                let frame = input.stage(&format!("-frame-{}", n + 1), "png")?;
                let still = |seek: Vec<String>, frames: &[&str]| -> JobResult<()> {
                    // -update writes to this exact name instead of reading it as a pattern.
                    let cmd = ffin(ffmpeg(ctx)?.args(seek))
                        .args(["-map", "0:v:0"])
                        .args(frames)
                        .args(["-vf", "scale='iw*sar':ih,setsar=1"])
                        .args(["-c:v", "png", "-update", "1", "-f", "image2"])
                        .arg(out(frame.path()));
                    let message = format!("Saving frame {} of {}", n + 1, times.len());
                    run(ctx, cmd, None, &message)
                };
                let seek = vec!["-ss".to_string(), format!("{time:.3}")];
                let first_try = still(seek, &["-frames:v", "1"]);
                if matches!(
                    first_try,
                    Err(JobError {
                        code: ErrorCode::Cancelled,
                        ..
                    })
                ) {
                    return first_try.map(|()| unreachable!());
                }
                // Past the start of the last frame there is none to find at or after the
                // time, yet a player shows the last frame there. Decode the end and keep it.
                if first_try.is_err() || std::fs::metadata(frame.path())?.len() == 0 {
                    still(vec!["-sseof".to_string(), "-1".to_string()], &[])?;
                }
                if std::fs::metadata(frame.path())?.len() == 0 {
                    return Err(JobError::tool_failed(
                        "no frame could be saved at that time",
                    ));
                }
                staged.push(frame);
            }
            let mut paths = Vec::with_capacity(staged.len());
            for frame in staged {
                paths.push(frame.commit()?);
            }
            Ok(Produced {
                paths,
                message: None,
            })
        }
        "videoSnapshots" => {
            let fps = o.fps.unwrap_or(1.0);
            let staged = input.stage_dir("-snapshots")?;
            let pattern = pattern_in(
                staged.path(),
                &format!("{}-%04d.jpg", input.stem().replace('%', "%%")),
            );
            let cmd = ffin(ffmpeg(ctx)?)
                .args([
                    "-map",
                    "0:v:0",
                    "-vf",
                    // Image viewers ignore a pixel aspect, so save the displayed shape.
                    &format!("scale='iw*sar':ih,setsar=1,fps={fps}"),
                    "-q:v",
                    "2",
                    "-f",
                    "image2",
                ])
                .arg(&pattern);
            run(ctx, cmd, p.duration, "Extracting frames")?;
            non_empty_dir(staged.path())?;
            Ok(staged.commit()?.into())
        }
        "splitVideo"
            if o.split_points.is_some()
                || o.equal_clips.is_some()
                || o.discard_clips.is_some()
                || o.join_clips == Some(true) =>
        {
            let bounds = split_bounds(o, p.duration)?;
            let clips = bounds.len() - 1;
            let discarded = o.discard_clips.clone().unwrap_or_default();
            if discarded.iter().any(|c| *c >= clips) {
                return Err(JobError::invalid_options(
                    "a removed clip is outside the split",
                ));
            }
            let kept: Vec<usize> = (0..clips).filter(|c| !discarded.contains(c)).collect();
            if kept.is_empty() {
                return Err(JobError::invalid_options("keep at least one clip"));
            }
            let stem = input.stem().to_string();
            let join = o.join_clips == Some(true);
            let render = |clip: usize, position: usize, target: &Path| -> JobResult<()> {
                let (a, b) = (bounds[clip], bounds[clip + 1]);
                // Cuts are re-encoded so they land on the requested time, not a keyframe.
                let cmd =
                    ffmpeg(ctx)?.args(["-ss", &format!("{a:.3}"), "-t", &format!("{:.3}", b - a)]);
                let cmd = ffin(cmd)
                    .args(reencode(None, None, 0))
                    .args(["-f", muxer(&ext)])
                    .arg(out(target));
                let message = format!("Saving clip {} of {}", position + 1, kept.len());
                run(ctx, cmd, Some(b - a), &message)
            };
            if !join {
                let staged = input.stage_dir("-parts")?;
                for (position, clip) in kept.iter().enumerate() {
                    let name = format!("{stem}-{:03}.{ext}", clip + 1);
                    render(*clip, position, &staged.path().join(name))?;
                }
                return Ok(staged.commit()?.into());
            }
            let staged = input.stage("-split", &ext)?;
            let work = WorkDir::new_in(&input.out_dir)?;
            let mut list = String::new();
            for (position, clip) in kept.iter().enumerate() {
                let name = format!("segment-{clip}.{ext}");
                render(*clip, position, &work.join(&name))?;
                // Explicit lengths keep encoder padding from adding up at each cut.
                let length = bounds[clip + 1] - bounds[*clip];
                list.push_str(&format!("file '{name}'\nduration {length:.3}\n"));
            }
            let list_path = work.join("clips.txt");
            std::fs::write(&list_path, list)?;
            let total: f64 = kept.iter().map(|c| bounds[c + 1] - bounds[*c]).sum();
            let cmd = ffmpeg(ctx)?
                .args(["-f", "concat", "-safe", "1", "-i"])
                .arg(&list_path)
                .args(["-map", "0", "-c", "copy", "-t", &format!("{total:.3}")])
                .args(["-f", muxer(&ext)])
                .arg(out(staged.path()));
            run(ctx, cmd, Some(total), "Joining clips")?;
            drop(work);
            Ok(staged.commit()?.into())
        }
        "splitVideo" => {
            let seg = o.segment_seconds.unwrap_or(60.0);
            let staged = input.stage_dir("-parts")?;
            let pattern = pattern_in(
                staged.path(),
                &format!("{}-%03d.{ext}", input.stem().replace('%', "%%")),
            );
            let cmd = ffin(ffmpeg(ctx)?)
                .args([
                    "-map",
                    "0",
                    "-c",
                    "copy",
                    "-f",
                    "segment",
                    "-segment_time",
                    &seg.to_string(),
                ])
                .args(["-reset_timestamps", "1", "-segment_format", muxer(&ext)])
                .arg(&pattern);
            run(ctx, cmd, p.duration, "Splitting")?;
            non_empty_dir(staged.path())?;
            Ok(staged.commit()?.into())
        }
        _ => Err(JobError::unsupported(format!("{tool} is not a media tool"))),
    }
}

/// Copy the streams to a new file with these tags changed, without re-encoding.
pub fn edit_tags(
    ctx: &Ctx,
    input: &Input,
    set: &std::collections::BTreeMap<String, String>,
    remove: &[String],
) -> JobResult<Produced> {
    let ext = input.ext();
    let p = probe(ctx, input);
    let staged = input.stage("-metadata", ext)?;
    let mut cmd = ffmpeg(ctx)?.arg("-i").arg_cat("file:", &input.path).args([
        "-map",
        "0",
        "-c",
        "copy",
        "-map_metadata",
        "0",
    ]);
    for (key, value) in set {
        cmd = cmd.arg("-metadata").arg(format!("{key}={value}"));
    }
    for key in remove {
        cmd = cmd.arg("-metadata").arg(format!("{key}="));
    }
    // MP4 and MOV write only a fixed tag list unless asked for other keys.
    if matches!(ext, "mp4" | "mov") {
        cmd = cmd.args(["-movflags", "+use_metadata_tags"]);
    }
    let cmd = cmd.args(["-f", muxer(ext)]).arg(out(staged.path()));
    run(ctx, cmd, p.duration, "Saving metadata")?;
    Ok(staged.commit()?.into())
}

/// Clip boundaries, from 0 to the end, for a split at chosen times or into equal parts.
fn split_bounds(o: &crate::options::Options, duration: Option<f64>) -> JobResult<Vec<f64>> {
    let total = duration
        .filter(|d| *d > 0.0)
        .ok_or_else(|| JobError::invalid_input("cannot split a video with an unknown duration"))?;
    let mut points: Vec<f64> = if let Some(points) = &o.split_points {
        if points.is_empty() {
            return Err(JobError::invalid_options("add at least one split point"));
        }
        // Repeats stay in: removed clips are numbered from these points, so a repeat
        // must fail the spacing check below rather than quietly renumber them.
        points.clone()
    } else if let Some(count) = o.equal_clips {
        if !(2..=10000).contains(&count) {
            return Err(JobError::invalid_options(
                "equal_clips must be between 2 and 10000",
            ));
        }
        (1..count)
            .map(|i| total * f64::from(i) / f64::from(count))
            .collect()
    } else {
        let every = o.segment_seconds.unwrap_or(60.0);
        if !every.is_finite() || every <= 0.0 || total / every > 10000.0 {
            return Err(JobError::invalid_options(
                "segment_seconds must give at most 10000 clips",
            ));
        }
        let count = ((total - 0.05) / every).floor() as u32;
        (1..=count).map(|i| f64::from(i) * every).collect()
    };
    if points
        .iter()
        .any(|t| !t.is_finite() || *t <= 0.0 || *t >= total)
    {
        return Err(JobError::invalid_options(
            "split points must fall inside the video",
        ));
    }
    points.sort_by(f64::total_cmp);
    let mut bounds = vec![0.0];
    bounds.append(&mut points);
    bounds.push(total);
    if bounds.windows(2).any(|w| w[1] - w[0] < 0.05) {
        return Err(JobError::invalid_options(
            "split points must be at least 0.05 seconds apart and from either end",
        ));
    }
    Ok(bounds)
}

/// An ffmpeg output pattern inside `dir`, with `%` in the directory escaped.
fn pattern_in(dir: &Path, file_pattern: &str) -> OsString {
    let mut s = OsString::from(dir.to_string_lossy().replace('%', "%%"));
    s.push("/");
    s.push(file_pattern);
    s
}

fn non_empty_dir(d: &Path) -> JobResult<()> {
    if std::fs::read_dir(d)?.next().is_none() {
        return Err(JobError::tool_failed("no output files were produced"));
    }
    Ok(())
}

fn concat_escape(p: &Path) -> String {
    p.to_string_lossy().replace('\'', "'\\''")
}

/// Join videos (`joined.<ext of first>`): stream copy when all streams match,
/// otherwise re-encode through the concat filter.
pub fn join(ctx: &Ctx, inputs: &[Input], out_dir: &Path) -> JobResult<PathBuf> {
    let ext = inputs[0].ext().to_string();
    let probes: Vec<Probe> = inputs.iter().map(|i| probe(ctx, i)).collect();
    let total: Option<f64> = probes.iter().map(|p| p.duration).sum();
    let list_safe = inputs
        .iter()
        .all(|i| !i.path.to_string_lossy().contains(['\n', '\r']));
    let same = list_safe
        && ctx.deps.has("ffprobe")
        && inputs.iter().all(|i| i.ext() == ext)
        && probes
            .iter()
            .all(|p| !p.signature.is_empty() && p.signature == probes[0].signature);
    let staged = Staged::file(out_dir, "joined", &ext)?;
    let work = WorkDir::new_in(out_dir)?;
    let mut cmd = ffmpeg(ctx)?;
    if same {
        let list = work.join("list.txt");
        let mut f = std::fs::File::create(&list)?;
        for i in inputs {
            writeln!(f, "file '{}'", concat_escape(&i.path))?;
        }
        drop(f);
        cmd = cmd
            .args(["-f", "concat", "-safe", "0", "-i"])
            .arg(&list)
            .args(["-map", "0", "-c", "copy"]);
    } else {
        let all_audio = probes.iter().all(|p| p.has_audio);
        // Clips are squared below, so a non-square first clip sets the canvas at its
        // displayed width. The cap keeps a damaged pixel aspect from asking for a huge frame.
        let stored = f64::from(probes[0].width.unwrap_or(1280));
        let displayed = (stored * probes[0].sar.unwrap_or(1.0)).min(16384.0) as u32;
        let (w, h) = (
            (displayed / 2 * 2).max(2),
            probes[0].height.unwrap_or(720) / 2 * 2,
        );
        let mut graph = String::new();
        for (n, i) in inputs.iter().enumerate() {
            cmd = cmd.arg("-i").arg(out(&i.path));
            graph.push_str(&format!(
                "[{n}:v:0]scale='iw*sar':ih,setsar=1,scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=30,format=yuv420p[v{n}];"
            ));
            if all_audio {
                graph.push_str(&format!(
                    "[{n}:a:0]aresample=48000,aformat=channel_layouts=stereo[a{n}];"
                ));
            }
        }
        for n in 0..inputs.len() {
            graph.push_str(&format!("[v{n}]"));
            if all_audio {
                graph.push_str(&format!("[a{n}]"));
            }
        }
        graph.push_str(&format!(
            "concat=n={}:v=1:a={}[v]{}",
            inputs.len(),
            u8::from(all_audio),
            if all_audio { "[a]" } else { "" }
        ));
        cmd = cmd.args(["-filter_complex", &graph, "-map", "[v]"]);
        if all_audio {
            cmd = cmd.args(["-map", "[a]"]).args(audio_codec(&ext));
        }
        let mut v = video_codec(&ext, None);
        // The graph already produces even sizes; drop the extra -vf.
        if let Some(idx) = v.iter().position(|x| x == "-vf") {
            v.drain(idx..idx + 2);
        }
        cmd = cmd.args(v);
    }
    cmd = cmd.args(["-f", muxer(&ext)]).arg(out(staged.path()));
    run(ctx, cmd, total, "Joining")?;
    drop(work);
    Ok(staged.commit()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atempo() {
        assert_eq!(atempo_chain(4.0), "atempo=2.0,atempo=2.000000");
        assert_eq!(atempo_chain(0.25), "atempo=0.5,atempo=0.500000");
        assert_eq!(atempo_chain(1.5), "atempo=1.500000");
    }
}

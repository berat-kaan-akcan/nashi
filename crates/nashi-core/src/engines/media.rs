//! Audio, video and GIF via ffmpeg.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::{Ctx, Input, Produced};
use crate::catalog::{category_of, Category};
use crate::deps::require;
use crate::detect::{ffprobe, Probe};
use crate::events::{JobError, JobResult};
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
        "avi" => s(&["-c:v", "mpeg4", "-q:v", "3", "-vtag", "xvid"]),
        "wmv" => s(&["-c:v", "wmv2", "-b:v", "2500k"]),
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
    let p = probe(ctx, input);
    let staged = input.stage("", target)?;
    let mut cmd = ffmpeg(ctx)?.arg("-i").arg_cat("file:", &input.path);
    let tcat = category_of(target);
    match tcat {
        Category::Audio => {
            if !p.has_audio {
                return Err(JobError::invalid_input("the file has no audio stream"));
            }
            cmd = cmd.args(["-vn", "-sn", "-dn", "-map", "0:a:0", "-map_metadata", "0"]);
            cmd = cmd.args(audio_codec(target));
        }
        Category::Gif => {
            cmd = cmd.args([
                "-an",
                "-filter_complex",
                "[0:v:0]fps=12,scale='min(640,iw)':-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse",
                "-loop",
                "0",
            ]);
        }
        Category::Video => {
            if !p.has_video {
                return Err(JobError::invalid_input("the file has no video stream"));
            }
            cmd = cmd.args(["-map", "0:v:0"]);
            if p.has_audio && input.category() != Category::Gif {
                cmd = cmd.args(["-map", "0:a:0?"]);
                cmd = cmd.args(audio_codec(target));
            } else {
                cmd = cmd.arg("-an");
            }
            cmd = cmd
                .args(video_codec(target, None))
                .args(["-map_metadata", "0"]);
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

/// Single-input audio/video tools.
pub fn tool(ctx: &Ctx, tool: &str, input: &Input) -> JobResult<Produced> {
    let o = ctx.opts;
    let ext = input.ext().to_string();
    let p = probe(ctx, input);
    let is_video = input.category() == Category::Video;
    let ffin = |c: Cmd| c.arg("-i").arg_cat("file:", &input.path);
    // Encoder args to re-encode into the same container.
    let reencode = |vf: Option<String>, af: Option<String>| -> Vec<String> {
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
            a.extend(["-vn".into(), "-map".into(), "0:a:0".into()]);
        }
        if p.has_audio {
            a.extend(audio_codec(&ext).into_iter().map(String::from));
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
        "compress" => {
            let staged = input.stage("-compressed", &ext)?;
            let mut cmd = ffin(ffmpeg(ctx)?);
            if is_video {
                let mut v = video_codec(&ext, Some(o.crf.unwrap_or(26)));
                v.extend(["-map".into(), "0:v:0".into(), "-map".into(), "0:a?".into()]);
                cmd = cmd.args(v);
                cmd = if ext == "webm" {
                    cmd.args(["-c:a", "libopus", "-b:a", "96k"])
                } else {
                    cmd.args(["-c:a", "aac", "-b:a", "128k"])
                };
            } else {
                let a: Vec<&str> = match ext.as_str() {
                    "mp3" => vec!["-c:a", "libmp3lame", "-b:a", "128k"],
                    "m4a" => vec!["-c:a", "aac", "-b:a", "128k"],
                    "flac" => vec!["-c:a", "flac", "-compression_level", "12"],
                    "wav" => vec!["-c:a", "pcm_s16le", "-ar", "22050"],
                    "ogg" => vec!["-c:a", "libvorbis", "-q:a", "3"],
                    "opus" => vec!["-c:a", "libopus", "-b:a", "64k"],
                    _ => audio_codec(&ext),
                };
                cmd = cmd.args(["-vn", "-map", "0:a:0"]).args(a);
            }
            finish(cmd, staged, p.duration, "Compressing")
        }
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
            let staged = input.stage("-normalized", &ext)?;
            let rate = p.sample_rate.unwrap_or(48000).to_string();
            let cmd = ffin(ffmpeg(ctx)?)
                .args([
                    "-vn",
                    "-map",
                    "0:a:0",
                    "-af",
                    "loudnorm=I=-16:TP=-1.5:LRA=11",
                    "-ar",
                    &rate,
                ])
                .args(audio_codec(&ext));
            finish(cmd, staged, p.duration, "Normalizing")
        }
        "audioChannels" => {
            let ch = o.channels.as_deref().unwrap_or("mono");
            let staged = input.stage(&format!("-{ch}"), &ext)?;
            let n = if ch == "mono" { "1" } else { "2" };
            let cmd = ffin(ffmpeg(ctx)?)
                .args(["-vn", "-map", "0:a:0", "-ac", n])
                .args(audio_codec(&ext));
            finish(cmd, staged, p.duration, "Converting channels")
        }
        "trimVideo" | "trimAudio" => {
            let (targs, dur) = trim_args(ctx, &p)?;
            let staged = input.stage("-trimmed", &ext)?;
            let cmd = ffmpeg(ctx)?.args(targs);
            let cmd = ffin(cmd).args(reencode(None, None));
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
            ));
            finish(cmd, staged, p.duration.map(|d| d / s), "Changing speed")
        }
        "cropVideo" => {
            let r = o
                .rect
                .ok_or_else(|| JobError::invalid_options("cropVideo needs a rect option"))?;
            let staged = input.stage("-cropped", &ext)?;
            let f = format!(
                "crop={}:{}:{}:{}",
                r.w.round(),
                r.h.round(),
                r.x.round(),
                r.y.round()
            );
            let cmd = ffin(ffmpeg(ctx)?).args(reencode(Some(f), None));
            finish(cmd, staged, p.duration, "Cropping")
        }
        "redactVideo" => {
            let rects = o
                .rects
                .as_ref()
                .filter(|r| !r.is_empty())
                .ok_or_else(|| JobError::invalid_options("redactVideo needs rects"))?;
            let f = rects
                .iter()
                .map(|r| {
                    let en = match (r.t0, r.t1) {
                        (None, None) => String::new(),
                        (a, b) => format!(
                            ":enable='between(t,{},{})'",
                            a.unwrap_or(0.0),
                            b.unwrap_or(1e9)
                        ),
                    };
                    format!(
                        "drawbox=x={}:y={}:w={}:h={}:color=black:t=fill{en}",
                        r.x.round(),
                        r.y.round(),
                        r.w.round(),
                        r.h.round()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let staged = input.stage("-redacted", &ext)?;
            let cmd = ffin(ffmpeg(ctx)?).args(reencode(Some(f), None));
            finish(cmd, staged, p.duration, "Redacting")
        }
        "redactAudio" => {
            let ranges = o
                .ranges
                .as_ref()
                .filter(|r| !r.is_empty())
                .ok_or_else(|| JobError::invalid_options("redactAudio needs ranges"))?;
            let f = ranges
                .iter()
                .map(|[a, b]| format!("volume=0:enable='between(t,{a},{b})'"))
                .collect::<Vec<_>>()
                .join(",");
            let staged = input.stage("-bleeped", &ext)?;
            let cmd = ffin(ffmpeg(ctx)?)
                .args(["-vn", "-map", "0:a:0", "-af", &f])
                .args(audio_codec(&ext));
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
                    &format!("fps={fps}"),
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
        let (w, h) = (
            probes[0].width.unwrap_or(1280) / 2 * 2,
            probes[0].height.unwrap_or(720) / 2 * 2,
        );
        let mut graph = String::new();
        for (n, i) in inputs.iter().enumerate() {
            cmd = cmd.arg("-i").arg(out(&i.path));
            graph.push_str(&format!(
                "[{n}:v:0]scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=30,format=yuv420p[v{n}];"
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

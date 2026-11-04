//! Reading and editing metadata fields: exiftool for images and PDFs, ffmpeg
//! tags for audio and video. Every edit writes a new file.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use serde::Serialize;

use super::{media, Ctx, Input, Produced};
use crate::catalog::Category;
use crate::deps::{require, Deps};
use crate::detect::detect;
use crate::events::{JobError, JobResult};
use crate::fsops::WorkDir;
use crate::process::Cmd;

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    pub key: String,
    pub value: String,
    pub editable: bool,
}

/// exiftool groups that describe the file or the tool rather than its metadata.
const HIDDEN_GROUPS: &[&str] = &["ExifTool", "System", "File", "Composite"];

/// exiftool groups it can write, as far as the fields a person would edit.
fn writable_group(group: &str) -> bool {
    group.starts_with("XMP")
        || matches!(
            group,
            "IFD0" | "IFD1" | "ExifIFD" | "GPS" | "IPTC" | "PDF" | "Photoshop" | "InteropIFD"
        )
}

fn value_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The metadata fields of `path`, sorted by key.
pub fn read(deps: &Deps, path: &Path) -> JobResult<Vec<Field>> {
    let category = detect(path).category;
    let mut fields = match category {
        Category::Image | Category::Gif | Category::Pdf => read_exiftool(deps, path)?,
        Category::Audio | Category::Video => read_tags(deps, path)?,
        _ => {
            return Err(JobError::unsupported(
                "this kind of file has no editable metadata",
            ))
        }
    };
    fields.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(fields)
}

fn read_exiftool(deps: &Deps, path: &Path) -> JobResult<Vec<Field>> {
    let out = Cmd::new(require(deps, "exiftool")?)
        .args(["-json", "-G1", "-s", "--"])
        .arg(path)
        .run_ok()?;
    let bad = || JobError::tool_failed("exiftool wrote unreadable metadata");
    let root: serde_json::Value = serde_json::from_str(&out.stdout).map_err(|_| bad())?;
    let tags = root[0].as_object().ok_or_else(bad)?;
    Ok(tags
        .iter()
        .filter_map(|(key, value)| {
            let (group, _) = key.split_once(':')?;
            if HIDDEN_GROUPS.contains(&group) {
                return None;
            }
            let value = value_text(value);
            Some(Field {
                key: key.clone(),
                editable: writable_group(group) && !value.starts_with("(Binary data"),
                value,
            })
        })
        .collect())
}

fn read_tags(deps: &Deps, path: &Path) -> JobResult<Vec<Field>> {
    let mut source = OsString::from("file:");
    source.push(path.as_os_str());
    let out = Cmd::new(require(deps, "ffprobe")?)
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(source)
        .run_ok()?;
    let root: serde_json::Value = serde_json::from_str(&out.stdout)
        .map_err(|_| JobError::tool_failed("ffprobe wrote unreadable metadata"))?;
    let tags = root["format"]["tags"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    Ok(tags
        .iter()
        .map(|(key, value)| Field {
            key: key.clone(),
            value: value_text(value),
            editable: true,
        })
        .collect())
}

/// Whether the options ask for individual fields rather than removing everything.
pub fn wants_edit(ctx: &Ctx) -> bool {
    ctx.opts.metadata_set.is_some() || ctx.opts.metadata_remove.is_some()
}

/// A key is passed to the tool as part of an argument, so it may only hold what
/// tag names hold: nothing that starts an option or redirects a value.
fn valid_key(key: &str, allow_group: bool) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') || (allow_group && c == ':')
        })
}

/// Apply `metadata_set` and `metadata_remove` to a copy of the input.
pub fn edit(ctx: &Ctx, input: &Input) -> JobResult<Produced> {
    let set = ctx.opts.metadata_set.clone().unwrap_or_default();
    let remove = ctx.opts.metadata_remove.clone().unwrap_or_default();
    if set.is_empty() && remove.is_empty() {
        return Err(JobError::invalid_options(
            "metadata_set or metadata_remove must name a field",
        ));
    }
    let media_file = matches!(input.category(), Category::Audio | Category::Video);
    for key in set.keys().chain(remove.iter()) {
        if !valid_key(key, !media_file) {
            return Err(JobError::invalid_options(format!(
                "'{key}' is not a metadata field name"
            )));
        }
    }
    ctx.progress(None, "Saving metadata");
    if media_file {
        return media::edit_tags(ctx, input, &set, &remove);
    }
    edit_exiftool(ctx, input, &set, &remove)
}

fn edit_exiftool(
    ctx: &Ctx,
    input: &Input,
    set: &BTreeMap<String, String>,
    remove: &[String],
) -> JobResult<Produced> {
    let exif = require(ctx.deps, "exiftool")?;
    let ext = input.ext();
    let staged = input.stage("-metadata", ext)?;
    let work = WorkDir::new_in(&input.out_dir)?;
    // exiftool refuses to write over a file, so it gets a name of its own.
    let edited = work.join(&format!("edited.{ext}"));
    let mut cmd = Cmd::new(exif);
    for (key, value) in set {
        cmd = cmd.arg(format!("-{key}={value}"));
    }
    for key in remove {
        cmd = cmd.arg(format!("-{key}="));
    }
    let out = cmd
        .arg("-o")
        .arg(&edited)
        .arg("--")
        .arg(&input.path)
        .run()?;
    // A tag it cannot write is only a warning, and the others would still be saved.
    if [
        "Warning: Sorry",
        "not writable",
        "isn't writable",
        "not defined",
    ]
    .iter()
    .any(|w| out.stderr.contains(w))
    {
        return Err(JobError::invalid_options(
            "one of these fields cannot be written to this file",
        ));
    }
    if !out.status.success() || !edited.exists() {
        return Err(crate::process::failure("exiftool", &out));
    }
    if input.category() != Category::Pdf {
        staged.take_from(&edited)?;
        return Ok(staged.commit()?.into());
    }
    // exiftool edits PDFs incrementally, so the old values stay recoverable
    // until the file is rewritten.
    let Some(qpdf) = ctx.deps.path("qpdf") else {
        staged.take_from(&edited)?;
        return Ok(Produced {
            paths: vec![staged.commit()?],
            message: Some(
                "The old values may still be recoverable from this PDF. Install qpdf to remove them."
                    .into(),
            ),
        });
    };
    let out = Cmd::new(qpdf)
        .arg("--linearize")
        .arg(&edited)
        .arg(staged.path())
        .run()?;
    // qpdf exit 3 = warnings, output still written.
    if !matches!(out.status.code(), Some(0) | Some(3)) {
        return Err(crate::process::failure("qpdf", &out));
    }
    Ok(staged.commit()?.into())
}

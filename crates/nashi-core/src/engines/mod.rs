//! Conversion and tool engines, dispatched by category.

pub mod archive;
pub mod epub;
pub mod image;
pub mod media;
pub mod office;
pub mod pdf;
pub mod subtitle;
pub mod textclip;
pub mod zipw;

use std::path::{Path, PathBuf};

use crate::catalog::{self, Category};
use crate::deps::Deps;
use crate::detect::Detected;
use crate::events::{Event, JobError, JobResult, Sink};
use crate::fsops::Staged;
use crate::options::Options;

/// One input file.
#[derive(Debug, Clone)]
pub struct Input {
    /// Absolute path (never starts with `-`).
    pub path: PathBuf,
    pub det: Detected,
    /// Destination directory for outputs.
    pub out_dir: PathBuf,
}

impl Input {
    pub fn ext(&self) -> &str {
        &self.det.ext
    }
    pub fn stem(&self) -> &str {
        &self.det.stem
    }
    pub fn category(&self) -> Category {
        self.det.category
    }
    /// Reserve `out_dir/<stem><suffix>.<ext>`.
    pub fn stage(&self, suffix: &str, ext: &str) -> JobResult<Staged> {
        Ok(Staged::file(
            &self.out_dir,
            &format!("{}{suffix}", self.stem()),
            ext,
        )?)
    }
    /// Reserve the folder `out_dir/<stem><suffix>/`.
    pub fn stage_dir(&self, suffix: &str) -> JobResult<Staged> {
        Ok(Staged::dir(
            &self.out_dir,
            &format!("{}{suffix}", self.stem()),
        )?)
    }
}

/// Per-item context.
pub struct Ctx<'a> {
    pub deps: &'a Deps,
    pub opts: &'a Options,
    pub sink: &'a dyn Sink,
    pub input: Option<String>,
    pub index: Option<usize>,
}

impl Ctx<'_> {
    pub fn progress(&self, fraction: Option<f64>, message: &str) {
        self.sink.emit(&Event::Progress {
            input: self.input.clone(),
            index: self.index,
            fraction,
            message: message.to_string(),
        });
    }
}

/// Successful result of one item.
#[derive(Debug, Default)]
pub struct Produced {
    pub paths: Vec<PathBuf>,
    pub message: Option<String>,
}

impl From<PathBuf> for Produced {
    fn from(p: PathBuf) -> Self {
        Produced {
            paths: vec![p],
            message: None,
        }
    }
}

/// Check route/tool requirements up-front for a clean `missing_dependency`.
pub fn check_requirements(deps: &Deps, req: &[catalog::AnyOf]) -> JobResult<()> {
    let miss = catalog::missing(req, deps);
    if miss.is_empty() {
        return Ok(());
    }
    let pkgs: Vec<String> = miss
        .iter()
        .map(|m| {
            crate::deps::DEPS
                .iter()
                .find(|d| d.id == m)
                .map(|d| d.package.to_string())
                .unwrap_or_else(|| m.clone())
        })
        .collect();
    Err(JobError::new(
        crate::events::ErrorCode::MissingDependency,
        format!("missing {} (install: {})", miss.join(", "), pkgs.join(", ")),
    ))
}

/// Convert one input to `target`.
pub fn convert(ctx: &Ctx, input: &Input, target: &str) -> JobResult<Produced> {
    let src = input.ext().to_string();
    if input.category() == Category::Unknown {
        return Err(JobError::unsupported(format!(
            "unknown file type{}",
            if src.is_empty() {
                String::new()
            } else {
                format!(" .{src}")
            }
        )));
    }
    if src == target {
        return Err(JobError::unsupported(format!("file is already .{target}")));
    }
    let req = catalog::route_requirements(&src, target).map_err(JobError::unsupported)?;
    check_requirements(ctx.deps, &req)?;
    match input.category() {
        Category::Image => image::convert(ctx, input, target).map(Into::into),
        Category::Audio | Category::Video | Category::Gif => {
            media::convert(ctx, input, target).map(Into::into)
        }
        Category::Pdf => pdf::convert(ctx, input, target),
        Category::Subtitle => subtitle::convert(ctx, input, target).map(Into::into),
        Category::Archive => archive::convert(ctx, input, target).map(Into::into),
        Category::Document | Category::Presentation | Category::Spreadsheet | Category::Text => {
            office::convert(ctx, input, target)
        }
        Category::Unknown => unreachable!(),
    }
}

/// Run a single-input tool on one file.
pub fn tool_single(ctx: &Ctx, tool: &str, input: &Input) -> JobResult<Produced> {
    if !catalog::tool_applies(tool, input.ext()) {
        return Err(JobError::unsupported(format!(
            "{tool} does not apply to .{} files",
            input.ext()
        )));
    }
    let req = catalog::tool_requirements(tool, input.category(), input.ext()).ok_or_else(|| {
        JobError::unsupported(format!("{tool} does not apply to .{}", input.ext()))
    })?;
    check_requirements(ctx.deps, &req)?;
    match input.category() {
        Category::Image | Category::Gif if tool == "removeMetadata" => {
            image::remove_metadata(ctx, input)
        }
        Category::Image => image::tool(ctx, tool, input),
        Category::Audio | Category::Video => media::tool(ctx, tool, input),
        Category::Pdf => pdf::tool(ctx, tool, input),
        Category::Archive => archive::extract_tool(ctx, input).map(Into::into),
        _ => Err(JobError::unsupported(format!(
            "{tool} is not available for .{}",
            input.ext()
        ))),
    }
}

/// Run a multi-input tool on all files at once.
pub fn tool_multi(ctx: &Ctx, tool: &str, inputs: &[Input], out_dir: &Path) -> JobResult<Produced> {
    let need = |cat: Category, what: &str| -> JobResult<()> {
        if let Some(bad) = inputs.iter().find(|i| i.category() != cat) {
            return Err(JobError::invalid_input(format!(
                "{tool} needs {what} files; {} is .{}",
                bad.path.display(),
                bad.ext()
            )));
        }
        Ok(())
    };
    let check_all = |cat: Category| -> JobResult<()> {
        for i in inputs {
            let req = catalog::tool_requirements(tool, cat, i.ext()).ok_or_else(|| {
                JobError::unsupported(format!("{tool} does not apply to .{}", i.ext()))
            })?;
            check_requirements(ctx.deps, &req)?;
        }
        Ok(())
    };
    match tool {
        "createPDF" | "createCollage" => {
            need(Category::Image, "image")?;
            check_all(Category::Image)?;
            if tool == "createPDF" {
                image::create_pdf(ctx, inputs, out_dir).map(Into::into)
            } else {
                image::collage(ctx, inputs, out_dir).map(Into::into)
            }
        }
        "mergePDF" => {
            need(Category::Pdf, "PDF")?;
            check_all(Category::Pdf)?;
            if inputs.len() < 2 {
                return Err(JobError::invalid_input("mergePDF needs at least two PDFs"));
            }
            pdf::merge(ctx, inputs, out_dir).map(Into::into)
        }
        "joinVideos" => {
            need(Category::Video, "video")?;
            check_all(Category::Video)?;
            if inputs.len() < 2 {
                return Err(JobError::invalid_input(
                    "joinVideos needs at least two videos",
                ));
            }
            media::join(ctx, inputs, out_dir).map(Into::into)
        }
        _ => Err(JobError::unsupported(format!(
            "{tool} is not a multi-input tool"
        ))),
    }
}

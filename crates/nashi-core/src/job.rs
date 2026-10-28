//! Job orchestration: one `start`, per-input progress/output/error, one `done`.

use std::path::{Path, PathBuf};

use crate::catalog;
use crate::deps::Deps;
use crate::detect::detect;
use crate::engines::{self, Ctx, Input, Produced};
use crate::events::{path_str, ErrorCode, Event, JobError, JobResult, Sink};
use crate::options::Options;
use crate::process::is_cancelled;

/// What a job is.
#[derive(Debug, Clone)]
pub enum JobKind {
    Convert(String),
    Tool(String),
}

/// Outcome summary (the CLI derives its exit code from it).
#[derive(Debug, Default)]
pub struct Summary {
    pub outputs: Vec<PathBuf>,
    pub failed: usize,
    pub total: usize,
    pub cancelled: bool,
    pub error_codes: Vec<ErrorCode>,
}

impl Summary {
    /// Exit code per protocol.
    pub fn exit_code(&self) -> i32 {
        if self.cancelled {
            130
        } else if self.failed == 0 {
            0
        } else if self.failed >= self.total
            && !self.error_codes.is_empty()
            && self
                .error_codes
                .iter()
                .all(|c| *c == ErrorCode::MissingDependency)
        {
            3
        } else {
            1
        }
    }
}

/// Validate a job before `start` (usage errors → exit 2).
pub fn validate(kind: &JobKind, files: &[PathBuf]) -> Result<(), String> {
    if files.is_empty() {
        return Err("no input files given".into());
    }
    match kind {
        JobKind::Convert(t) => {
            let known = catalog::upstream()
                .conversion_matrix
                .values()
                .any(|v| v.iter().any(|x| x == t));
            if !known {
                return Err(format!("unknown conversion target '{t}'"));
            }
        }
        JobKind::Tool(id) => {
            if catalog::tool_def(id).is_none() {
                return Err(format!("unknown tool '{id}'"));
            }
        }
    }
    Ok(())
}

fn job_id() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!(
        "{:04x}",
        (t ^ std::process::id().wrapping_mul(2_654_435_761)) & 0xffff
    )
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

fn make_input(path: &Path, out_dir: Option<&Path>) -> JobResult<Input> {
    let path = absolute(path);
    let meta = std::fs::metadata(&path)
        .map_err(|e| JobError::invalid_input(format!("{}: {e}", path.display())))?;
    let det = detect(&path);
    if meta.is_dir() && det.ext != "rtfd" {
        return Err(JobError::invalid_input(format!(
            "{} is a directory",
            path.display()
        )));
    }
    let out_dir = match out_dir {
        Some(d) => absolute(d),
        None => path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("/")),
    };
    Ok(Input { path, det, out_dir })
}

/// Run a job, emitting events to `sink`.
pub fn run(
    kind: &JobKind,
    files: &[PathBuf],
    out_dir: Option<&Path>,
    opts: &Options,
    deps: &Deps,
    sink: &dyn Sink,
) -> Summary {
    let mut sum = Summary {
        total: files.len(),
        ..Default::default()
    };
    sink.emit(&Event::Start {
        job: job_id(),
        total: files.len(),
    });
    if let Some(d) = out_dir {
        if let Err(e) = std::fs::create_dir_all(d) {
            let err = JobError::from(e);
            sink.emit(&Event::Error {
                input: None,
                index: None,
                code: err.code,
                message: format!("--out-dir: {}", err.message),
            });
            sum.failed = sum.total;
            sum.error_codes.push(err.code);
            finish(sink, &mut sum);
            return sum;
        }
    }

    let multi =
        matches!(kind, JobKind::Tool(t) if catalog::tool_def(t).is_some_and(|d| d.multi_input));
    if multi {
        let JobKind::Tool(tool) = kind else {
            unreachable!()
        };
        let ctx = Ctx {
            deps,
            opts,
            sink,
            input: None,
            index: None,
        };
        let result = files
            .iter()
            .map(|f| make_input(f, out_dir))
            .collect::<JobResult<Vec<Input>>>()
            .and_then(|inputs| {
                let dir = out_dir
                    .map(absolute)
                    .unwrap_or_else(|| inputs[0].out_dir.clone());
                engines::tool_multi(&ctx, tool, &inputs, &dir)
            });
        report(sink, &mut sum, None, None, result, files.len());
    } else {
        for (i, f) in files.iter().enumerate() {
            if is_cancelled() {
                break;
            }
            let shown = path_str(&absolute(f));
            let ctx = Ctx {
                deps,
                opts,
                sink,
                input: Some(shown.clone()),
                index: Some(i),
            };
            let result = make_input(f, out_dir).and_then(|input| match kind {
                JobKind::Convert(t) => engines::convert(&ctx, &input, t),
                JobKind::Tool(t) => engines::tool_single(&ctx, t, &input),
            });
            report(sink, &mut sum, Some(shown), Some(i), result, 1);
        }
    }
    if is_cancelled() {
        sum.cancelled = true;
    }
    finish(sink, &mut sum);
    sum
}

fn report(
    sink: &dyn Sink,
    sum: &mut Summary,
    input: Option<String>,
    index: Option<usize>,
    r: JobResult<Produced>,
    weight: usize,
) {
    match r {
        Ok(p) => {
            for path in &p.paths {
                sink.emit(&Event::Output {
                    input: input.clone(),
                    index,
                    path: path_str(path),
                    message: p.message.clone(),
                });
            }
            sum.outputs.extend(p.paths);
        }
        Err(e) if e.code == ErrorCode::Cancelled || is_cancelled() => {
            sum.cancelled = true;
        }
        Err(e) => {
            sink.emit(&Event::Error {
                input,
                index,
                code: e.code,
                message: e.message,
            });
            sum.failed += weight;
            sum.error_codes.push(e.code);
        }
    }
}

fn finish(sink: &dyn Sink, sum: &mut Summary) {
    sink.emit(&Event::Done {
        outputs: sum.outputs.iter().map(|p| path_str(p)).collect(),
        failed: sum.failed,
        cancelled: sum.cancelled,
    });
}

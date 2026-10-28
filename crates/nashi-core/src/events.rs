//! Job events (the JSON-lines stream of `docs/CLI-PROTOCOL.md`) and errors.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

/// Protocol error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnsupportedRoute,
    MissingDependency,
    ToolFailed,
    InvalidInput,
    InvalidOptions,
    Io,
    Cancelled,
}

/// Failure of one job item.
#[derive(Debug, Clone)]
pub struct JobError {
    pub code: ErrorCode,
    pub message: String,
}

impl JobError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        JobError {
            code,
            message: message.into(),
        }
    }
    pub fn unsupported(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::UnsupportedRoute, m)
    }
    pub fn invalid_input(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, m)
    }
    pub fn invalid_options(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidOptions, m)
    }
    pub fn tool_failed(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::ToolFailed, m)
    }
    pub fn cancelled() -> Self {
        Self::new(ErrorCode::Cancelled, "Cancelled")
    }
    pub fn missing(tool: &str, package: &str) -> Self {
        Self::new(
            ErrorCode::MissingDependency,
            format!("{tool} is not installed (install the '{package}' package)"),
        )
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for JobError {}

impl From<std::io::Error> for JobError {
    fn from(e: std::io::Error) -> Self {
        JobError::new(ErrorCode::Io, e.to_string())
    }
}

pub type JobResult<T> = Result<T, JobError>;

/// One event of the stream.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Start {
        job: String,
        total: usize,
    },
    Progress {
        input: Option<String>,
        index: Option<usize>,
        fraction: Option<f64>,
        message: String,
    },
    Output {
        input: Option<String>,
        index: Option<usize>,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Error {
        input: Option<String>,
        index: Option<usize>,
        code: ErrorCode,
        message: String,
    },
    Done {
        outputs: Vec<String>,
        failed: usize,
        cancelled: bool,
    },
}

/// Receiver of events (the CLI prints them).
pub trait Sink {
    fn emit(&self, event: &Event);
}

/// Sink that collects events (tests, library use).
#[derive(Default)]
pub struct VecSink(pub std::sync::Mutex<Vec<Event>>);

impl Sink for VecSink {
    fn emit(&self, event: &Event) {
        self.0.lock().unwrap().push(event.clone());
    }
}

pub fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

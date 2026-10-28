//! Nashi conversion engine: catalog, detection, dependency probing, and the
//! conversion/tool engines used by `nashi-cli` (see `docs/CLI-PROTOCOL.md`).

pub mod catalog;
pub mod deps;
pub mod detect;
pub mod engines;
pub mod events;
pub mod fsops;
pub mod job;
pub mod options;
pub mod process;

pub use catalog::Category;
pub use deps::Deps;
pub use events::{ErrorCode, Event, JobError, JobResult, Sink};
pub use job::{JobKind, Summary};
pub use options::Options;

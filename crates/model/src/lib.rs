//! Application state, and the traits that I/O backends implement for it.

mod config;
#[cfg(feature = "fixtures")]
pub mod fixtures;
mod git;
mod projects;

pub use config::{ConfigBackend, ConfigEvent, ConfigFiles};
pub use git::{GitBackend, Repository};
pub use projects::{Branch, Project, ProjectId, Projects, Work, WorkKind, Worktree};

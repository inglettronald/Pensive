//! Application state, and the traits that I/O backends implement for it.

#[cfg(feature = "fixtures")]
pub mod fixtures;
mod projects;

pub use projects::{Branch, Project, ProjectId, Projects, Work, WorkKind, Worktree};

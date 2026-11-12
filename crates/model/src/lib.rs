//! Application state, and the traits that I/O backends implement for it.

#[cfg(feature = "fixtures")]
pub mod fixtures;
mod git;
mod projects;

pub use git::{GitBackend, Repository};
pub use projects::{Branch, Project, ProjectId, Projects, Work, WorkKind, Worktree};

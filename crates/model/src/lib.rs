//! Application state, and the traits that I/O backends implement for it.

mod changes;
mod config;
#[cfg(feature = "fixtures")]
pub mod fixtures;
mod git;
mod projects;

pub use changes::{
    Base, Changes, Commit, FileChange, FileStatus, Hunk, HunkLine, LineChange, Staging, Symbol,
    SymbolKind, Upstream,
};
pub use config::{ConfigBackend, ConfigEvent, ConfigFiles};
pub use git::{GitBackend, Repository};
pub use projects::{Branch, Project, ProjectId, Projects, Work, WorkKind, Worktree};

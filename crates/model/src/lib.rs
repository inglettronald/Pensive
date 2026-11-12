//! Application state, and the traits that I/O backends implement for it.

mod changes;
#[cfg(feature = "fixtures")]
pub mod fixtures;
mod projects;

pub use changes::{
    Changes, Commit, FileChange, FileStatus, Hunk, HunkLine, LineChange, Symbol, SymbolKind,
    Upstream,
};
pub use projects::{Branch, Project, ProjectId, Projects, Work, WorkKind, Worktree};

//! Application state, and the traits that I/O backends implement for it.

mod backend;
mod changes;
mod config;
#[cfg(feature = "fixtures")]
pub mod fixtures;
mod git;
mod notice;
mod notices;
mod projects;

pub use backend::Backend;
pub use changes::{
    Base, Changes, Commit, FileChange, FileStatus, Hunk, HunkLine, LineChange, Staging, Symbol,
    SymbolKind, Upstream,
};
pub use config::{ConfigBackend, ConfigEvent, ConfigFiles};
pub use git::{GitBackend, Repository};
pub use notice::{FixtureGap, Notice, Origin, Severity};
pub use notices::{Held, Notices};
pub use projects::{Branch, Project, ProjectId, Projects, Work, WorkKind, Worktree};

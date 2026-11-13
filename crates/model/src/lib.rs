//! Application state, and the traits that I/O backends implement for it.

mod backend;
mod changes;
mod config;
mod diff;
#[cfg(feature = "fixtures")]
pub mod fixtures;
mod git;
mod notice;
mod notices;
mod projects;

pub use backend::Backend;
pub use changes::{
    Base, Changes, Commit, FileChange, FileKey, FileStatus, Hunk, HunkLine, LineChange, Source,
    Staging, Symbol, SymbolKind, Upstream, short_sha,
};
pub use config::{ConfigBackend, ConfigEvent, ConfigFiles};
pub use diff::{FileDiff, FileVersions, Version};
pub use git::{GitBackend, Repository};
pub use notice::{FixtureGap, Notice, Origin, Severity};
pub use notices::{Held, Notices};
pub use projects::{Branch, Project, ProjectId, Projects, Work, WorkKind, Worktree};

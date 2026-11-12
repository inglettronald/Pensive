//! The trait that Git backends implement. Calls block, so models run them off the UI thread.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{Branch, ProjectId, Worktree};

pub trait GitBackend: Send + Sync {
    /// The main worktree of the repository that contains `path`.
    fn root(&self, path: &Path) -> Result<PathBuf>;

    /// The checked-out worktrees of the repository at `root`, labeled as `project`'s.
    fn worktrees(&self, project: ProjectId, root: &Path) -> Result<Vec<Worktree>>;

    /// The local branches of the repository at `root`, labeled as `project`'s.
    fn branches(&self, project: ProjectId, root: &Path) -> Result<Vec<Branch>>;
}

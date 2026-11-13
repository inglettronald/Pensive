//! The trait that Git backends implement, and the repository it reads.

use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{Branch, Changes, FileKey, FileVersions, Worktree};

/// Calls block, so models run them off the UI thread.
pub trait GitBackend: Send + Sync + Debug {
    /// Reads the repository that contains `path`.
    fn repository(&self, path: &Path) -> Result<Repository>;

    /// Reads the uncommitted changes and the branch's commits in the worktree at `worktree`, with
    /// each file's line counts but not its lines.
    fn changes(&self, worktree: &Path) -> Result<Changes>;

    /// Reads both versions of the file `key` names. A renamed file's old version is at
    /// `renamed_from`.
    fn versions(
        &self,
        worktree: &Path,
        key: &FileKey,
        renamed_from: Option<&Path>,
    ) -> Result<FileVersions>;
}

/// A repository as Git reports it.
#[derive(Clone, Debug)]
pub struct Repository {
    /// The first folder `git worktree list` gives: the main worktree, or the repository itself
    /// when it's bare.
    pub root: PathBuf,
    /// The worktrees that are checked out, the main one first.
    pub worktrees: Vec<Worktree>,
    pub branches: Vec<Branch>,
}

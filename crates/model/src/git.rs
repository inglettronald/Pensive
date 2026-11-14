//! The trait that Git backends implement, and the repository it reads.

use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{Branch, Changes, FileKey, FileVersions, Version, Worktree};

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

    /// Reads the file at `path` in the worktree as it is on disk.
    fn read_file(&self, worktree: &Path, path: &Path) -> Result<Version>;

    /// Replaces the file at `path` in the worktree with `text`.
    fn write_file(&self, worktree: &Path, path: &Path, text: &str) -> Result<()>;

    /// Stages all of each file at `paths`, relative to the worktree's root, or unstages it back
    /// to its committed state. A path can be a deleted file, or the old path of a rename.
    fn set_staged(&self, worktree: &Path, paths: &[PathBuf], staged: bool) -> Result<()>;
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

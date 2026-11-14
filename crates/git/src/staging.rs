//! Moves whole files into Git's index, and back out to their committed state.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::command::git_with_input;

/// Stages all of each file at `paths`, relative to the worktree's root, or unstages it. A path can
/// be a file that was deleted, or, when unstaging, the old path of a rename. The paths go to Git
/// on its standard input, so any number fits, and literally, so a name such as `*.rs` matches only
/// itself.
pub(crate) fn set_staged(worktree: &Path, paths: &[PathBuf], staged: bool) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(path.as_os_str().as_encoded_bytes());
        input.push(0);
    }
    let command: &[&str] = if staged {
        &["add", "--all"]
    } else {
        // Unlike `git restore --staged`, `git reset` also works before the first commit.
        &["reset", "-q"]
    };
    let args = [&["--literal-pathspecs"], command, PATHS_FROM_STDIN].concat();
    git_with_input(worktree, &args, Some(&input)).map(drop)
}

const PATHS_FROM_STDIN: &[&str] = &["--pathspec-from-file=-", "--pathspec-file-nul"];

//! Lists a worktree's files with `git ls-files`: what Git tracks or would add, and what
//! `.gitignore` leaves out.

use std::path::{Path, PathBuf};

use anyhow::Result;
use model::WorktreeFiles;

use crate::GitCli;

/// Tracked files, and untracked ones `.gitignore` doesn't leave out.
const LISTED: &[&str] = &[
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
];
/// What `.gitignore` leaves out, with each wholly ignored folder as one entry ending in `/`.
const IGNORED: &[&str] = &[
    "ls-files",
    "-z",
    "--others",
    "--ignored",
    "--exclude-standard",
    "--directory",
];

pub(crate) fn files(cli: &GitCli, worktree: &Path) -> Result<WorktreeFiles> {
    let mut files: Vec<PathBuf> = entries(&cli.git(worktree, LISTED)?)
        // A tracked file deleted from disk is still in the index.
        .filter(|path| worktree.join(path).exists())
        .map(PathBuf::from)
        .collect();
    files.dedup();
    let (folders, ignored): (Vec<_>, Vec<_>) =
        entries(&cli.git(worktree, IGNORED)?).partition(|path| path.ends_with('/'));
    Ok(WorktreeFiles {
        files,
        ignored_files: ignored.into_iter().map(PathBuf::from).collect(),
        ignored_folders: folders
            .into_iter()
            .map(|folder| PathBuf::from(folder.trim_end_matches('/')))
            .collect(),
    })
}

/// The NUL-separated paths `git ls-files -z` prints.
fn entries(output: &[u8]) -> impl Iterator<Item = String> + '_ {
    output
        .split(|&byte| byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8_lossy(entry).into_owned())
}

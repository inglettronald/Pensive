//! The `git` commands this backend runs, each next to the parser for what it prints.

use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

pub(crate) const WORKTREES: &[&str] = &["worktree", "list", "--porcelain", "-z"];

pub(crate) struct ListedWorktree {
    pub path: PathBuf,
    /// The commit checked out, or `None` before the first commit.
    pub head: Option<String>,
    pub branch: Option<String>,
    /// False for a bare repository's own entry, and for a worktree Git knows is gone.
    pub checked_out: bool,
}

/// The main worktree, or the bare repository, comes first.
pub(crate) fn worktrees(output: &[u8]) -> Vec<ListedWorktree> {
    let mut worktrees = Vec::new();
    for field in output.split(|&byte| byte == 0) {
        if let Some(path) = field.strip_prefix(b"worktree ") {
            worktrees.push(ListedWorktree {
                path: path_from(path),
                head: None,
                branch: None,
                checked_out: true,
            });
            continue;
        }
        let Some(worktree) = worktrees.last_mut() else {
            continue;
        };
        if let Some(head) = field.strip_prefix(b"HEAD ") {
            // An unborn branch reports a HEAD of all zeros.
            worktree.head = head
                .iter()
                .any(|&byte| byte != b'0')
                .then(|| String::from_utf8_lossy(head).into_owned());
        } else if let Some(branch) = field.strip_prefix(b"branch refs/heads/") {
            worktree.branch = Some(String::from_utf8_lossy(branch).into_owned());
        } else if field == b"bare" || field.starts_with(b"prunable") {
            worktree.checked_out = false;
        }
    }
    worktrees
}

pub(crate) const BRANCHES: &[&str] = &[
    "for-each-ref",
    "--format=%(refname:strip=2)%00%(committerdate:unix)",
    "refs/heads",
];

/// Followed by the commits to look up.
pub(crate) const COMMIT_TIMES: &[&str] = &[
    "log",
    "--no-walk=unsorted",
    "--format=%H%x00%ct",
    "--end-of-options",
];

/// Reads `BRANCHES` and `COMMIT_TIMES`: a name and a Unix time on each line.
pub(crate) fn named_times(output: &[u8]) -> impl Iterator<Item = (String, SystemTime)> {
    output.split(|&byte| byte == b'\n').filter_map(|line| {
        let separator = line.iter().position(|&byte| byte == 0)?;
        let (name, seconds) = (&line[..separator], &line[separator + 1..]);
        let seconds = std::str::from_utf8(seconds).ok()?.trim().parse().ok()?;
        Some((
            String::from_utf8_lossy(name).into_owned(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
        ))
    })
}

pub(crate) const STATUS: &[&str] = &["status", "--porcelain=v1", "-z", "--no-renames"];

/// The changed paths, relative to the worktree.
pub(crate) fn status_paths(output: &[u8]) -> Vec<PathBuf> {
    output
        .split(|&byte| byte == 0)
        .filter_map(|entry| entry.get(3..))
        .filter(|path| !path.is_empty())
        .map(path_from)
        .collect()
}

/// Git prints paths as the file system stores them, which on Unix need not be UTF-8.
fn path_from(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};
        PathBuf::from(OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktrees_marks_bare_and_missing_entries() {
        let output = b"worktree /repo.git\0bare\0\0\
            worktree /wt/a\0HEAD 1111111111111111111111111111111111111111\0branch refs/heads/a\0\0\
            worktree /wt/gone\0HEAD 2222222222222222222222222222222222222222\0detached\0prunable gitdir file points to non-existent location\0\0";
        let listed = worktrees(output);
        let checked_out: Vec<_> = listed.iter().map(|worktree| worktree.checked_out).collect();
        assert_eq!(checked_out, [false, true, false]);
        assert_eq!(listed[1].branch.as_deref(), Some("a"));
        assert_eq!(listed[2].branch, None);
    }

    /// Git doesn't call a locked worktree prunable, even once its folder is gone, so the backend
    /// checks the folder itself.
    #[test]
    fn worktrees_reads_a_locked_entry() {
        let output = b"worktree /wt/locked\0HEAD 1111111111111111111111111111111111111111\0branch refs/heads/locked\0locked on a removable drive\0\0";
        let listed = worktrees(output);
        assert_eq!(listed[0].path, PathBuf::from("/wt/locked"));
        assert_eq!(listed[0].branch.as_deref(), Some("locked"));
        assert!(listed[0].checked_out);
    }

    #[test]
    fn worktrees_strips_only_the_ref_prefix() {
        let output = b"worktree /wt\0HEAD 1111111111111111111111111111111111111111\0branch refs/heads/refs/heads/weird\0\0";
        assert_eq!(
            worktrees(output)[0].branch.as_deref(),
            Some("refs/heads/weird")
        );
    }

    #[test]
    fn status_paths_keeps_spaces_in_names() {
        let output = b" M src/a file.rs\0?? new/\0";
        assert_eq!(
            status_paths(output),
            [PathBuf::from("src/a file.rs"), PathBuf::from("new/")]
        );
    }
}

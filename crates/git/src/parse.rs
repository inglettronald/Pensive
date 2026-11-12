//! Reads the machine-readable output of the `git` commands this backend runs.

use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

/// One entry of `git worktree list --porcelain -z`.
pub(crate) struct ListedWorktree {
    pub path: PathBuf,
    /// The commit checked out, or `None` before the first commit.
    pub head: Option<String>,
    pub branch: Option<String>,
    /// False for a bare repository's own entry, and for a worktree whose folder is gone.
    pub checked_out: bool,
}

/// Parses `git worktree list --porcelain -z`. The main worktree comes first.
pub(crate) fn worktrees(output: &[u8]) -> Vec<ListedWorktree> {
    let mut worktrees = Vec::new();
    for field in output.split(|&byte| byte == 0) {
        let field = String::from_utf8_lossy(field);
        if let Some(path) = field.strip_prefix("worktree ") {
            worktrees.push(ListedWorktree {
                path: PathBuf::from(path),
                head: None,
                branch: None,
                checked_out: true,
            });
            continue;
        }
        let Some(worktree) = worktrees.last_mut() else {
            continue;
        };
        if let Some(head) = field.strip_prefix("HEAD ") {
            // An unborn branch reports a HEAD of all zeros.
            worktree.head = head
                .bytes()
                .any(|byte| byte != b'0')
                .then(|| head.to_string());
        } else if let Some(branch) = field.strip_prefix("branch ") {
            worktree.branch = Some(branch.trim_start_matches("refs/heads/").to_string());
        } else if field == "bare" || field.starts_with("prunable") {
            worktree.checked_out = false;
        }
    }
    worktrees
}

/// Parses lines of a name, a NUL and a Unix time, as printed by `--format=<name>%00<time>`.
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

/// Parses `git status --porcelain=v1 -z --no-renames` into the paths it lists, relative to the
/// worktree.
pub(crate) fn status_paths(output: &[u8]) -> Vec<PathBuf> {
    output
        .split(|&byte| byte == 0)
        .filter_map(|entry| entry.get(3..))
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
        .collect()
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

    #[test]
    fn status_paths_keeps_spaces_in_names() {
        let output = b" M src/a file.rs\0?? new/\0";
        assert_eq!(
            status_paths(output),
            [PathBuf::from("src/a file.rs"), PathBuf::from("new/")]
        );
    }
}

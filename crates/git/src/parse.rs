//! The `git` commands this backend runs, each next to the parser for what it prints.

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use model::{FileChange, FileStatus};

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

pub(crate) const UNCOMMITTED: &[&str] = &[
    "status",
    "--porcelain=v2",
    "-z",
    "--untracked-files=all",
    "--find-renames",
];

/// Reads `UNCOMMITTED`: each changed file, with no line counts yet.
pub(crate) fn uncommitted(output: &[u8]) -> Vec<FileChange> {
    let mut files: Vec<FileChange> = Vec::new();
    let mut fields = output.split(|&byte| byte == 0);
    while let Some(entry) = fields.next() {
        let (kind, rest) = (entry.first(), entry.get(2..).unwrap_or_default());
        let file = match kind {
            // `1 XY sub mH mI mW hH hI path`
            Some(b'1') => {
                let (letters, path) = (rest.get(..2), nth_rest(rest, 7));
                letters.zip(path).map(|(letters, path)| {
                    let (index, worktree) = sides(letters);
                    file(path, None, index, worktree)
                })
            }
            // `2 XY sub mH mI mW hH hI Xscore path`, then the old path as its own field.
            Some(b'2') => {
                let from = fields.next().map(path_from);
                let (letters, path) = (rest.get(..2), nth_rest(rest, 8));
                letters.zip(path).map(|(letters, path)| {
                    let (index, worktree) = sides(letters);
                    file(path, from, index, worktree)
                })
            }
            // `u XY sub m1 m2 m3 mW h1 h2 h3 path`
            Some(b'u') => {
                nth_rest(rest, 9).map(|path| file(path, None, None, Some(FileStatus::Conflicted)))
            }
            Some(b'?') => {
                // `git rm --cached` lists a file as deleted and untracked both. It's one file.
                let path = path_from(rest);
                if let Some(deleted) = files.iter_mut().find(|file| file.path == path) {
                    deleted.worktree = Some(FileStatus::Untracked);
                    None
                } else {
                    Some(file(rest, None, None, Some(FileStatus::Untracked)))
                }
            }
            _ => None,
        };
        files.extend(file);
    }
    files
}

/// The staged and unstaged sides of a status's two letters.
fn sides(letters: &[u8]) -> (Option<FileStatus>, Option<FileStatus>) {
    (status(letters[0]), status(letters[1]))
}

/// A status letter as `git status` and `git log --raw` print it. Type changes count as
/// modified, and copies as added.
fn status(letter: u8) -> Option<FileStatus> {
    match letter {
        b'M' | b'T' => Some(FileStatus::Modified),
        b'A' | b'C' => Some(FileStatus::Added),
        b'D' => Some(FileStatus::Deleted),
        b'R' => Some(FileStatus::Renamed),
        b'U' => Some(FileStatus::Conflicted),
        _ => None,
    }
}

fn file(
    path: &[u8],
    renamed_from: Option<PathBuf>,
    index: Option<FileStatus>,
    worktree: Option<FileStatus>,
) -> FileChange {
    FileChange {
        path: path_from(path),
        renamed_from,
        index,
        worktree,
        added: 0,
        removed: 0,
        symbols: Vec::new(),
    }
}

/// What follows the `n`th space, which for a status entry is the path, spaces and all.
fn nth_rest(entry: &[u8], n: usize) -> Option<&[u8]> {
    let mut rest = entry;
    for _ in 0..n {
        let space = rest.iter().position(|&byte| byte == b' ')?;
        rest = &rest[space + 1..];
    }
    Some(rest)
}

/// Followed by the commit to compare the worktree with.
pub(crate) const LINE_COUNTS: &[&str] = &["diff", "-M", "--numstat", "-z"];

/// Reads `LINE_COUNTS`: lines added and removed, by each file's new path.
pub(crate) fn line_counts(output: &[u8]) -> HashMap<PathBuf, (usize, usize)> {
    let mut counts = HashMap::new();
    let mut fields = output.split(|&byte| byte == 0);
    while let Some(field) = fields.next() {
        if let Some((count, path)) = numstat(field, &mut fields) {
            counts.insert(path, count);
        }
    }
    counts
}

/// One `--numstat -z` entry that starts with `field`: `added\tremoved\tpath`, or for a rename,
/// `added\tremoved\t` with the old and new paths as the next two fields. A binary file counts
/// no lines.
fn numstat<'a>(
    field: &[u8],
    fields: &mut impl Iterator<Item = &'a [u8]>,
) -> Option<((usize, usize), PathBuf)> {
    let mut parts = field.splitn(3, |&byte| byte == b'\t');
    let added = parts.next()?;
    let removed = parts.next()?;
    let path = parts.next()?;
    let count = |digits: &[u8]| {
        std::str::from_utf8(digits)
            .ok()
            .and_then(|digits| digits.parse().ok())
            .unwrap_or(0)
    };
    let path = if path.is_empty() {
        fields.next()?;
        fields.next()?
    } else {
        path
    };
    Some(((count(added), count(removed)), path_from(path)))
}

/// Separates commits in `log`'s output, ahead of each one's fields.
const RECORD: u8 = 0x1e;
/// Separates a commit's fields.
const FIELD: u8 = 0x1f;

/// Followed by the commits to list. Each commit's files compare it with its first parent.
pub(crate) const LOG: &[&str] = &[
    "log",
    "-z",
    "--first-parent",
    "-m",
    "--root",
    "-M",
    "--raw",
    "--numstat",
    "--format=%x1e%H%x1f%s%x1f%b",
];

pub(crate) struct LoggedCommit {
    pub sha: String,
    pub title: String,
    pub description: Option<String>,
    pub files: Vec<FileChange>,
}

/// Reads `LOG`: each commit's fields, its files from `--raw`, then their counts from
/// `--numstat`.
pub(crate) fn log(output: &[u8]) -> Vec<LoggedCommit> {
    output
        .split(|&byte| byte == RECORD)
        .filter_map(|record| {
            let header_end = record
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(record.len());
            let mut header = record[..header_end].split(|&byte| byte == FIELD);
            let mut text = || {
                String::from_utf8_lossy(header.next()?.trim_ascii())
                    .into_owned()
                    .into()
            };
            let sha: Option<String> = text();
            let title = text()?;
            let description: Option<String> = text().filter(|body: &String| !body.is_empty());
            Some(LoggedCommit {
                sha: sha?,
                title,
                description,
                files: logged_files(record.get(header_end + 1..).unwrap_or_default()),
            })
        })
        .collect()
}

/// The `--raw` entries of one commit, each `:modes oids STATUS` then its path, or for a rename
/// or copy its old and new paths, followed by the `--numstat` entries for the same files.
fn logged_files(entries: &[u8]) -> Vec<FileChange> {
    let mut files: Vec<FileChange> = Vec::new();
    let mut by_path = HashMap::new();
    let mut fields = entries.split(|&byte| byte == 0);
    while let Some(field) = fields.next() {
        let field = field.trim_ascii_start();
        if let Some(raw) = field.strip_prefix(b":") {
            let letters = raw.rsplit(|&byte| byte == b' ').next().unwrap_or_default();
            let Some(&letter) = letters.first() else {
                continue;
            };
            let from = matches!(letter, b'R' | b'C')
                .then(|| fields.next().map(path_from))
                .flatten();
            let Some(path) = fields.next() else {
                break;
            };
            let from = from.filter(|_| letter == b'R');
            by_path.insert(path_from(path), files.len());
            files.push(file(path, from, status(letter), None));
        } else if let Some(((added, removed), path)) = numstat(field, &mut fields)
            && let Some(&index) = by_path.get(&path)
        {
            files[index].added = added;
            files[index].removed = removed;
        }
    }
    files
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
    use std::path::Path;

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
    fn uncommitted_reads_every_kind_of_entry() {
        let output = b"1 MM N... 100644 100644 100644 aaaa bbbb src/a file.rs\0\
            2 R. N... 100644 100644 100644 aaaa bbbb R100 new.rs\0old.rs\0\
            u UU N... 100644 100644 100644 100644 aaaa bbbb cccc both.rs\0\
            ? notes.md\0";
        let files = uncommitted(output);
        let summary: Vec<_> = files
            .iter()
            .map(|file| (file.path.clone(), file.index, file.worktree))
            .collect();
        assert_eq!(
            summary,
            [
                (
                    PathBuf::from("src/a file.rs"),
                    Some(FileStatus::Modified),
                    Some(FileStatus::Modified)
                ),
                (PathBuf::from("new.rs"), Some(FileStatus::Renamed), None),
                (PathBuf::from("both.rs"), None, Some(FileStatus::Conflicted)),
                (PathBuf::from("notes.md"), None, Some(FileStatus::Untracked)),
            ]
        );
        assert_eq!(files[1].renamed_from, Some(PathBuf::from("old.rs")));
    }

    #[test]
    fn uncommitted_lists_a_file_removed_from_the_index_once() {
        let output = b"1 D. N... 100644 000000 100644 aaaa 0000 f.rs ? f.rs ";
        let files = uncommitted(output);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].index, Some(FileStatus::Deleted));
        assert_eq!(files[0].worktree, Some(FileStatus::Untracked));
    }

    #[test]
    fn line_counts_reads_renames_and_binary_files() {
        let output = b"3\t1\tsrc/a.rs\0\
            0\t0\t\0old.rs\0new.rs\0\
            -\t-\timage.png\0";
        let counts = line_counts(output);
        assert_eq!(counts[Path::new("src/a.rs")], (3, 1));
        assert_eq!(counts[Path::new("new.rs")], (0, 0));
        assert_eq!(counts[Path::new("image.png")], (0, 0));
    }

    #[test]
    fn log_reads_each_commit_with_its_files() {
        let output = b"\x1eaaaa\x1fsecond\x1fbody line\n\0\n\
            :100644 100644 587be6b 587be6b R100\0g\0h\0\
            :100644 100644 1111111 2222222 M\0f\0\
            0\t0\t\0g\0h\0\
            2\t1\tf\0\
            \x1ebbbb\x1ffirst\x1f\0\n\
            :000000 100644 0000000 de98044 A\0f\0\
            3\t0\tf\0";
        let commits = log(output);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].sha, "aaaa");
        assert_eq!(commits[0].description.as_deref(), Some("body line"));
        assert_eq!(commits[1].description, None);
        let second = &commits[0].files;
        assert_eq!(second[0].path, PathBuf::from("h"));
        assert_eq!(second[0].renamed_from, Some(PathBuf::from("g")));
        assert_eq!(second[0].index, Some(FileStatus::Renamed));
        assert_eq!((second[1].added, second[1].removed), (2, 1));
        assert_eq!(commits[1].files[0].index, Some(FileStatus::Added));
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

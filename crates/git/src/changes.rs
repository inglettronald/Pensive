//! A worktree's uncommitted files and its branch's commits, and the two versions of one file.

use std::{collections::HashSet, ffi::OsString, fs, io, path::Path};

use anyhow::{Context as _, Result};
use model::{Base, Changes, Commit, FileChange, FileKey, FileVersions, Source, Upstream, Version};

use crate::{command::git, parse};

/// Commits listed past the base, or on the base branch itself, so a long-lived branch still
/// reads quickly.
const MAX_COMMITS: usize = 100;
/// An untracked file larger than this counts no lines, rather than being read to count them.
const MAX_COUNTED_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) fn changes(worktree: &Path) -> Result<Changes> {
    let head = text(git(worktree, &["rev-parse", "-q", "--verify", "HEAD"]).ok());
    let branch = text(git(worktree, &["symbolic-ref", "-q", "--short", "HEAD"]).ok());
    let uncommitted = uncommitted(worktree, head.is_some())?;
    let Some(head) = head else {
        // Before the first commit there's nothing to compare with.
        return Ok(Changes {
            uncommitted,
            ..Changes::default()
        });
    };
    let tracking = match &branch {
        Some(branch) => tracking(worktree, branch)?,
        None => None,
    };
    let base = base(worktree, branch.as_deref(), tracking.as_ref())?;
    let unpushed: Option<HashSet<String>> = match &tracking {
        Some(tracking) => {
            let exclude = format!("^{}", tracking.reference);
            let listed = git(worktree, &["rev-list", &head, &exclude, "--"])?;
            Some(lines(&listed).collect())
        }
        None => None,
    };
    let limit = format!("--max-count={MAX_COMMITS}");
    let range = match &base {
        Some((_, reference)) => format!("{reference}..{head}"),
        None => head,
    };
    let args = [parse::LOG, &[&limit, &range, "--"]].concat();
    let commits = parse::log(&git(worktree, &args)?)
        .into_iter()
        .map(|logged| Commit {
            pushed: unpushed
                .as_ref()
                .is_some_and(|unpushed| !unpushed.contains(&logged.sha)),
            sha: logged.short_sha.into(),
            title: logged.title.into(),
            description: logged.description.map(Into::into),
            files: logged.files,
        })
        .collect();
    Ok(Changes {
        base: base.map(|(branch, _)| Base {
            branch: branch.into(),
        }),
        upstream: tracking.map(|tracking| Upstream {
            remote: tracking.remote.into(),
            behind: tracking.behind,
        }),
        uncommitted,
        commits,
    })
}

/// Each changed file with its line counts against HEAD, staged and unstaged together.
fn uncommitted(worktree: &Path, has_head: bool) -> Result<Vec<FileChange>> {
    let mut files = parse::uncommitted(&git(worktree, parse::UNCOMMITTED)?);
    // Before the first commit only staged files can be compared, with nothing.
    let against = if has_head { "HEAD" } else { "--cached" };
    let counts = parse::line_counts(&git(
        worktree,
        &[parse::LINE_COUNTS, &[against, "--"]].concat(),
    )?);
    for file in &mut files {
        (file.added, file.removed) = match counts.get(&file.path) {
            Some(&count) => count,
            None => (untracked_lines(&worktree.join(&file.path)), 0),
        };
    }
    Ok(files)
}

/// The lines in a file Git doesn't track yet, all of them added.
fn untracked_lines(path: &Path) -> usize {
    let small = fs::metadata(path).is_ok_and(|metadata| metadata.len() <= MAX_COUNTED_BYTES);
    match small
        .then(|| fs::read(path).ok())
        .flatten()
        .map(Version::from_bytes)
    {
        Some(Version::Text(text)) => text.lines().count(),
        _ => 0,
    }
}

/// The remote branch `branch` tracks.
struct Tracking {
    remote: String,
    /// Its full ref, such as `refs/remotes/origin/main`.
    reference: String,
    behind: usize,
}

fn tracking(worktree: &Path, branch: &str) -> Result<Option<Tracking>> {
    let local = format!("refs/heads/{branch}");
    let listed = git(
        worktree,
        &[
            "for-each-ref",
            "--format=%(upstream:remotename)%00%(upstream)",
            &local,
        ],
    )?;
    let Some((remote, reference)) = lines(&listed)
        .next()
        .and_then(|line| {
            line.split_once('\0')
                .map(|(a, b)| (a.to_owned(), b.to_owned()))
        })
        .filter(|(_, reference)| !reference.is_empty())
    else {
        return Ok(None);
    };
    // The upstream is configured but hasn't been fetched yet.
    let Some(reference) = resolve(worktree, &reference) else {
        return Ok(None);
    };
    let range = format!("HEAD..{reference}");
    let behind = text(Some(git(worktree, &["rev-list", "--count", &range, "--"])?))
        .and_then(|count| count.parse().ok())
        .unwrap_or(0);
    Ok(Some(Tracking {
        remote,
        reference,
        behind,
    }))
}

/// The branch this one started from, as its name and the ref to compare with, or `None` on that
/// branch itself. It's the remote's default branch, or failing that `main` or `master`, and the
/// local branch is preferred over the remote one, since that's what a merge would land on.
fn base(
    worktree: &Path,
    branch: Option<&str>,
    tracking: Option<&Tracking>,
) -> Result<Option<(String, String)>> {
    let remote = tracking.map_or("origin", |tracking| tracking.remote.as_str());
    let remote_head = format!("refs/remotes/{remote}/HEAD");
    let default = text(git(worktree, &["symbolic-ref", "-q", &remote_head]).ok())
        .and_then(|target| {
            let name = target
                .strip_prefix(&format!("refs/remotes/{remote}/"))?
                .to_owned();
            Some((name, target))
        })
        .or_else(|| {
            ["main", "master"].into_iter().find_map(|name| {
                let local = resolve(worktree, &format!("refs/heads/{name}"))?;
                Some((name.to_owned(), local))
            })
        });
    let Some((name, remote_ref)) = default else {
        return Ok(None);
    };
    if branch == Some(name.as_str()) {
        return Ok(None);
    }
    let reference = resolve(worktree, &format!("refs/heads/{name}")).unwrap_or(remote_ref);
    Ok(Some((name, reference)))
}

/// `reference` if it exists.
fn resolve(worktree: &Path, reference: &str) -> Option<String> {
    git(worktree, &["rev-parse", "-q", "--verify", reference])
        .is_ok()
        .then(|| reference.to_owned())
}

pub(crate) fn versions(
    worktree: &Path,
    key: &FileKey,
    renamed_from: Option<&Path>,
) -> Result<FileVersions> {
    let old_path = renamed_from.unwrap_or(&key.path);
    Ok(match &key.source {
        Source::Uncommitted => FileVersions {
            old: blob(worktree, "HEAD", old_path)?,
            new: match fs::read(worktree.join(&key.path)) {
                Ok(bytes) => Version::from_bytes(bytes),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Version::Missing,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("couldn't read {}", key.path.display()));
                }
            },
        },
        Source::Commit(sha) => FileVersions {
            old: blob(worktree, &format!("{sha}^"), old_path)?,
            new: blob(worktree, sha, &key.path)?,
        },
    })
}

/// The file at `path` in commit `revision`, as checkout would write it, so its line endings
/// match the worktree's.
fn blob(worktree: &Path, revision: &str, path: &Path) -> Result<Version> {
    let mut object = OsString::from(format!("{revision}:"));
    object.push(path);
    let found = git(
        worktree,
        &[
            OsString::from("rev-parse"),
            "-q".into(),
            "--verify".into(),
            object,
        ],
    );
    let Some(id) = text(found.ok()) else {
        return Ok(Version::Missing);
    };
    let mut path_arg = OsString::from("--path=");
    path_arg.push(path);
    let content = git(
        worktree,
        &[
            OsString::from("cat-file"),
            "--filters".into(),
            path_arg,
            id.into(),
        ],
    )?;
    Ok(Version::from_bytes(content))
}

/// The lines of a command's output.
fn lines(output: &[u8]) -> impl Iterator<Item = String> + '_ {
    output
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| String::from_utf8_lossy(line).into_owned())
}

/// The first line of a command's output, if it printed one.
fn text(output: Option<Vec<u8>>) -> Option<String> {
    lines(&output?).next()
}

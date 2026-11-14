//! A worktree's uncommitted files and its branch's commits, and the two versions of one file.

use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    io::{self, Write as _},
    path::Path,
};

use anyhow::{Context as _, Result};
use model::{
    Base, Changes, Commit, FileChange, FileKey, FileStatus, FileVersions, Source, Upstream, Version,
};

use crate::{GitCli, parse};

/// Commits listed past the base, or on the base branch itself, so a long-lived branch still
/// reads quickly.
const MAX_COMMITS: usize = 100;
/// An untracked file larger than this counts no lines, rather than being read to count them.
const MAX_COUNTED_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) fn changes(cli: &GitCli, worktree: &Path) -> Result<Changes> {
    let head = text(
        cli.git(worktree, &["rev-parse", "-q", "--verify", "HEAD"])
            .ok(),
    );
    let branch = text(
        cli.git(worktree, &["symbolic-ref", "-q", "--short", "HEAD"])
            .ok(),
    );
    let uncommitted = uncommitted(cli, worktree, head.is_some())?;
    let Some(head) = head else {
        // Before the first commit there's nothing to compare with.
        return Ok(Changes {
            uncommitted,
            ..Changes::default()
        });
    };
    let tracking = match &branch {
        Some(branch) => tracking(cli, worktree, branch)?,
        None => None,
    };
    let base = base(cli, worktree, branch.as_deref(), tracking.as_ref())?;
    let unpushed: Option<HashSet<String>> = match &tracking {
        Some(tracking) => {
            let exclude = format!("^{}", tracking.reference);
            let listed = cli.git(worktree, &["rev-list", &head, &exclude, "--"])?;
            Some(lines(&listed).collect())
        }
        None => None,
    };
    let limit = format!("--max-count={MAX_COMMITS}");
    let mut range = vec![limit, head];
    if let Some((_, references)) = &base {
        range.extend(references.iter().map(|reference| format!("^{reference}")));
    }
    range.push("--".to_owned());
    let args = [
        parse::LOG,
        &range.iter().map(String::as_str).collect::<Vec<_>>(),
    ]
    .concat();
    let commits = parse::log(&cli.git(worktree, &args)?)
        .into_iter()
        .map(|logged| Commit {
            pushed: unpushed
                .as_ref()
                .is_some_and(|unpushed| !unpushed.contains(&logged.sha)),
            sha: logged.sha.into(),
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
fn uncommitted(cli: &GitCli, worktree: &Path, has_head: bool) -> Result<Vec<FileChange>> {
    let mut files = parse::uncommitted(&cli.git(worktree, parse::UNCOMMITTED)?);
    // Before the first commit only staged files can be compared, with nothing.
    let against = if has_head { "HEAD" } else { "--cached" };
    let counts =
        parse::line_counts(&cli.git(worktree, &[parse::LINE_COUNTS, &[against, "--"]].concat())?);
    for file in &mut files {
        (file.added, file.removed) = match counts.get(&file.path) {
            Some(&count)
                if file.index.is_some() || file.worktree != Some(FileStatus::Untracked) =>
            {
                count
            }
            _ => (untracked_lines(&worktree.join(&file.path)), 0),
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

fn tracking(cli: &GitCli, worktree: &Path, branch: &str) -> Result<Option<Tracking>> {
    let local = format!("refs/heads/{branch}");
    let listed = cli.git(
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
    let Some(reference) = resolve(cli, worktree, &reference) else {
        return Ok(None);
    };
    let range = format!("HEAD..{reference}");
    let behind = text(Some(
        cli.git(worktree, &["rev-list", "--count", &range, "--"])?,
    ))
    .and_then(|count| count.parse().ok())
    .unwrap_or(0);
    Ok(Some(Tracking {
        remote,
        reference,
        behind,
    }))
}

/// The branch this one started from, as its name and the refs whose commits aren't this branch's,
/// or `None` on that branch itself. It's the remote's default branch, or failing that `main` or
/// `master`. Both its local and remote refs count, since either may be behind the other.
fn base(
    cli: &GitCli,
    worktree: &Path,
    branch: Option<&str>,
    tracking: Option<&Tracking>,
) -> Result<Option<(String, Vec<String>)>> {
    let remote = tracking.map_or("origin", |tracking| tracking.remote.as_str());
    let remote_head = format!("refs/remotes/{remote}/HEAD");
    let default = text(
        cli.git(worktree, &["symbolic-ref", "-q", &remote_head])
            .ok(),
    )
    .and_then(|target| {
        let name = target
            .strip_prefix(&format!("refs/remotes/{remote}/"))?
            .to_owned();
        Some((name, target))
    })
    .or_else(|| {
        ["main", "master"].into_iter().find_map(|name| {
            let local = resolve(cli, worktree, &format!("refs/heads/{name}"))?;
            Some((name.to_owned(), local))
        })
    });
    let Some((name, remote_ref)) = default else {
        return Ok(None);
    };
    if branch == Some(name.as_str()) {
        return Ok(None);
    }
    let mut references = vec![remote_ref];
    references.extend(resolve(cli, worktree, &format!("refs/heads/{name}")));
    references.dedup();
    Ok(Some((name, references)))
}

/// `reference` if it exists.
fn resolve(cli: &GitCli, worktree: &Path, reference: &str) -> Option<String> {
    cli.git(worktree, &["rev-parse", "-q", "--verify", reference])
        .is_ok()
        .then(|| reference.to_owned())
}

pub(crate) fn versions(
    cli: &GitCli,
    worktree: &Path,
    key: &FileKey,
    renamed_from: Option<&Path>,
) -> Result<FileVersions> {
    let old_path = renamed_from.unwrap_or(&key.path);
    Ok(match &key.source {
        Source::Uncommitted => FileVersions {
            old: blob(cli, worktree, "HEAD", old_path)?,
            new: read_file(worktree, &key.path)?,
        },
        Source::Commit(sha) => FileVersions {
            old: blob(cli, worktree, &format!("{sha}^"), old_path)?,
            new: blob(cli, worktree, sha, &key.path)?,
        },
    })
}

/// The file at `path` in the worktree. A submodule or nested repository is a folder, with no text
/// of its own, so it's missing.
pub(crate) fn read_file(worktree: &Path, path: &Path) -> Result<Version> {
    let full = worktree.join(path);
    if full.is_dir() {
        return Ok(Version::Missing);
    }
    match fs::read(full) {
        Ok(bytes) => Ok(Version::from_bytes(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Version::Missing),
        Err(error) => Err(error).with_context(|| format!("couldn't read {}", path.display())),
    }
}

/// Replaces the file in one step, so a crash never leaves half of it. A symlink's target is
/// replaced, so the link survives, and the file keeps its permissions. A file that's gone is
/// written as any new file is, with the default permissions.
pub(crate) fn write_file(worktree: &Path, path: &Path, text: &str) -> Result<()> {
    let link = worktree.join(path);
    let target = fs::canonicalize(&link).unwrap_or(link);
    let Ok(metadata) = fs::metadata(&target) else {
        fs::write(&target, text)?;
        return Ok(());
    };
    let folder = target
        .parent()
        .with_context(|| format!("{} has no folder", target.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(folder)?;
    temporary.write_all(text.as_bytes())?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())?;
    temporary.as_file().sync_all()?;
    temporary.persist(&target)?;
    Ok(())
}

/// The file at `path` in commit `revision`, as checkout would write it, so its line endings
/// match the worktree's. A submodule there is a commit, not a file, so it's missing, and so is
/// every file in a revision that doesn't exist, such as HEAD before the first commit or the parent
/// of a root commit.
fn blob(cli: &GitCli, worktree: &Path, revision: &str, path: &Path) -> Result<Version> {
    let listed = match cli.git(
        worktree,
        &[
            OsString::from("ls-tree"),
            "-z".into(),
            "--full-tree".into(),
            "--format=%(objecttype) %(objectname)".into(),
            revision.into(),
            "--".into(),
            path.into(),
        ],
    ) {
        Ok(listed) => listed,
        Err(_) if resolve(cli, worktree, revision).is_none() => return Ok(Version::Missing),
        Err(error) => return Err(error),
    };
    // Nothing listed means the revision has no file at `path`.
    let entry = listed.split(|&byte| byte == 0).next().unwrap_or_default();
    let Some(id) = entry.strip_prefix(b"blob ") else {
        return Ok(Version::Missing);
    };
    let id = OsString::from(String::from_utf8_lossy(id).into_owned());
    let mut path_arg = OsString::from("--path=");
    path_arg.push(path);
    let content = cli.git(
        worktree,
        &[OsString::from("cat-file"), "--filters".into(), path_arg, id],
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

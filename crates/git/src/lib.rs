//! A `GitBackend` that runs the `git` command line.

mod changes;
mod command;
mod parse;

use std::{collections::HashMap, fs, path::Path, time::SystemTime};

use anyhow::{Context as _, Result};
use model::{Branch, Changes, FileKey, FileVersions, GitBackend, Repository, Worktree};

use crate::{command::git, parse::ListedWorktree};

#[derive(Debug)]
pub struct GitCli;

impl GitBackend for GitCli {
    fn repository(&self, path: &Path) -> Result<Repository> {
        let listed = parse::worktrees(&git(path, parse::WORKTREES)?);
        let root = listed
            .first()
            .map(|worktree| worktree.path.clone())
            .context("Git listed no worktrees")?;
        let listed = listed
            .into_iter()
            // Git keeps listing a locked worktree after its folder is deleted.
            .filter(|worktree| worktree.checked_out && worktree.path.is_dir())
            .collect();
        Ok(Repository {
            worktrees: worktrees(&root, listed)?,
            branches: branches(&root)?,
            root,
        })
    }

    fn changes(&self, worktree: &Path) -> Result<Changes> {
        changes::changes(worktree)
    }

    fn versions(
        &self,
        worktree: &Path,
        key: &FileKey,
        renamed_from: Option<&Path>,
    ) -> Result<FileVersions> {
        changes::versions(worktree, key, renamed_from)
    }
}

fn worktrees(root: &Path, listed: Vec<ListedWorktree>) -> Result<Vec<Worktree>> {
    let heads: Vec<&str> = listed
        .iter()
        .filter_map(|worktree| worktree.head.as_deref())
        .collect();
    let commit_times: HashMap<String, SystemTime> = if heads.is_empty() {
        HashMap::new()
    } else {
        let args = [parse::COMMIT_TIMES, &heads].concat();
        parse::named_times(&git(root, &args)?).collect()
    };
    Ok(listed
        .into_iter()
        .map(|worktree| {
            let committed = worktree
                .head
                .and_then(|head| commit_times.get(&head).copied());
            // A worktree whose status Git can't read still shows, dated by its commit.
            let changed = newest_change(&worktree.path).unwrap_or_default();
            Worktree {
                path: worktree.path,
                branch: worktree.branch.map(Into::into),
                last_change: committed.max(changed).unwrap_or(SystemTime::UNIX_EPOCH),
            }
        })
        .collect())
}

fn branches(root: &Path) -> Result<Vec<Branch>> {
    Ok(parse::named_times(&git(root, parse::BRANCHES)?)
        .map(|(name, last_commit)| Branch {
            name: name.into(),
            last_commit,
        })
        .collect())
}

/// When the newest uncommitted change in the worktree at `path` was saved, if it has any.
fn newest_change(path: &Path) -> Result<Option<SystemTime>> {
    let status = git(path, parse::STATUS)?;
    Ok(parse::status_paths(&status)
        .iter()
        // A deleted file has no time to report.
        .filter_map(|changed| fs::metadata(path.join(changed)).ok()?.modified().ok())
        .max())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        process::Command,
        time::{Duration, UNIX_EPOCH},
    };

    use model::{FileStatus, Source, Staging, Version};
    use tempfile::TempDir;

    use super::*;

    /// A repository in a temporary folder, built with Git's own commands.
    struct Repo {
        dir: TempDir,
    }

    impl Repo {
        fn new() -> Self {
            let repo = Self {
                dir: tempfile::tempdir().unwrap(),
            };
            fs::create_dir(repo.path()).unwrap();
            fs::write(repo.dir.path().join("gitconfig"), "").unwrap();
            repo.run(&["init", "-q", "--initial-branch=main"]);
            repo
        }

        fn path(&self) -> PathBuf {
            self.dir.path().join("main")
        }

        /// Git in the main worktree, set up as Pensive runs it but ignoring this machine's config.
        fn command(&self) -> Command {
            let mut command = command::command(&self.path());
            command
                .env("GIT_CONFIG_GLOBAL", self.dir.path().join("gitconfig"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Pensive")
                .env("GIT_AUTHOR_EMAIL", "pensive@example.com")
                .env("GIT_COMMITTER_NAME", "Pensive")
                .env("GIT_COMMITTER_EMAIL", "pensive@example.com");
            command
        }

        fn run(&self, args: &[&str]) {
            check(self.command().args(args));
        }

        /// Commits a change to `file`, dated `seconds` after the Unix epoch.
        fn commit(&self, seconds: u64) {
            fs::write(self.path().join("file"), seconds.to_string()).unwrap();
            self.run(&["add", "file"]);
            let date = format!("{seconds} +0000");
            check(
                self.command()
                    .args(["commit", "-q", "-m", &date])
                    .env("GIT_AUTHOR_DATE", &date)
                    .env("GIT_COMMITTER_DATE", &date),
            );
        }

        fn read(&self) -> Repository {
            GitCli.repository(&self.path()).unwrap()
        }

        fn write(&self, name: &str, text: &str) {
            let path = self.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        /// Stages everything and commits it.
        fn commit_all(&self, message: &str) {
            self.run(&["add", "-A"]);
            self.run(&["commit", "-q", "-m", message]);
        }

        fn commit_all_empty(&self, message: &str) {
            self.run(&["commit", "-q", "--allow-empty", "-m", message]);
        }

        fn changes(&self) -> Changes {
            GitCli.changes(&self.path()).unwrap()
        }

        fn versions(&self, key: &FileKey, renamed_from: Option<&Path>) -> FileVersions {
            GitCli.versions(&self.path(), key, renamed_from).unwrap()
        }
    }

    fn uncommitted(path: &str) -> FileKey {
        FileKey {
            source: Source::Uncommitted,
            path: path.into(),
        }
    }

    fn check(command: &mut Command) {
        let status = command.status().unwrap();
        assert!(status.success(), "{command:?}");
    }

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn lists_branches_with_their_last_commit() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        repo.run(&["branch", "old"]);
        repo.commit(1_100_000_000);
        let mut branches: Vec<_> = repo
            .read()
            .branches
            .into_iter()
            .map(|branch| (branch.name.to_string(), branch.last_commit))
            .collect();
        branches.sort();
        assert_eq!(
            branches,
            [
                ("main".to_string(), at(1_100_000_000)),
                ("old".to_string(), at(1_000_000_000))
            ]
        );
    }

    #[test]
    fn a_linked_worktree_reads_as_the_same_repository() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        repo.run(&["worktree", "add", "-q", "-b", "feat", "../linked"]);
        let from_linked = GitCli.repository(&repo.dir.path().join("linked")).unwrap();
        assert_eq!(from_linked.root, repo.read().root);
        let branches: Vec<_> = from_linked
            .worktrees
            .into_iter()
            .map(|worktree| worktree.branch.map(|branch| branch.to_string()))
            .collect();
        assert_eq!(branches, [Some("main".into()), Some("feat".into())]);
    }

    #[test]
    fn a_locked_worktree_whose_folder_is_gone_is_left_out() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        repo.run(&["worktree", "add", "-q", "-b", "locked", "../locked"]);
        repo.run(&["worktree", "lock", "../locked"]);
        fs::remove_dir_all(repo.dir.path().join("locked")).unwrap();
        assert_eq!(repo.read().worktrees.len(), 1);
    }

    /// macOS and Windows don't allow folder names that aren't UTF-8.
    #[cfg(target_os = "linux")]
    #[test]
    fn reads_a_worktree_path_that_isnt_utf8() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

        let repo = Repo::new();
        repo.commit(1_000_000_000);
        let name = OsStr::from_bytes(b"linked-\xff");
        check(
            repo.command()
                .args(["worktree", "add", "-q", "-b", "odd"])
                .arg(repo.dir.path().join(name)),
        );
        let worktrees = repo.read().worktrees;
        assert_eq!(worktrees[1].path.file_name(), Some(name));
    }

    #[test]
    fn a_detached_head_has_no_branch() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        repo.run(&["checkout", "-q", "--detach"]);
        assert_eq!(repo.read().worktrees[0].branch, None);
    }

    #[test]
    fn an_uncommitted_change_is_the_last_change() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        assert_eq!(repo.read().worktrees[0].last_change, at(1_000_000_000));
        fs::write(repo.path().join("file"), "edited").unwrap();
        assert!(repo.read().worktrees[0].last_change > at(1_000_000_000));
    }

    #[test]
    fn a_repository_without_commits_still_reads() {
        let repository = Repo::new().read();
        assert_eq!(repository.worktrees[0].branch.as_deref(), Some("main"));
        assert!(repository.branches.is_empty());
    }

    #[test]
    fn uncommitted_files_have_their_status_and_line_counts() {
        let repo = Repo::new();
        repo.write("kept.rs", "a\nb\nc\n");
        repo.commit_all("first");
        repo.write("kept.rs", "a\nB\nc\nd\n");
        repo.run(&["add", "kept.rs"]);
        repo.write("kept.rs", "a\nB\nc\nd\ne\n");
        repo.write("new folder/notes.md", "one\ntwo\n");
        let changes = repo.changes();
        let kept = changes.file(Path::new("kept.rs")).unwrap();
        assert_eq!(kept.staging(), Staging::Partly);
        assert_eq!((kept.added, kept.removed), (3, 1));
        let notes = changes.file(Path::new("new folder/notes.md")).unwrap();
        assert_eq!(notes.worktree, Some(FileStatus::Untracked));
        assert_eq!((notes.added, notes.removed), (2, 0));
    }

    #[test]
    fn versions_of_an_uncommitted_file_are_head_and_the_worktree() {
        let repo = Repo::new();
        repo.write("kept.rs", "old\n");
        repo.commit_all("first");
        repo.write("kept.rs", "new\n");
        repo.write("added.rs", "added\n");
        let kept = repo.versions(&uncommitted("kept.rs"), None);
        assert_eq!(kept.old, Version::Text("old\n".into()));
        assert_eq!(kept.new, Version::Text("new\n".into()));
        let added = repo.versions(&uncommitted("added.rs"), None);
        assert_eq!(added.old, Version::Missing);
        fs::remove_file(repo.path().join("kept.rs")).unwrap();
        assert_eq!(
            repo.versions(&uncommitted("kept.rs"), None).new,
            Version::Missing
        );
    }

    #[test]
    fn a_branch_lists_its_own_commits_newest_first() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        repo.commit_all("on main");
        repo.run(&["checkout", "-q", "-b", "feat"]);
        repo.write("a.rs", "a\nb\n");
        repo.commit_all("first on feat");
        repo.run(&["mv", "a.rs", "b.rs"]);
        repo.commit_all("rename");
        let changes = repo.changes();
        assert_eq!(changes.base.unwrap().branch, "main");
        let titles: Vec<_> = changes
            .commits
            .iter()
            .map(|commit| commit.title.to_string())
            .collect();
        assert_eq!(titles, ["rename", "first on feat"]);
        let renamed = &changes.commits[0].files[0];
        assert_eq!(renamed.index, Some(FileStatus::Renamed));
        assert_eq!(renamed.renamed_from.as_deref(), Some(Path::new("a.rs")));
        assert_eq!(
            (
                changes.commits[1].files[0].added,
                changes.commits[1].files[0].removed
            ),
            (1, 0)
        );

        let key = FileKey {
            source: Source::Commit(changes.commits[1].sha.clone()),
            path: "a.rs".into(),
        };
        let versions = repo.versions(&key, None);
        assert_eq!(versions.old, Version::Text("a\n".into()));
        assert_eq!(versions.new, Version::Text("a\nb\n".into()));
    }

    #[test]
    fn commits_the_upstream_has_are_pushed() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        repo.commit_all("on main");
        repo.run(&["checkout", "-q", "-b", "feat"]);
        repo.commit_all_empty("pushed");
        repo.run(&[
            "config",
            "remote.origin.url",
            "https://example.com/repo.git",
        ]);
        repo.run(&[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ]);
        repo.run(&["config", "branch.feat.remote", "origin"]);
        repo.run(&["config", "branch.feat.merge", "refs/heads/feat"]);
        repo.run(&["update-ref", "refs/remotes/origin/feat", "HEAD"]);
        repo.commit_all_empty("not pushed");
        let changes = repo.changes();
        let pushed: Vec<_> = changes.commits.iter().map(|commit| commit.pushed).collect();
        assert_eq!(pushed, [false, true]);
        let upstream = changes.upstream.unwrap();
        assert_eq!((upstream.remote.as_ref(), upstream.behind), ("origin", 0));
    }

    #[test]
    fn the_base_branch_itself_has_no_base() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        repo.commit_all("on main");
        let changes = repo.changes();
        assert!(changes.base.is_none());
        assert_eq!(changes.commits.len(), 1);
    }

    #[test]
    fn a_repository_without_commits_lists_its_files() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        let changes = repo.changes();
        assert_eq!(changes.uncommitted.len(), 1);
        assert!(changes.commits.is_empty());
    }

    #[test]
    fn a_folder_outside_any_repository_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(GitCli.repository(dir.path()).is_err());
    }
}

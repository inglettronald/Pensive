//! A `GitBackend` that runs the `git` command line.

mod changes;
mod command;
mod find;
mod parse;
mod staging;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context as _, Result};
use model::{Branch, Changes, FileKey, FileVersions, GitBackend, Repository, Version, Worktree};

pub use crate::find::Git;
use crate::{find::Finder, parse::ListedWorktree};

/// Runs the `git` command line, with the Git it finds on first use.
#[derive(Debug)]
pub struct GitCli {
    finder: Finder,
}

impl GitBackend for GitCli {
    fn repository(&self, path: &Path) -> Result<Repository> {
        let listed = parse::worktrees(&self.git(path, parse::WORKTREES)?);
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
            worktrees: worktrees(self, &root, listed)?,
            branches: branches(self, &root)?,
            root,
        })
    }

    fn changes(&self, worktree: &Path) -> Result<Changes> {
        changes::changes(self, worktree)
    }

    fn versions(
        &self,
        worktree: &Path,
        key: &FileKey,
        renamed_from: Option<&Path>,
    ) -> Result<FileVersions> {
        changes::versions(self, worktree, key, renamed_from)
    }

    fn read_file(&self, worktree: &Path, path: &Path) -> Result<Version> {
        changes::read_file(worktree, path)
    }

    fn write_file(&self, worktree: &Path, path: &Path, text: &str) -> Result<()> {
        changes::write_file(worktree, path, text)
    }

    fn set_staged(&self, worktree: &Path, paths: &[PathBuf], staged: bool) -> Result<()> {
        staging::set_staged(self, worktree, paths, staged)
    }
}

fn worktrees(cli: &GitCli, root: &Path, listed: Vec<ListedWorktree>) -> Result<Vec<Worktree>> {
    let heads: Vec<&str> = listed
        .iter()
        .filter_map(|worktree| worktree.head.as_deref())
        .collect();
    let commit_times: HashMap<String, SystemTime> = if heads.is_empty() {
        HashMap::new()
    } else {
        let args = [parse::COMMIT_TIMES, &heads].concat();
        parse::named_times(&cli.git(root, &args)?).collect()
    };
    Ok(listed
        .into_iter()
        .map(|worktree| {
            let committed = worktree
                .head
                .and_then(|head| commit_times.get(&head).copied());
            // A worktree whose status Git can't read still shows, dated by its commit.
            let changed = newest_change(cli, &worktree.path).unwrap_or_default();
            Worktree {
                path: worktree.path,
                branch: worktree.branch.map(Into::into),
                last_change: committed.max(changed).unwrap_or(SystemTime::UNIX_EPOCH),
            }
        })
        .collect())
}

fn branches(cli: &GitCli, root: &Path) -> Result<Vec<Branch>> {
    Ok(parse::named_times(&cli.git(root, parse::BRANCHES)?)
        .map(|(name, last_commit)| Branch {
            name: name.into(),
            last_commit,
        })
        .collect())
}

/// When the newest uncommitted change in the worktree at `path` was saved, if it has any.
fn newest_change(cli: &GitCli, path: &Path) -> Result<Option<SystemTime>> {
    let status = cli.git(path, parse::STATUS)?;
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
            // The backend reads this machine's config, and Git for Windows turns on CRLF
            // checkouts system-wide. The repository's own setting wins.
            repo.run(&["config", "core.autocrlf", "false"]);
            repo
        }

        fn path(&self) -> PathBuf {
            self.dir.path().join("main")
        }

        /// Git in the main worktree, set up as Pensive runs it but ignoring this machine's config.
        fn command(&self) -> Command {
            let mut command = GitCli::from_env().command(&self.path()).unwrap();
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
            GitCli::from_env().repository(&self.path()).unwrap()
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
            GitCli::from_env().changes(&self.path()).unwrap()
        }

        fn versions(&self, key: &FileKey, renamed_from: Option<&Path>) -> FileVersions {
            GitCli::from_env()
                .versions(&self.path(), key, renamed_from)
                .unwrap()
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
        let from_linked = GitCli::from_env()
            .repository(&repo.dir.path().join("linked"))
            .unwrap();
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
    fn a_local_base_behind_its_remote_lends_no_commits() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        repo.commit_all("on main");
        repo.run(&["checkout", "-q", "-b", "feat"]);
        repo.commit_all_empty("fetched to origin/main");
        repo.run(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
        repo.run(&[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ]);
        repo.commit_all_empty("on feat");
        let titles: Vec<_> = repo
            .changes()
            .commits
            .iter()
            .map(|commit| commit.title.to_string())
            .collect();
        assert_eq!(titles, ["on feat"]);
    }

    #[test]
    fn a_file_removed_from_the_index_is_listed_once_as_untracked() {
        let repo = Repo::new();
        repo.write("a.rs", "a\nb\n");
        repo.commit_all("on main");
        repo.run(&["rm", "-q", "--cached", "a.rs"]);
        let changes = repo.changes();
        assert_eq!(changes.uncommitted.len(), 1);
        assert_eq!(changes.uncommitted[0].worktree, Some(FileStatus::Untracked));
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
    fn a_file_has_no_old_version_before_the_first_commit_or_in_a_root_commit() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        let before = repo.versions(&uncommitted("a.rs"), None);
        assert!(matches!(before.old, Version::Missing));
        repo.commit_all("first");
        let sha = repo.changes().commits[0].sha.to_string();
        let key = FileKey {
            source: Source::Commit(sha.into()),
            path: "a.rs".into(),
        };
        let root = repo.versions(&key, None);
        assert!(matches!(root.old, Version::Missing));
        assert!(matches!(root.new, Version::Text(_)));
    }

    #[test]
    fn a_revision_git_cant_list_fails_instead_of_reading_as_missing() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        repo.commit_all("first");
        // A blob's id names an object that exists, but ls-tree can't list it.
        let blob = String::from_utf8(
            GitCli::from_env()
                .git(&repo.path(), &["rev-parse", "HEAD:a.rs"])
                .unwrap(),
        );
        let key = FileKey {
            source: Source::Commit(blob.unwrap().trim().to_owned().into()),
            path: "a.rs".into(),
        };
        assert!(
            GitCli::from_env()
                .versions(&repo.path(), &key, None)
                .is_err()
        );
    }

    #[test]
    fn writing_a_file_replaces_its_text_and_reads_back() {
        let repo = Repo::new();
        repo.write("a.rs", "old\n");
        GitCli::from_env()
            .write_file(&repo.path(), Path::new("a.rs"), "new\n")
            .unwrap();
        assert_eq!(
            GitCli::from_env()
                .read_file(&repo.path(), Path::new("a.rs"))
                .unwrap(),
            Version::Text("new\n".into())
        );
        assert_eq!(
            GitCli::from_env()
                .read_file(&repo.path(), Path::new("gone.rs"))
                .unwrap(),
            Version::Missing
        );
    }

    #[cfg(unix)]
    #[test]
    fn writing_keeps_permissions_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let repo = Repo::new();
        repo.write("run.sh", "echo old\n");
        let script = repo.path().join("run.sh");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        symlink("run.sh", repo.path().join("link.sh")).unwrap();
        GitCli::from_env()
            .write_file(&repo.path(), Path::new("link.sh"), "echo new\n")
            .unwrap();
        assert!(
            fs::symlink_metadata(repo.path().join("link.sh"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&script).unwrap(), "echo new\n");
        let mode = fs::metadata(&script).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[test]
    fn staging_moves_whole_files_into_the_index_and_back() {
        let repo = Repo::new();
        repo.write("kept.rs", "a\n");
        repo.write("gone.rs", "one\ntwo\nthree\n");
        repo.commit_all("first");
        repo.write("kept.rs", "a\nb\n");
        fs::remove_file(repo.path().join("gone.rs")).unwrap();
        repo.write("new [a].rs", "other\n");
        let paths: Vec<PathBuf> = ["kept.rs", "gone.rs", "new [a].rs"]
            .map(PathBuf::from)
            .into();
        let staging = |repo: &Repo| -> Vec<Staging> {
            let changes = repo.changes();
            paths
                .iter()
                .map(|path| changes.file(path).unwrap().staging())
                .collect()
        };

        GitCli::from_env()
            .set_staged(&repo.path(), &paths, true)
            .unwrap();
        assert_eq!(staging(&repo), [Staging::Staged; 3]);

        GitCli::from_env()
            .set_staged(&repo.path(), &paths[..1], false)
            .unwrap();
        assert_eq!(
            staging(&repo),
            [Staging::Unstaged, Staging::Staged, Staging::Staged]
        );

        GitCli::from_env()
            .set_staged(&repo.path(), &paths, false)
            .unwrap();
        assert_eq!(staging(&repo), [Staging::Unstaged; 3]);
        let new = repo.changes();
        let new = new.file(Path::new("new [a].rs")).unwrap();
        assert_eq!(new.worktree, Some(FileStatus::Untracked));
    }

    #[test]
    fn a_rename_stages_by_its_new_path_and_unstages_with_the_old_one() {
        let repo = Repo::new();
        repo.write("old.rs", "a\nb\nc\n");
        repo.commit_all("first");
        repo.run(&["mv", "old.rs", "new.rs"]);
        repo.write("new.rs", "a\nb\nc\nd\n");
        let renamed = repo.changes();
        let renamed = renamed.file(Path::new("new.rs")).unwrap();
        assert_eq!(renamed.renamed_from.as_deref(), Some(Path::new("old.rs")));
        assert_eq!(renamed.staging(), Staging::Partly);

        GitCli::from_env()
            .set_staged(&repo.path(), &[PathBuf::from("new.rs")], true)
            .unwrap();
        assert_eq!(repo.changes().uncommitted[0].staging(), Staging::Staged);

        let paths = ["new.rs", "old.rs"].map(PathBuf::from);
        GitCli::from_env()
            .set_staged(&repo.path(), &paths, false)
            .unwrap();
        let changes = repo.changes();
        assert!(
            changes
                .uncommitted
                .iter()
                .all(|file| file.staging() == Staging::Unstaged)
        );
    }

    #[test]
    fn staging_works_before_the_first_commit() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        let paths = [PathBuf::from("a.rs")];
        GitCli::from_env()
            .set_staged(&repo.path(), &paths, true)
            .unwrap();
        assert_eq!(repo.changes().uncommitted[0].staging(), Staging::Staged);
        GitCli::from_env()
            .set_staged(&repo.path(), &paths, false)
            .unwrap();
        assert_eq!(repo.changes().uncommitted[0].staging(), Staging::Unstaged);
    }

    #[test]
    fn staging_a_path_git_doesnt_know_fails_with_gits_reason() {
        let repo = Repo::new();
        repo.write("a.rs", "a\n");
        let error = GitCli::from_env()
            .set_staged(&repo.path(), &[PathBuf::from("missing.rs")], true)
            .unwrap_err();
        assert!(error.to_string().contains("missing.rs"), "{error}");
    }

    #[test]
    fn keeps_the_git_contract() {
        let repo = Repo::new();
        repo.write("modified.rs", "a\nb\n");
        repo.write("deleted.rs", "gone\n");
        repo.write("renamed_in_commit.rs", "one\ntwo\nthree\n");
        repo.write("deleted_in_commit.rs", "gone\n");
        repo.write("renamed.rs", "four\nfive\nsix\n");
        repo.commit_all("first");
        repo.run(&["mv", "renamed_in_commit.rs", "moved_in_commit.rs"]);
        repo.commit_all("rename");
        repo.run(&["rm", "-q", "deleted_in_commit.rs"]);
        repo.commit_all("delete");
        repo.run(&["worktree", "add", "-q", "-b", "feat", "../linked"]);
        repo.write("modified.rs", "a\nB\n");
        fs::remove_file(repo.path().join("deleted.rs")).unwrap();
        repo.write("new.rs", "new\n");
        repo.run(&["mv", "renamed.rs", "moved.rs"]);
        repo.write("moved.rs", "four\nfive\nsix\nseven\n");
        model::fixtures::contract::git(&GitCli::from_env(), &repo.path());
    }

    #[test]
    fn a_folder_outside_any_repository_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(GitCli::from_env().repository(dir.path()).is_err());
    }
}

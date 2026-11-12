//! A `GitBackend` that runs the `git` command line.

mod command;
mod parse;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context as _, Result};
use model::{Branch, GitBackend, ProjectId, Worktree};

use crate::command::git;

const LIST_WORKTREES: &[&str] = &["worktree", "list", "--porcelain", "-z"];

pub struct GitCli;

impl GitBackend for GitCli {
    fn root(&self, path: &Path) -> Result<PathBuf> {
        parse::worktrees(&git(path, LIST_WORKTREES)?)
            .into_iter()
            .next()
            .map(|worktree| worktree.path)
            .context("Git listed no worktrees")
    }

    fn worktrees(&self, project: ProjectId, root: &Path) -> Result<Vec<Worktree>> {
        let listed: Vec<_> = parse::worktrees(&git(root, LIST_WORKTREES)?)
            .into_iter()
            .filter(|worktree| worktree.checked_out)
            .collect();
        let heads: Vec<&str> = listed
            .iter()
            .filter_map(|worktree| worktree.head.as_deref())
            .collect();
        let commit_times: HashMap<String, SystemTime> = if heads.is_empty() {
            HashMap::new()
        } else {
            let args = [
                &["log", "--no-walk=unsorted", "--format=%H%x00%ct"],
                &heads[..],
            ]
            .concat();
            parse::named_times(&git(root, &args)?).collect()
        };
        listed
            .into_iter()
            .map(|worktree| {
                let committed = worktree
                    .head
                    .and_then(|head| commit_times.get(&head).copied());
                let changed = newest_change(&worktree.path)?;
                Ok(Worktree {
                    project,
                    path: worktree.path,
                    branch: worktree.branch.map(Into::into),
                    last_change: committed.max(changed).unwrap_or(SystemTime::UNIX_EPOCH),
                })
            })
            .collect()
    }

    fn branches(&self, project: ProjectId, root: &Path) -> Result<Vec<Branch>> {
        let args = [
            "for-each-ref",
            "--format=%(refname:strip=2)%00%(committerdate:unix)",
            "refs/heads",
        ];
        Ok(parse::named_times(&git(root, &args)?)
            .map(|(name, last_commit)| Branch {
                project,
                name: name.into(),
                last_commit,
            })
            .collect())
    }
}

/// When the newest uncommitted change in the worktree at `path` was saved, if it has any.
fn newest_change(path: &Path) -> Result<Option<SystemTime>> {
    let status = git(path, &["status", "--porcelain=v1", "-z", "--no-renames"])?;
    Ok(parse::status_paths(&status)
        .iter()
        // A deleted file has no time to report.
        .filter_map(|changed| fs::metadata(path.join(changed)).ok()?.modified().ok())
        .max())
}

#[cfg(test)]
mod tests {
    use std::{
        process::Command,
        time::{Duration, UNIX_EPOCH},
    };

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
            fs::create_dir(repo.dir.path().join("main")).unwrap();
            fs::write(repo.dir.path().join("gitconfig"), "").unwrap();
            repo.run(&["init", "--initial-branch=main"]);
            repo
        }

        fn path(&self) -> PathBuf {
            self.dir.path().join("main")
        }

        /// Runs Git in the main worktree with `env` set, ignoring the machine's own Git config.
        fn run_with(&self, args: &[&str], env: &[(&str, &str)]) {
            let status = Command::new("git")
                .arg("-C")
                .arg(self.path())
                .args(args)
                .env("GIT_CONFIG_GLOBAL", self.dir.path().join("gitconfig"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Pensive")
                .env("GIT_AUTHOR_EMAIL", "pensive@example.com")
                .env("GIT_COMMITTER_NAME", "Pensive")
                .env("GIT_COMMITTER_EMAIL", "pensive@example.com")
                .envs(env.iter().copied())
                .status()
                .unwrap();
            assert!(status.success(), "git {}", args.join(" "));
        }

        fn run(&self, args: &[&str]) {
            self.run_with(args, &[]);
        }

        /// Commits a change to `file`, dated `seconds` after the Unix epoch.
        fn commit(&self, seconds: u64) {
            fs::write(self.path().join("file"), seconds.to_string()).unwrap();
            self.run(&["add", "file"]);
            let date = format!("{seconds} +0000");
            self.run_with(
                &["commit", "-q", "-m", &date],
                &[("GIT_AUTHOR_DATE", &date), ("GIT_COMMITTER_DATE", &date)],
            );
        }
    }

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn worktrees(repo: &Repo) -> Vec<Worktree> {
        let root = GitCli.root(&repo.path()).unwrap();
        GitCli.worktrees(ProjectId(0), &root).unwrap()
    }

    #[test]
    fn lists_branches_with_their_last_commit() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        repo.run(&["branch", "old"]);
        repo.commit(1_100_000_000);
        let mut branches: Vec<_> = GitCli
            .branches(ProjectId(0), &repo.path())
            .unwrap()
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
    fn a_linked_worktree_shares_the_main_root() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        repo.run(&["worktree", "add", "-q", "-b", "feat", "../linked"]);
        let linked = repo.dir.path().join("linked");
        assert_eq!(
            GitCli.root(&linked).unwrap(),
            GitCli.root(&repo.path()).unwrap()
        );
        let branches: Vec<_> = worktrees(&repo)
            .into_iter()
            .map(|worktree| worktree.branch.map(|branch| branch.to_string()))
            .collect();
        assert_eq!(branches, [Some("main".into()), Some("feat".into())]);
    }

    #[test]
    fn a_detached_head_has_no_branch() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        repo.run(&["checkout", "-q", "--detach"]);
        assert_eq!(worktrees(&repo)[0].branch, None);
    }

    #[test]
    fn an_uncommitted_change_is_the_last_change() {
        let repo = Repo::new();
        repo.commit(1_000_000_000);
        assert_eq!(worktrees(&repo)[0].last_change, at(1_000_000_000));
        fs::write(repo.path().join("file"), "edited").unwrap();
        assert!(worktrees(&repo)[0].last_change > at(1_000_000_000));
    }

    #[test]
    fn a_repository_without_commits_still_reads() {
        let repo = Repo::new();
        let worktrees = worktrees(&repo);
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert!(
            GitCli
                .branches(ProjectId(0), &repo.path())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_folder_outside_any_repository_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(GitCli.root(dir.path()).is_err());
    }
}

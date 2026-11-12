//! The projects Pensive knows about, with their worktrees and branches.

use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

use gpui::SharedString;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProjectId(pub usize);

#[derive(Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: SharedString,
    pub root: PathBuf,
    /// When Pensive last opened it.
    pub last_opened: SystemTime,
}

impl Project {
    /// The name's first letter in upper case, for the letter tile that stands in for an icon.
    pub fn initial(&self) -> SharedString {
        self.name
            .chars()
            .next()
            .map(|letter| letter.to_uppercase().to_string().into())
            .unwrap_or_default()
    }
}

/// A checkout of a project. The project's own root is its main worktree.
#[derive(Clone, Debug)]
pub struct Worktree {
    pub project: ProjectId,
    pub path: PathBuf,
    /// The checked-out branch, or `None` when the head is detached.
    pub branch: Option<SharedString>,
    /// The later of the branch's newest commit and the newest uncommitted change.
    pub last_change: SystemTime,
}

#[derive(Clone, Debug)]
pub struct Branch {
    pub project: ProjectId,
    pub name: SharedString,
    pub last_commit: SystemTime,
}

/// A branch or worktree to show as one piece of work. A branch that is checked out shows once,
/// as its worktree.
#[derive(Clone, Debug)]
pub struct Work {
    pub project: ProjectId,
    pub name: SharedString,
    pub kind: WorkKind,
    pub last_change: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkKind {
    /// Checked out in the project's root.
    Main,
    /// Checked out in a linked worktree at this path.
    Linked(PathBuf),
    /// Not checked out anywhere.
    Branch,
}

/// Every known project, cached so that reading it never runs Git.
#[derive(Clone, Debug, Default)]
pub struct Projects {
    pub projects: Vec<Project>,
    pub worktrees: Vec<Worktree>,
    pub branches: Vec<Branch>,
    /// When this was last refreshed. Ages are measured from here, so views never read the clock.
    pub as_of: Option<SystemTime>,
}

impl Projects {
    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    /// Most recently opened first.
    pub fn recent(&self) -> Vec<&Project> {
        let mut projects: Vec<_> = self.projects.iter().collect();
        projects.sort_by_key(|project| std::cmp::Reverse(project.last_opened));
        projects
    }

    /// The branch checked out in the project's root.
    pub fn current_branch(&self, id: ProjectId) -> Option<&SharedString> {
        let root = &self.project(id)?.root;
        self.worktrees
            .iter()
            .find(|worktree| worktree.project == id && &worktree.path == root)?
            .branch
            .as_ref()
    }

    /// Every worktree and branch, newest change first.
    pub fn work(&self) -> Vec<Work> {
        let mut work: Vec<Work> = self
            .worktrees
            .iter()
            .map(|worktree| {
                let main = self
                    .project(worktree.project)
                    .is_some_and(|project| project.root == worktree.path);
                Work {
                    project: worktree.project,
                    name: worktree.branch.clone().unwrap_or_else(|| "detached".into()),
                    kind: if main {
                        WorkKind::Main
                    } else {
                        WorkKind::Linked(worktree.path.clone())
                    },
                    last_change: worktree.last_change,
                }
            })
            .collect();
        let checked_out = |branch: &Branch| {
            self.worktrees.iter().any(|worktree| {
                worktree.project == branch.project && worktree.branch.as_ref() == Some(&branch.name)
            })
        };
        work.extend(
            self.branches
                .iter()
                .filter(|branch| !checked_out(branch))
                .map(|branch| Work {
                    project: branch.project,
                    name: branch.name.clone(),
                    kind: WorkKind::Branch,
                    last_change: branch.last_commit,
                }),
        );
        work.sort_by_key(|work| std::cmp::Reverse(work.last_change));
        work
    }

    /// Work that changed within `window` of `as_of`, newest first.
    pub fn active(&self, window: Duration) -> Vec<Work> {
        let Some(as_of) = self.as_of else {
            return Vec::new();
        };
        self.work()
            .into_iter()
            .filter(|work| {
                as_of
                    .duration_since(work.last_change)
                    .is_ok_and(|age| age <= window)
            })
            .collect()
    }
}

#[cfg(all(test, feature = "fixtures"))]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn a_checked_out_branch_shows_once_as_its_worktree() {
        let projects = fixtures::projects();
        let work = projects.work();
        let scopes: Vec<_> = work
            .iter()
            .filter(|work| work.name == "feat/palette-scopes")
            .collect();
        assert_eq!(scopes.len(), 1);
        assert!(matches!(scopes[0].kind, WorkKind::Linked(_)));
    }

    #[test]
    fn active_leaves_out_work_older_than_the_window() {
        let projects = fixtures::projects();
        let week = Duration::from_secs(7 * 24 * 60 * 60);
        let active = projects.active(week);
        assert!(active.iter().all(|work| work.name != "hotfix/1.8.2"));
        assert!(active.iter().any(|work| work.name == "feat/palette-scopes"));
    }
}

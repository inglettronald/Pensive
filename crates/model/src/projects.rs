//! The projects Pensive knows about, with their worktrees and branches.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::{Result, anyhow};
use gpui::{Context, SharedString, Task};

use crate::GitBackend;

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
#[derive(Clone, Default)]
pub struct Projects {
    pub projects: Vec<Project>,
    pub worktrees: Vec<Worktree>,
    pub branches: Vec<Branch>,
    /// When this was last refreshed. Ages are measured from here, so views never read the clock.
    pub as_of: Option<SystemTime>,
    pub(crate) git: Option<Arc<dyn GitBackend>>,
    pub(crate) next_id: usize,
}

impl Projects {
    /// No projects yet, reading Git through `git`.
    pub fn new(git: Arc<dyn GitBackend>) -> Self {
        Self {
            git: Some(git),
            ..Self::default()
        }
    }

    /// Adds the repository that contains `path`, reading it off the UI thread. Opening one that's
    /// already here reads it again.
    pub fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Task<Result<ProjectId>> {
        let Some(git) = self.git.clone() else {
            return Task::ready(Err(anyhow!("no Git backend")));
        };
        let id = ProjectId(self.next_id);
        self.next_id += 1;
        cx.spawn(async move |this, cx| {
            let repository = cx
                .background_executor()
                .spawn(async move { Repository::read(&*git, id, &path) })
                .await?;
            this.update(cx, |this, cx| {
                let id = this.add(repository);
                cx.notify();
                id
            })
        })
    }

    /// Adds `repository`, replacing what's known of a project at the same root.
    fn add(&mut self, repository: Repository) -> ProjectId {
        let Repository {
            mut project,
            mut worktrees,
            mut branches,
        } = repository;
        if let Some(known) = self
            .projects
            .iter()
            .find(|known| known.root == project.root)
        {
            project.id = known.id;
            worktrees
                .iter_mut()
                .for_each(|worktree| worktree.project = known.id);
            branches
                .iter_mut()
                .for_each(|branch| branch.project = known.id);
        }
        let id = project.id;
        self.projects.retain(|project| project.id != id);
        self.worktrees.retain(|worktree| worktree.project != id);
        self.branches.retain(|branch| branch.project != id);
        self.projects.push(project);
        self.worktrees.extend(worktrees);
        self.branches.extend(branches);
        self.as_of = Some(SystemTime::now());
        id
    }

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

/// A repository as Git reports it, before it joins `Projects`.
struct Repository {
    project: Project,
    worktrees: Vec<Worktree>,
    branches: Vec<Branch>,
}

impl Repository {
    fn read(git: &dyn GitBackend, id: ProjectId, path: &Path) -> Result<Self> {
        let root = git.root(path)?;
        let name = root
            .file_name()
            .unwrap_or(root.as_os_str())
            .to_string_lossy()
            .into_owned();
        Ok(Self {
            worktrees: git.worktrees(id, &root)?,
            branches: git.branches(id, &root)?,
            project: Project {
                id,
                name: name.into(),
                root,
                last_opened: SystemTime::now(),
            },
        })
    }
}

#[cfg(all(test, feature = "fixtures"))]
mod tests {
    use super::*;
    use crate::fixtures::{self, FakeGit};

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

    #[test]
    fn opening_a_linked_worktree_adds_its_whole_project() {
        let read = Repository::read(
            &FakeGit::default(),
            ProjectId(0),
            Path::new("~/Projects/pensive-wt/palette-scopes"),
        )
        .unwrap();
        assert_eq!(read.project.root, Path::new("~/Projects/pensive"));
        assert_eq!(read.worktrees.len(), 4);
    }

    #[test]
    fn opening_a_project_twice_keeps_one() {
        let git = FakeGit::default();
        let mut projects = Projects::default();
        for (id, path) in [(0, "~/code/harbor"), (1, "~/code/harbor-wt/tooltip-clip")] {
            let read = Repository::read(&git, ProjectId(id), Path::new(path)).unwrap();
            projects.add(read);
        }
        assert_eq!(projects.projects.len(), 1);
        assert_eq!(projects.worktrees.len(), 3);
        assert!(
            projects
                .work()
                .iter()
                .all(|work| work.project == ProjectId(0))
        );
    }
}

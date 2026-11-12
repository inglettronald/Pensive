//! The projects Pensive knows about, with their worktrees and branches.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use gpui::{Context, SharedString, Task};

use crate::{Changes, GitBackend, Repository};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProjectId(pub usize);

/// A repository Pensive has opened.
#[derive(Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub name: SharedString,
    /// When Pensive last opened it.
    pub last_opened: SystemTime,
    pub repository: Repository,
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

/// A checkout of a repository.
#[derive(Clone, Debug)]
pub struct Worktree {
    pub path: PathBuf,
    /// The checked-out branch, or `None` when the head is detached.
    pub branch: Option<SharedString>,
    /// The later of the branch's newest commit and the newest uncommitted change.
    pub last_change: SystemTime,
}

#[derive(Clone, Debug)]
pub struct Branch {
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
#[derive(Clone, Debug)]
pub struct Projects {
    pub projects: Vec<Project>,
    /// When this was last refreshed. Ages are measured from here, so views never read the clock.
    pub as_of: Option<SystemTime>,
    /// Each worktree's changes, by its path, once they've been read.
    changes: HashMap<PathBuf, Changes>,
    git: Arc<dyn GitBackend>,
}

impl Projects {
    /// No projects yet, reading Git through `git`.
    pub fn new(git: Arc<dyn GitBackend>) -> Self {
        Self {
            projects: Vec::new(),
            as_of: None,
            changes: HashMap::new(),
            git,
        }
    }

    /// Adds the repository that contains `path`, reading it off the UI thread. Opening one that's
    /// already here reads it again.
    pub fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Task<Result<ProjectId>> {
        let git = self.git.clone();
        cx.spawn(async move |this, cx| {
            let repository = cx
                .background_executor()
                .spawn(async move { git.repository(&path) })
                .await?;
            this.update(cx, |this, cx| {
                let id = this.add(repository, SystemTime::now());
                cx.notify();
                id
            })
        })
    }

    /// Adds `repository` as opened at `opened`, or updates the project already at its root.
    pub(crate) fn add(&mut self, repository: Repository, opened: SystemTime) -> ProjectId {
        self.as_of = Some(opened);
        let root = &repository.root;
        if let Some(known) = self
            .projects
            .iter_mut()
            .find(|known| &known.repository.root == root)
        {
            known.repository = repository;
            known.last_opened = opened;
            return known.id;
        }
        let id = ProjectId(
            self.projects
                .iter()
                .map(|project| project.id.0 + 1)
                .max()
                .unwrap_or(0),
        );
        let name = root
            .file_name()
            .unwrap_or(root.as_os_str())
            .to_string_lossy()
            .into_owned();
        self.projects.push(Project {
            id,
            name: name.into(),
            last_opened: opened,
            repository,
        });
        id
    }

    /// The changes in the worktree at `worktree`, once read.
    pub fn changes(&self, worktree: &Path) -> Option<&Changes> {
        self.changes.get(worktree)
    }

    /// Reads the changes in the worktree at `worktree` off the UI thread, replacing what's
    /// cached.
    pub fn read_changes(&mut self, worktree: PathBuf, cx: &mut Context<Self>) -> Task<Result<()>> {
        let git = self.git.clone();
        cx.spawn(async move |this, cx| {
            let read = worktree.clone();
            let changes = cx
                .background_executor()
                .spawn(async move { git.changes(&read) })
                .await?;
            this.update(cx, |this, cx| {
                this.insert_changes(worktree, changes);
                cx.notify();
            })
        })
    }

    pub(crate) fn insert_changes(&mut self, worktree: PathBuf, changes: Changes) {
        self.changes.insert(worktree, changes);
    }

    /// Stages or unstages one file in the cached changes. Git's index follows once a backend
    /// writes it.
    pub fn set_staged(
        &mut self,
        worktree: &Path,
        path: &Path,
        staged: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(changes) = self.changes.get_mut(worktree) {
            changes.set_staged(path, staged);
            cx.notify();
        }
    }

    /// [`Self::set_staged`] for every file.
    pub fn set_all_staged(&mut self, worktree: &Path, staged: bool, cx: &mut Context<Self>) {
        if let Some(changes) = self.changes.get_mut(worktree) {
            changes.set_all_staged(staged);
            cx.notify();
        }
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
        let repository = &self.project(id)?.repository;
        repository
            .worktrees
            .iter()
            .find(|worktree| worktree.path == repository.root)?
            .branch
            .as_ref()
    }

    /// Every worktree and branch, newest change first.
    pub fn work(&self) -> Vec<Work> {
        let mut work = Vec::new();
        for project in &self.projects {
            let Repository {
                root,
                worktrees,
                branches,
            } = &project.repository;
            work.extend(worktrees.iter().map(|worktree| Work {
                project: project.id,
                name: worktree.branch.clone().unwrap_or_else(|| "detached".into()),
                kind: if &worktree.path == root {
                    WorkKind::Main
                } else {
                    WorkKind::Linked(worktree.path.clone())
                },
                last_change: worktree.last_change,
            }));
            let checked_out = |branch: &Branch| {
                worktrees
                    .iter()
                    .any(|worktree| worktree.branch.as_ref() == Some(&branch.name))
            };
            work.extend(
                branches
                    .iter()
                    .filter(|branch| !checked_out(branch))
                    .map(|branch| Work {
                        project: project.id,
                        name: branch.name.clone(),
                        kind: WorkKind::Branch,
                        last_change: branch.last_commit,
                    }),
            );
        }
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
    use std::path::Path;

    use gpui::{AppContext as _, TestAppContext};

    use super::*;
    use crate::fixtures::{self, FakeGit, HARBOR};

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
        let mut projects = Projects::new(Arc::new(FakeGit::default()));
        let repository = FakeGit::default()
            .repository(Path::new("~/Projects/pensive-wt/palette-scopes"))
            .unwrap();
        let id = projects.add(repository, SystemTime::now());
        let project = projects.project(id).unwrap();
        assert_eq!(project.repository.root, Path::new("~/Projects/pensive"));
        assert_eq!(project.repository.worktrees.len(), 4);
    }

    #[test]
    fn a_new_project_takes_an_unused_id() {
        let mut projects = fixtures::projects();
        let repository = Repository {
            root: "~/code/new".into(),
            worktrees: Vec::new(),
            branches: Vec::new(),
        };
        let id = projects.add(repository, SystemTime::now());
        assert_eq!(projects.projects.len(), 7);
        assert_eq!(projects.project(id).unwrap().name, "new");
    }

    #[gpui::test]
    async fn open_reads_the_repository_and_keeps_one_project_per_root(cx: &mut TestAppContext) {
        let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
        let mut opened = Vec::new();
        for path in ["~/code/harbor", "~/code/harbor-wt/tooltip-clip"] {
            let open = projects.update(cx, |projects, cx| projects.open(path.into(), cx));
            opened.push(open.await.unwrap());
        }
        assert_eq!(opened, [ProjectId(0), ProjectId(0)]);
        projects.read_with(cx, |projects, _| {
            assert_eq!(projects.projects.len(), 1);
            assert_eq!(
                projects.current_branch(ProjectId(0)).unwrap(),
                "morgan/ui-scaling"
            );
        });
    }

    #[gpui::test]
    async fn open_fails_outside_any_repository(cx: &mut TestAppContext) {
        let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
        let open = projects.update(cx, |projects, cx| projects.open("~/nowhere".into(), cx));
        assert!(open.await.is_err());
        projects.read_with(cx, |projects, _| assert!(projects.projects.is_empty()));
    }

    #[gpui::test]
    async fn read_changes_caches_them_by_worktree(cx: &mut TestAppContext) {
        let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
        let read = projects.update(cx, |projects, cx| {
            projects.read_changes(fixtures::PENSIVE_ROOT.into(), cx)
        });
        read.await.unwrap();
        projects.read_with(cx, |projects, _| {
            let changes = projects.changes(Path::new(fixtures::PENSIVE_ROOT)).unwrap();
            assert_eq!(changes.uncommitted.len(), 7);
            assert!(projects.changes(Path::new("~/code/harbor")).is_none());
        });
    }

    #[test]
    fn sample_ids_match_their_names() {
        let projects = fixtures::projects();
        assert_eq!(projects.project(HARBOR).unwrap().name, "harbor");
    }
}

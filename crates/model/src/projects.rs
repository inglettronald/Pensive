//! The projects Pensive knows about, with their worktrees and branches.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Task};

use crate::{
    Backend, Changes, Document, DocumentEvent, FileKey, FileVersions, GitBackend, Origin,
    Repository, Source, clock, document::Contents, queue::Queue,
};

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
pub struct Projects {
    pub projects: Vec<Project>,
    /// The time ages are measured from, fixed in fixtures so mocks don't change. The clock
    /// otherwise.
    pub as_of: Option<SystemTime>,
    /// Each worktree's changes, by its path, once they've been read.
    changes: HashMap<PathBuf, Changes>,
    /// Files that have been shown, by worktree and file. They stay open, so edits that aren't
    /// saved survive switching files.
    documents: HashMap<(PathBuf, FileKey), Entity<Document>>,
    /// Documents being opened, so a second request waits for the first.
    opening: HashSet<(PathBuf, FileKey)>,
    /// Files with a binary version, which have no lines to show.
    binary: HashSet<(PathBuf, FileKey)>,
    git: Backend<dyn GitBackend>,
    /// Writes to Git's index, one at a time, since two at once would both need the index's lock.
    staging: Queue,
    /// How many reads of each worktree's changes have started.
    reads: HashMap<PathBuf, u64>,
    _saves: Vec<Subscription>,
}

impl Projects {
    /// No projects yet, reading Git through `git`.
    pub fn new(git: Arc<dyn GitBackend>) -> Self {
        Self {
            projects: Vec::new(),
            as_of: None,
            changes: HashMap::new(),
            documents: HashMap::new(),
            opening: HashSet::new(),
            binary: HashSet::new(),
            git: Backend::new(git, Origin::Git),
            staging: Queue::default(),
            reads: HashMap::new(),
            _saves: Vec::new(),
        }
    }

    /// Adds the repository that contains `path`, reading it off the UI thread. Opening one that's
    /// already here reads it again.
    pub fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Task<Result<ProjectId>> {
        let title = format!("Couldn't open {}", path.display());
        let read = self.git.call(title, move |git| git.repository(&path), cx);
        cx.spawn(async move |this, cx| {
            let repository = read.await?;
            this.update(cx, |this, cx| {
                let id = this.add(repository, clock::now());
                cx.notify();
                id
            })
        })
    }

    /// Adds `repository` as opened at `opened`, or updates the project already at its root.
    pub(crate) fn add(&mut self, repository: Repository, opened: SystemTime) -> ProjectId {
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
    /// cached. Open uncommitted files are read again too, since they may have changed. A commit's
    /// files never change. Reads can finish in any order, so one that finishes after a newer one
    /// started is dropped.
    pub fn read_changes(&mut self, worktree: PathBuf, cx: &mut Context<Self>) -> Task<Result<()>> {
        let number = self.start_read(&worktree);
        let path = worktree.clone();
        let read = self
            .git
            .call("Couldn't read changes", move |git| git.changes(&path), cx);
        cx.spawn(async move |this, cx| {
            let changes = read.await?;
            this.update(cx, |this, cx| {
                if this.finish_read(&worktree, number, changes) {
                    this.refresh_documents(&worktree, cx);
                    cx.notify();
                }
            })
        })
    }

    /// Counts a read of the worktree's changes, and returns its number.
    fn start_read(&mut self, worktree: &Path) -> u64 {
        let started = self.reads.entry(worktree.to_path_buf()).or_default();
        *started += 1;
        *started
    }

    /// Caches `changes` from read `number`, unless a newer read started since. Returns whether it
    /// did.
    fn finish_read(&mut self, worktree: &Path, number: u64, changes: Changes) -> bool {
        if self.reads.get(worktree) != Some(&number) {
            return false;
        }
        self.insert_changes(worktree.to_path_buf(), changes);
        true
    }

    pub(crate) fn insert_changes(&mut self, worktree: PathBuf, changes: Changes) {
        self.changes.insert(worktree, changes);
    }

    /// The file `key` names, once [`Self::open_document`] has read it.
    pub fn document(&self, worktree: &Path, key: &FileKey) -> Option<&Entity<Document>> {
        self.documents.get(&(worktree.to_path_buf(), key.clone()))
    }

    /// Whether the file `key` names turned out to be binary when it was opened.
    pub fn is_binary(&self, worktree: &Path, key: &FileKey) -> bool {
        self.binary.contains(&(worktree.to_path_buf(), key.clone()))
    }

    /// Reads both versions of the file `key` names, off the UI thread, as a document: editable
    /// when it's uncommitted, read-only in a commit. Does nothing when it's open, being opened or
    /// binary.
    pub fn open_document(
        &mut self,
        worktree: PathBuf,
        key: FileKey,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let entry = (worktree, key);
        if self.documents.contains_key(&entry)
            || self.binary.contains(&entry)
            || !self.opening.insert(entry.clone())
        {
            return Task::ready(Ok(()));
        }
        let read = self.read_versions(&entry.0, &entry.1, cx);
        cx.spawn(async move |this, cx| {
            let contents = match read.await {
                Ok(versions) => Ok(cx
                    .background_executor()
                    .spawn(async move { Contents::new(versions) })
                    .await),
                Err(error) => Err(error),
            };
            this.update(cx, |this, cx| {
                this.opening.remove(&entry);
                let (worktree, key) = entry.clone();
                let Some(contents) = contents? else {
                    this.binary.insert(entry);
                    cx.notify();
                    return Ok(());
                };
                let git = this.git.clone();
                let document = cx.new(|_| Document::new(worktree.clone(), key, contents, git));
                let saved = cx.subscribe(&document, move |this, _, event, cx| match event {
                    DocumentEvent::Saved => this.read_changes(worktree.clone(), cx).detach(),
                });
                this._saves.push(saved);
                this.documents.insert(entry, document);
                cx.notify();
                Ok(())
            })?
        })
    }

    /// Both versions of the file `key` names, following a rename the changes list.
    fn read_versions(
        &self,
        worktree: &Path,
        key: &FileKey,
        cx: &mut Context<Self>,
    ) -> Task<Result<FileVersions>> {
        let renamed_from = self
            .changes(worktree)
            .and_then(|changes| changes.find(key))
            .and_then(|file| file.renamed_from.clone());
        let (worktree, key) = (worktree.to_path_buf(), key.clone());
        let title = format!("Couldn't open {}", key.path.display());
        self.git.call(
            title,
            move |git| git.versions(&worktree, &key, renamed_from.as_deref()),
            cx,
        )
    }

    /// Reads each open uncommitted file in `worktree` again, for its document to follow the disk.
    fn refresh_documents(&mut self, worktree: &Path, cx: &mut Context<Self>) {
        let open: Vec<_> = self
            .documents
            .iter()
            .filter(|((path, key), _)| path == worktree && key.source == Source::Uncommitted)
            .map(|((_, key), document)| {
                let stamp = document.read(cx).disk_stamp();
                (key.clone(), document.downgrade(), stamp)
            })
            .collect();
        for (key, document, stamp) in open {
            let read = self.read_versions(worktree, &key, cx);
            cx.spawn(async move |_, cx| {
                let Ok(versions) = read.await else {
                    return;
                };
                document
                    .update(cx, |document, cx| document.refresh(stamp, versions, cx))
                    .ok();
            })
            .detach();
        }
    }

    /// Stages all of the uncommitted file at `path`, or unstages it. The cached changes show it
    /// at once. Then Git's index is written, after any write before it, and the changes are read
    /// again, so they end up as Git has them even when the write fails.
    pub fn set_staged(
        &mut self,
        worktree: &Path,
        path: &Path,
        staged: bool,
        cx: &mut Context<Self>,
    ) {
        self.write_staging(worktree, vec![path.to_path_buf()], staged, cx);
    }

    /// [`Self::set_staged`] for every uncommitted file.
    pub fn set_all_staged(&mut self, worktree: &Path, staged: bool, cx: &mut Context<Self>) {
        let paths = self.changes(worktree).map_or_else(Vec::new, |changes| {
            changes
                .uncommitted
                .iter()
                .map(|file| file.path.clone())
                .collect()
        });
        self.write_staging(worktree, paths, staged, cx);
    }

    fn write_staging(
        &mut self,
        worktree: &Path,
        paths: Vec<PathBuf>,
        staged: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(changes) = self.changes.get_mut(worktree) else {
            return;
        };
        let mut pathspecs = Vec::new();
        for path in paths {
            changes.set_staged(&path, staged);
            // A rename is staged or unstaged with the path it came from.
            if let Some(from) = changes
                .file(&path)
                .and_then(|file| file.renamed_from.clone())
            {
                pathspecs.push(from);
            }
            pathspecs.push(path);
        }
        cx.notify();

        let title = if staged {
            "Couldn't stage"
        } else {
            "Couldn't unstage"
        };
        let worktree = worktree.to_path_buf();
        self.staging.push(
            async move |this, cx| {
                let path = worktree.clone();
                let Ok(write) = this.update(cx, |this, cx| {
                    this.git.call(
                        title,
                        move |git| git.set_staged(&path, &pathspecs, staged),
                        cx,
                    )
                }) else {
                    return;
                };
                // The backend reports a write Git refused. The read shows what Git has either way.
                write.await.ok();
                if let Ok(read) = this.update(cx, |this, cx| this.read_changes(worktree, cx)) {
                    read.await.ok();
                }
            },
            cx,
        );
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

    pub fn now(&self) -> SystemTime {
        self.as_of.unwrap_or_else(clock::now)
    }

    /// Work that changed within `window` of now, newest first.
    pub fn active(&self, window: Duration) -> Vec<Work> {
        let now = self.now();
        self.work()
            .into_iter()
            .filter(|work| {
                now.duration_since(work.last_change)
                    .is_ok_and(|age| age <= window)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gpui::TestAppContext;

    use super::*;
    use crate::{
        Notices, Staging,
        fixtures::{self, FakeGit, HARBOR},
    };

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
    fn ages_count_from_now_not_from_when_a_project_opened() {
        let mut projects = Projects::new(Arc::new(FakeGit::default()));
        let opened = SystemTime::UNIX_EPOCH;
        let repository = Repository {
            root: "~/code/old".into(),
            worktrees: vec![Worktree {
                path: "~/code/old".into(),
                branch: Some("main".into()),
                last_change: opened,
            }],
            branches: Vec::new(),
        };
        projects.add(repository, opened);
        let day = Duration::from_secs(24 * 60 * 60);
        assert!(projects.now() > opened + day);
        assert!(projects.active(day).is_empty());
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
        let id = projects.add(repository, SystemTime::UNIX_EPOCH);
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
        let id = projects.add(repository, SystemTime::UNIX_EPOCH);
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
    async fn open_fails_outside_any_repository_and_reports_it(cx: &mut TestAppContext) {
        let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
        let open = projects.update(cx, |projects, cx| projects.open("~/nowhere".into(), cx));
        assert!(open.await.is_err());
        projects.read_with(cx, |projects, _| assert!(projects.projects.is_empty()));
        let notices = cx.update(Notices::global);
        notices.read_with(cx, |notices, _| {
            let held = notices.held();
            assert_eq!(held[0].notice.title, "Couldn't open ~/nowhere");
            // FakeGit has no such repository, which is a gap in the samples, not a Git failure.
            assert_eq!(held[0].notice.origin, Origin::Fixture);
        });
    }

    #[gpui::test]
    async fn staging_is_written_to_git_and_survives_the_next_read(cx: &mut TestAppContext) {
        let root = PathBuf::from(fixtures::PENSIVE_ROOT);
        let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
        let staging = |cx: &mut TestAppContext, path: &Path| {
            projects.read_with(cx, |projects, _| {
                projects
                    .changes(&root)
                    .unwrap()
                    .file(path)
                    .unwrap()
                    .staging()
            })
        };
        projects
            .update(cx, |projects, cx| projects.read_changes(root.clone(), cx))
            .await
            .unwrap();
        let path = projects.read_with(cx, |projects, _| {
            let changes = projects.changes(&root).unwrap();
            let unstaged = changes.uncommitted.iter();
            let mut unstaged = unstaged.filter(|file| file.staging() == Staging::Unstaged);
            unstaged.next().unwrap().path.clone()
        });

        projects.update(cx, |projects, cx| {
            projects.set_staged(&root, &path, true, cx)
        });
        assert_eq!(staging(cx, &path), Staging::Staged);
        cx.run_until_parked();
        // Saving a file reads the changes again, as here.
        projects
            .update(cx, |projects, cx| projects.read_changes(root.clone(), cx))
            .await
            .unwrap();
        assert_eq!(staging(cx, &path), Staging::Staged);
    }

    #[test]
    fn a_read_that_finishes_after_a_newer_one_started_is_dropped() {
        let mut projects = Projects::new(Arc::new(FakeGit::default()));
        let root = Path::new(fixtures::PENSIVE_ROOT);
        let older = projects.start_read(root);
        let newer = projects.start_read(root);
        let mut staged = fixtures::changes();
        staged.set_all_staged(true);

        assert!(projects.finish_read(root, newer, staged));
        assert!(!projects.finish_read(root, older, fixtures::changes()));
        let cached = projects.changes(root).unwrap();
        assert_eq!(cached.staging(), Staging::Staged);
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

//! A worktree's uncommitted files and its branch's commits, with how far it is from its upstream.

use std::{
    ops::Range,
    path::{Path, PathBuf},
};

use gpui::SharedString;

/// How a file changed, as Git's status letters say it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    /// Moved from the path in [`FileChange::renamed_from`], perhaps with edits.
    Renamed,
    /// New and not yet added. Only the worktree side can be untracked.
    Untracked,
    /// Left mid-merge with conflicts to resolve.
    Conflicted,
}

/// How much of a file's change the next commit takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Staging {
    Unstaged,
    /// Some hunks are staged and some aren't, as when Git reports a file as `MM`.
    Partly,
    Staged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    /// A struct, enum, trait or class.
    Type,
    /// A Markdown heading.
    Heading,
}

/// A function, type or heading that a file's hunks touch.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: SharedString,
    pub added: usize,
    pub removed: usize,
}

/// What happened to a line, with its number in the new file where it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineChange {
    Context(usize),
    Added(usize),
    Removed,
}

#[derive(Clone, Debug)]
pub struct HunkLine {
    pub change: LineChange,
    pub text: SharedString,
    /// Byte ranges of the words that changed, when the line pairs with one on the other side.
    pub changed: Vec<Range<usize>>,
}

#[derive(Clone, Debug)]
pub struct Hunk {
    /// A line shown above the hunk to introduce it, such as the line ranges it covers.
    pub header: SharedString,
    pub lines: Vec<HunkLine>,
}

#[derive(Clone, Debug)]
pub struct FileChange {
    /// Relative to the worktree's root.
    pub path: PathBuf,
    /// Where a renamed file came from.
    pub renamed_from: Option<PathBuf>,
    /// The staged part: what the index changes against HEAD, if anything.
    pub index: Option<FileStatus>,
    /// The unstaged part: what the worktree changes against the index, if anything.
    pub worktree: Option<FileStatus>,
    pub added: usize,
    pub removed: usize,
    pub symbols: Vec<Symbol>,
}

impl FileChange {
    pub fn name(&self) -> SharedString {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned().into())
            .unwrap_or_default()
    }

    /// The folder it's in, relative to the root, or an empty string at the root.
    pub fn folder(&self) -> SharedString {
        self.path
            .parent()
            .map(|folder| folder.to_string_lossy().into_owned().into())
            .unwrap_or_default()
    }

    /// The status to show: a conflict first, then the staged side, then the unstaged one.
    pub fn status(&self) -> FileStatus {
        if self.worktree == Some(FileStatus::Conflicted) {
            return FileStatus::Conflicted;
        }
        self.index.or(self.worktree).unwrap_or(FileStatus::Modified)
    }

    pub fn staging(&self) -> Staging {
        match (self.index, self.worktree) {
            (Some(_), None) => Staging::Staged,
            (Some(_), Some(_)) => Staging::Partly,
            (None, _) => Staging::Unstaged,
        }
    }

    /// Moves the whole change into the index, or all of it back out.
    fn set_staged(&mut self, staged: bool) {
        let status = self.status();
        if staged {
            self.index = Some(match status {
                FileStatus::Untracked => FileStatus::Added,
                status => status,
            });
            self.worktree = None;
        } else {
            self.worktree = Some(match status {
                FileStatus::Added => FileStatus::Untracked,
                status => status,
            });
            self.index = None;
        }
    }
}

/// Which list a file is in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Uncommitted,
    Commit(SharedString),
}

/// A file in one of a worktree's lists, by its path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileKey {
    pub source: Source,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Commit {
    pub sha: SharedString,
    pub title: SharedString,
    pub description: Option<SharedString>,
    /// Whether the upstream has it.
    pub pushed: bool,
    /// Each file's change sits on its `index` side, so it shows as staged: all of it is in.
    pub files: Vec<FileChange>,
}

/// The branch this one started from.
#[derive(Clone, Debug)]
pub struct Base {
    pub branch: SharedString,
}

/// The remote branch this one tracks.
#[derive(Clone, Debug)]
pub struct Upstream {
    pub remote: SharedString,
    /// Commits the upstream has that this branch doesn't.
    pub behind: usize,
}

/// One worktree's changes, cached so that reading them never runs Git.
#[derive(Clone, Debug, Default)]
pub struct Changes {
    pub base: Option<Base>,
    pub upstream: Option<Upstream>,
    pub uncommitted: Vec<FileChange>,
    /// The branch's own commits since its base, newest first.
    pub commits: Vec<Commit>,
}

impl Changes {
    /// How many commits the branch has over its base.
    pub fn ahead_of_base(&self) -> usize {
        self.commits.len()
    }

    /// How many commits the upstream doesn't have yet.
    pub fn ahead_of_upstream(&self) -> usize {
        self.commits.iter().filter(|commit| !commit.pushed).count()
    }

    /// Files with at least part of their change staged.
    pub fn staged_count(&self) -> usize {
        self.uncommitted
            .iter()
            .filter(|file| file.staging() != Staging::Unstaged)
            .count()
    }

    /// How much of the uncommitted work the next commit takes, across every file.
    pub fn staging(&self) -> Staging {
        let staged = |file: &FileChange| file.staging() == Staging::Staged;
        if !self.uncommitted.is_empty() && self.uncommitted.iter().all(staged) {
            Staging::Staged
        } else if self.staged_count() > 0 {
            Staging::Partly
        } else {
            Staging::Unstaged
        }
    }

    pub fn file(&self, path: &Path) -> Option<&FileChange> {
        self.uncommitted.iter().find(|file| file.path == path)
    }

    /// Stages or unstages all of one uncommitted file.
    pub fn set_staged(&mut self, path: &Path, staged: bool) {
        if let Some(file) = self.uncommitted.iter_mut().find(|file| file.path == path) {
            file.set_staged(staged);
        }
    }

    pub fn set_all_staged(&mut self, staged: bool) {
        for file in &mut self.uncommitted {
            file.set_staged(staged);
        }
    }

    pub fn commit(&self, sha: &str) -> Option<&Commit> {
        self.commits.iter().find(|commit| commit.sha == sha)
    }

    /// The file `key` names, in the uncommitted list or its commit.
    pub fn find(&self, key: &FileKey) -> Option<&FileChange> {
        let files = match &key.source {
            Source::Uncommitted => &self.uncommitted,
            Source::Commit(sha) => &self.commit(sha)?.files,
        };
        files.iter().find(|file| file.path == key.path)
    }
}

#[cfg(all(test, feature = "fixtures"))]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn staging_everything_then_one_file_less() {
        let mut changes = fixtures::changes();
        changes.set_all_staged(true);
        assert_eq!(changes.staging(), Staging::Staged);
        let first = changes.uncommitted[0].path.clone();
        changes.set_staged(&first, false);
        assert_eq!(changes.staging(), Staging::Partly);
        assert_eq!(changes.staged_count(), changes.uncommitted.len() - 1);
    }

    #[test]
    fn unstaging_a_new_file_makes_it_untracked_again() {
        let mut file = fixtures::changes().uncommitted[1].clone();
        assert_eq!(file.index, Some(FileStatus::Added));
        file.set_staged(false);
        assert_eq!(file.worktree, Some(FileStatus::Untracked));
        file.set_staged(true);
        assert_eq!(file.index, Some(FileStatus::Added));
    }

    #[test]
    fn a_file_with_both_sides_is_partly_staged() {
        let changes = fixtures::changes();
        let partly = changes
            .uncommitted
            .iter()
            .find(|file| file.index.is_some() && file.worktree.is_some())
            .expect("the sample has a partly staged file");
        assert_eq!(partly.staging(), Staging::Partly);
    }

    #[test]
    fn ahead_counts_come_from_the_commits() {
        let changes = fixtures::changes();
        assert_eq!(changes.ahead_of_base(), 3);
        assert_eq!(changes.ahead_of_upstream(), 2);
    }

    #[test]
    fn a_file_splits_into_folder_and_name() {
        let changes = fixtures::changes();
        let file = &changes.uncommitted[1];
        assert_eq!(file.name(), "row.rs");
        assert_eq!(file.folder(), "crates/components/src/palette");
    }
}

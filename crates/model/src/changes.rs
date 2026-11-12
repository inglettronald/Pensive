//! A worktree's uncommitted files and its branch's commits, with how far it is from its upstream.

use std::{
    ops::Range,
    path::{Path, PathBuf},
};

use gpui::SharedString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineChange {
    Context,
    Added,
    Removed,
}

#[derive(Clone, Debug)]
pub struct HunkLine {
    pub change: LineChange,
    /// The line's number in the new file. Removed lines have none.
    pub number: Option<usize>,
    pub text: SharedString,
    /// Byte ranges of the words that changed, when the line pairs with one on the other side.
    pub changed: Vec<Range<usize>>,
}

#[derive(Clone, Debug)]
pub struct Hunk {
    /// The `@@` line, with the enclosing context Git prints after it.
    pub header: SharedString,
    pub lines: Vec<HunkLine>,
}

#[derive(Clone, Debug)]
pub struct FileChange {
    /// Relative to the worktree's root.
    pub path: PathBuf,
    pub status: FileStatus,
    pub added: usize,
    pub removed: usize,
    /// Always true in a commit, since every file in it is in it.
    pub staged: bool,
    pub symbols: Vec<Symbol>,
    pub hunks: Vec<Hunk>,
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
}

#[derive(Clone, Debug)]
pub struct Commit {
    pub sha: SharedString,
    pub title: SharedString,
    pub description: Option<SharedString>,
    /// Whether the upstream has it.
    pub pushed: bool,
    pub files: Vec<FileChange>,
}

/// The remote branch this one tracks, and how far apart they are.
#[derive(Clone, Debug)]
pub struct Upstream {
    pub remote: SharedString,
    pub ahead: usize,
    pub behind: usize,
}

/// One worktree's changes, cached so that reading them never runs Git.
#[derive(Clone, Debug, Default)]
pub struct Changes {
    /// The branch this one started from, such as `master`, and how many commits it's ahead.
    pub base: Option<(SharedString, usize)>,
    pub upstream: Option<Upstream>,
    pub uncommitted: Vec<FileChange>,
    /// The branch's own commits, newest first.
    pub commits: Vec<Commit>,
}

impl Changes {
    pub fn staged_count(&self) -> usize {
        self.uncommitted.iter().filter(|file| file.staged).count()
    }

    /// Stages or unstages one uncommitted file.
    pub fn set_staged(&mut self, path: &Path, staged: bool) {
        if let Some(file) = self.uncommitted.iter_mut().find(|file| file.path == path) {
            file.staged = staged;
        }
    }

    pub fn set_all_staged(&mut self, staged: bool) {
        for file in &mut self.uncommitted {
            file.staged = staged;
        }
    }

    pub fn commit(&self, sha: &str) -> Option<&Commit> {
        self.commits.iter().find(|commit| commit.sha == sha)
    }
}

#[cfg(all(test, feature = "fixtures"))]
mod tests {
    use crate::fixtures;

    #[test]
    fn staging_everything_then_one_file_less() {
        let mut changes = fixtures::changes();
        changes.set_all_staged(true);
        assert_eq!(changes.staged_count(), changes.uncommitted.len());
        let first = changes.uncommitted[0].path.clone();
        changes.set_staged(&first, false);
        assert_eq!(changes.staged_count(), changes.uncommitted.len() - 1);
    }

    #[test]
    fn a_file_splits_into_folder_and_name() {
        let changes = fixtures::changes();
        let file = &changes.uncommitted[1];
        assert_eq!(file.name(), "row.rs");
        assert_eq!(file.folder(), "crates/components/src/palette");
    }
}

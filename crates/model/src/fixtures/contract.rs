//! Checks that a backend keeps the promises its trait makes, so the real backend's tests and the
//! fake's run the same ones.

use std::path::{Path, PathBuf};

use crate::{
    ConfigBackend, FileChange, FileKey, FileStatus, FileVersions, GitBackend, Source, Staging,
    Version,
};

/// Checks `backend` on the main worktree at `worktree`, whose repository must also have a linked
/// worktree. Its uncommitted files must include a modified, a new and a deleted file, and its
/// branch's commits must rename a file and delete one. An uncommitted rename is checked when there
/// is one. The checks write a file and change what's staged.
pub fn git(backend: &dyn GitBackend, worktree: &Path) {
    lists_the_main_worktree_first(backend, worktree);
    a_written_file_reads_back(backend, worktree);
    uncommitted_versions_match_the_files(backend, worktree);
    commit_versions_match_the_changes(backend, worktree);
    staging_shows_in_the_changes(backend, worktree);
}

/// Checks `backend` before anything is stored in it. For `ConfigFolder` that's a folder that
/// doesn't exist yet. The checks write `settings.toml`.
pub fn config(backend: &dyn ConfigBackend) {
    nothing_stored_reads_as_empty(backend);
    written_settings_read_back(backend);
}

fn lists_the_main_worktree_first(backend: &dyn GitBackend, worktree: &Path) {
    let repository = backend.repository(worktree).unwrap();
    assert!(
        repository.worktrees.len() > 1,
        "the repository needs a linked worktree"
    );
    let first = &repository.worktrees[0].path;
    assert_eq!(resolved(first), resolved(worktree));
    assert_eq!(repository.root, *first);
    for listed in &repository.worktrees {
        let root = backend.repository(&listed.path).unwrap().root;
        assert_eq!(root, repository.root, "from {}", listed.path.display());
    }
}

fn a_written_file_reads_back(backend: &dyn GitBackend, worktree: &Path) {
    let uncommitted = backend.changes(worktree).unwrap().uncommitted;
    let modified = find(&uncommitted, &[FileStatus::Modified], "a modified file");
    let text = "written by the contract\n";
    backend.write_file(worktree, &modified.path, text).unwrap();
    assert_eq!(
        backend.read_file(worktree, &modified.path).unwrap(),
        Version::Text(text.into())
    );
}

/// An uncommitted file's new version is the file as it is on disk.
fn uncommitted_versions_match_the_files(backend: &dyn GitBackend, worktree: &Path) {
    let uncommitted = backend.changes(worktree).unwrap().uncommitted;
    find(
        &uncommitted,
        &[FileStatus::Added, FileStatus::Untracked],
        "a new file",
    );
    find(&uncommitted, &[FileStatus::Deleted], "a deleted file");
    for file in &uncommitted {
        let versions = checked_versions(backend, worktree, Source::Uncommitted, file);
        let on_disk = backend.read_file(worktree, &file.path).unwrap();
        let path = file.path.display();
        assert_eq!(versions.new, on_disk, "{path}'s new version isn't on disk");
    }
}

fn commit_versions_match_the_changes(backend: &dyn GitBackend, worktree: &Path) {
    let commits = backend.changes(worktree).unwrap().commits;
    let files = || commits.iter().flat_map(|commit| &commit.files);
    find(
        files(),
        &[FileStatus::Renamed],
        "a commit that renames a file",
    );
    find(
        files(),
        &[FileStatus::Deleted],
        "a commit that deletes a file",
    );
    for commit in &commits {
        for file in &commit.files {
            checked_versions(backend, worktree, Source::Commit(commit.sha.clone()), file);
        }
    }
}

/// Staging all of the files, none of them, then one, shows that way when the changes are read.
fn staging_shows_in_the_changes(backend: &dyn GitBackend, worktree: &Path) {
    let set_staged = |files: &[FileChange], staged| {
        let paths = paths(files, staged);
        backend.set_staged(worktree, &paths, staged).unwrap();
        backend.changes(worktree).unwrap().uncommitted
    };
    let uncommitted = backend.changes(worktree).unwrap().uncommitted;

    let uncommitted = set_staged(&uncommitted, true);
    assert_staged(&uncommitted, |_| true);

    let uncommitted = set_staged(&uncommitted, false);
    assert_staged(&uncommitted, |_| false);

    let first = uncommitted.first().expect("a file is uncommitted").clone();
    let uncommitted = set_staged(std::slice::from_ref(&first), true);
    assert_staged(&uncommitted, |file| file.path == first.path);
}

/// Both versions of `file`, checked against its status: a new file has no old version, a deleted
/// one has no new version, and a renamed one's old version is at `renamed_from`, not its new path.
fn checked_versions(
    backend: &dyn GitBackend,
    worktree: &Path,
    source: Source,
    file: &FileChange,
) -> FileVersions {
    let key = FileKey {
        source,
        path: file.path.clone(),
    };
    let versions = backend
        .versions(worktree, &key, file.renamed_from.as_deref())
        .unwrap();
    let path = file.path.display();
    match file.status() {
        FileStatus::Added | FileStatus::Untracked => {
            assert_eq!(versions.old, Version::Missing, "{path} is new");
        }
        FileStatus::Deleted => {
            assert_eq!(versions.new, Version::Missing, "{path} is deleted");
        }
        FileStatus::Renamed => {
            assert!(file.renamed_from.is_some(), "{path} has no old path");
            assert_ne!(
                versions.old,
                Version::Missing,
                "{path} has no old version at its old path"
            );
            let at_new_path = backend.versions(worktree, &key, None).unwrap();
            assert_eq!(
                at_new_path.old,
                Version::Missing,
                "{path} has an old version at its new path"
            );
        }
        FileStatus::Modified | FileStatus::Conflicted => {}
    }
    versions
}

/// Each file is all staged when `staged` says so, and else all unstaged.
fn assert_staged(files: &[FileChange], staged: impl Fn(&FileChange) -> bool) {
    for file in files {
        let expected = if staged(file) {
            Staging::Staged
        } else {
            Staging::Unstaged
        };
        assert_eq!(file.staging(), expected, "{}", file.path.display());
    }
}

fn find<'a>(
    files: impl IntoIterator<Item = &'a FileChange>,
    statuses: &[FileStatus],
    what: &str,
) -> &'a FileChange {
    files
        .into_iter()
        .find(|file| statuses.contains(&file.status()))
        .unwrap_or_else(|| panic!("the worktree needs {what}"))
}

/// What to pass `set_staged` for all of each file. Unstaging a rename takes the path it came from
/// too.
fn paths(files: &[FileChange], staged: bool) -> Vec<PathBuf> {
    files
        .iter()
        .flat_map(|file| {
            let from = file.renamed_from.iter().filter(|_| !staged);
            from.chain([&file.path])
        })
        .cloned()
        .collect()
}

/// `path` with its symlinks resolved, since Git reports a temporary folder on macOS by its real
/// path under `/private`. A fake's paths aren't on disk, so they stay as they are.
fn resolved(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn nothing_stored_reads_as_empty(backend: &dyn ConfigBackend) {
    assert_eq!(backend.read_settings().unwrap(), "");
    assert_eq!(backend.read_themes().unwrap(), []);
}

/// Each write replaces the whole file, down to nothing, and keeps its line endings as written.
fn written_settings_read_back(backend: &dyn ConfigBackend) {
    for text in [
        "density = \"dense\"\ntheme = \"nord\"\n",
        "theme = \"nord\"\n",
        "density = \"dense\"\r\ntheme = \"nord\"\r\n",
        "theme = \"nord\"",
        "",
    ] {
        backend.write_settings(text).unwrap();
        assert_eq!(backend.read_settings().unwrap(), text, "{text:?}");
    }
}

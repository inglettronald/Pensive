//! Checks that a backend keeps the promises its trait makes, so the real backend's tests and the
//! fake's run the same ones.

use std::path::{Path, PathBuf};

use crate::{FileChange, FileKey, FileStatus, FileVersions, GitBackend, Source, Staging, Version};

/// Checks `backend` on the main worktree at `worktree`. Its uncommitted files must include a
/// modified, a new and a deleted file, and a commit on its branch must rename one. The checks
/// write a file and change what's staged.
pub fn git(backend: &dyn GitBackend, worktree: &Path) {
    lists_the_main_worktree_first(backend, worktree);
    a_written_file_reads_back(backend, worktree);
    uncommitted_versions_match_the_files(backend, worktree);
    commit_versions_follow_renames(backend, worktree);
    staging_shows_in_the_changes(backend, worktree);
}

fn lists_the_main_worktree_first(backend: &dyn GitBackend, worktree: &Path) {
    let repository = backend.repository(worktree).unwrap();
    let first = repository.worktrees.first().expect("a worktree is listed");
    assert_eq!(repository.root, first.path);
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
    find(&uncommitted, NEW, "a new file");
    find(&uncommitted, &[FileStatus::Deleted], "a deleted file");
    for file in &uncommitted {
        let key = FileKey {
            source: Source::Uncommitted,
            path: file.path.clone(),
        };
        let versions = backend
            .versions(worktree, &key, file.renamed_from.as_deref())
            .unwrap();
        let on_disk = backend.read_file(worktree, &file.path).unwrap();
        let path = file.path.display();
        assert_eq!(versions.new, on_disk, "{path}'s new version isn't on disk");
        sides_match_the_status(file, &versions);
    }
}

/// A renamed file's old version is read at `renamed_from`, and its new path has none.
fn commit_versions_follow_renames(backend: &dyn GitBackend, worktree: &Path) {
    let commits = backend.changes(worktree).unwrap().commits;
    let mut renamed = 0;
    for commit in &commits {
        for file in &commit.files {
            let key = FileKey {
                source: Source::Commit(commit.sha.clone()),
                path: file.path.clone(),
            };
            let versions = backend
                .versions(worktree, &key, file.renamed_from.as_deref())
                .unwrap();
            sides_match_the_status(file, &versions);
            if file.status() != FileStatus::Renamed {
                continue;
            }
            renamed += 1;
            let path = file.path.display();
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
    }
    assert!(
        renamed > 0,
        "the worktree's branch needs a commit with a rename"
    );
}

/// Staging all of the files, none of them, then one, shows that way when the changes are read.
fn staging_shows_in_the_changes(backend: &dyn GitBackend, worktree: &Path) {
    let set_staged = |files: &[FileChange], staged| {
        backend.set_staged(worktree, &paths(files), staged).unwrap();
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

const NEW: &[FileStatus] = &[FileStatus::Added, FileStatus::Untracked];

fn find<'a>(files: &'a [FileChange], statuses: &[FileStatus], what: &str) -> &'a FileChange {
    files
        .iter()
        .find(|file| statuses.contains(&file.status()))
        .unwrap_or_else(|| panic!("the worktree needs {what}"))
}

/// A new file has no old version, and a deleted one no new version.
fn sides_match_the_status(file: &FileChange, versions: &FileVersions) {
    let path = file.path.display();
    let status = file.status();
    if NEW.contains(&status) {
        assert_eq!(versions.old, Version::Missing, "{path} is new");
    }
    if status == FileStatus::Deleted {
        assert_eq!(versions.new, Version::Missing, "{path} is deleted");
    }
}

/// What to pass `set_staged` for all of each file, including where a rename came from.
fn paths(files: &[FileChange]) -> Vec<PathBuf> {
    files
        .iter()
        .flat_map(|file| file.renamed_from.iter().chain([&file.path]))
        .cloned()
        .collect()
}

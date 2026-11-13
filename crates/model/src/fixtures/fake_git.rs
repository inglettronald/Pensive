//! A `GitBackend` that answers from the sample repositories instead of running Git, and keeps
//! saved files in memory.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::Result;

use crate::{Changes, FileKey, FileVersions, FixtureGap, GitBackend, Repository, Source, Version};

#[derive(Debug)]
pub struct FakeGit {
    repositories: Vec<Repository>,
    /// Files written in pensive's main worktree, by path, over the samples.
    saved: Mutex<HashMap<PathBuf, String>>,
}

impl Default for FakeGit {
    fn default() -> Self {
        Self {
            repositories: super::repositories()
                .into_iter()
                .map(|(repository, _)| repository)
                .collect(),
            saved: Mutex::default(),
        }
    }
}

impl FakeGit {
    fn sample_root(worktree: &Path, path: &Path) -> Result<()> {
        if worktree == Path::new(super::PENSIVE_ROOT) {
            Ok(())
        } else {
            Err(FixtureGap(format!("{} has no sample versions", path.display())).into())
        }
    }
}

impl GitBackend for FakeGit {
    fn repository(&self, path: &Path) -> Result<Repository> {
        self.repositories
            .iter()
            .find(|repository| {
                repository
                    .worktrees
                    .iter()
                    .any(|worktree| worktree.path == path)
            })
            .cloned()
            .ok_or_else(|| {
                FixtureGap(format!("{} is not in a sample repository", path.display())).into()
            })
    }

    /// The sample changes in pensive's main worktree, and none in any other sample worktree.
    fn changes(&self, worktree: &Path) -> Result<Changes> {
        self.repository(worktree)?;
        Ok(if worktree == Path::new(super::PENSIVE_ROOT) {
            super::changes()
        } else {
            Changes::default()
        })
    }

    fn versions(
        &self,
        worktree: &Path,
        key: &FileKey,
        _renamed_from: Option<&Path>,
    ) -> Result<FileVersions> {
        Self::sample_root(worktree, &key.path)?;
        let mut versions = super::changes::versions(key)
            .ok_or_else(|| FixtureGap(format!("{} has no sample versions", key.path.display())))?;
        if key.source == Source::Uncommitted {
            versions.new = self.read_file(worktree, &key.path)?;
        }
        Ok(versions)
    }

    /// A saved file, or else the sample's new version, or else missing.
    fn read_file(&self, worktree: &Path, path: &Path) -> Result<Version> {
        Self::sample_root(worktree, path)?;
        if let Some(text) = self.saved.lock().expect("not poisoned").get(path) {
            return Ok(Version::Text(text.clone()));
        }
        let key = FileKey {
            source: Source::Uncommitted,
            path: path.to_path_buf(),
        };
        Ok(super::changes::versions(&key).map_or(Version::Missing, |versions| versions.new))
    }

    fn write_file(&self, worktree: &Path, path: &Path, text: &str) -> Result<()> {
        Self::sample_root(worktree, path)?;
        self.saved
            .lock()
            .expect("not poisoned")
            .insert(path.to_path_buf(), text.to_owned());
        Ok(())
    }
}

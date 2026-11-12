//! A `GitBackend` that answers from the sample repositories instead of running Git.

use std::path::Path;

use anyhow::Result;

use crate::{Changes, FixtureGap, GitBackend, Repository};

#[derive(Debug)]
pub struct FakeGit {
    repositories: Vec<Repository>,
}

impl Default for FakeGit {
    fn default() -> Self {
        Self {
            repositories: super::repositories()
                .into_iter()
                .map(|(repository, _)| repository)
                .collect(),
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
}

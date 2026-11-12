//! Sample projects for mocks and tests: the ones drawn on the command palette's design canvas.

mod fake_git;

pub use fake_git::FakeGit;

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use crate::{Branch, ProjectId, Projects, Repository, Worktree};

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

pub const PENSIVE: ProjectId = ProjectId(0);
pub const HARBOR: ProjectId = ProjectId(1);
pub const LANTERN: ProjectId = ProjectId(2);
pub const GLYPH_UI: ProjectId = ProjectId(3);
pub const DOTFILES: ProjectId = ProjectId(4);
pub const ZED: ProjectId = ProjectId(5);

/// Six projects with worktrees and branches of every age the palette groups by, read through
/// `FakeGit`.
pub fn projects() -> Projects {
    let mut projects = Projects::new(Arc::new(FakeGit::default()));
    for (repository, opened) in repositories() {
        projects.add(repository, opened);
    }
    projects.as_of = Some(as_of());
    projects
}

fn as_of() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

/// The sample repositories in `ProjectId` order, each with when it was last opened.
fn repositories() -> Vec<(Repository, SystemTime)> {
    let ago = |seconds: u64| as_of() - Duration::from_secs(seconds);
    let worktree = |path: &str, branch: &str, changed| Worktree {
        path: path.into(),
        branch: Some(branch.to_string().into()),
        last_change: ago(changed),
    };
    let branch = |name: &str, committed| Branch {
        name: name.to_string().into(),
        last_commit: ago(committed),
    };
    let repository = |root: &str, worktrees, branches, opened| {
        let repository = Repository {
            root: root.into(),
            worktrees,
            branches,
        };
        (repository, ago(opened))
    };

    vec![
        repository(
            "~/Projects/pensive",
            vec![
                worktree("~/Projects/pensive", "feat/command-palette", 3 * HOUR),
                worktree(
                    "~/Projects/pensive-wt/palette-scopes",
                    "feat/palette-scopes",
                    4 * MINUTE,
                ),
                worktree(
                    "~/Projects/pensive-wt/fuzzy-ranking",
                    "fix/fuzzy-ranking",
                    11 * MINUTE,
                ),
                worktree(
                    "~/Projects/pensive-wt/gutter-icons",
                    "fix/gutter-icons",
                    2 * DAY,
                ),
            ],
            vec![
                branch("feat/command-palette", 3 * HOUR),
                branch("master", 2 * WEEK),
            ],
            2 * MINUTE,
        ),
        repository(
            "~/code/harbor",
            vec![
                worktree("~/code/harbor", "morgan/ui-scaling", 26 * MINUTE),
                worktree(
                    "~/code/harbor-wt/settings-search",
                    "agent/settings-search",
                    HOUR + 5 * MINUTE,
                ),
                worktree(
                    "~/code/harbor-wt/tooltip-clip",
                    "fix/tooltip-clip",
                    5 * HOUR,
                ),
            ],
            vec![
                branch("morgan/ui-scaling", 26 * MINUTE),
                branch("morgan/gui-tooltips", DAY),
                branch("morgan/fix-locale-fallback", 3 * DAY),
                branch("fix/crash-on-resize", WEEK),
                branch("hotfix/1.8.2", 2 * WEEK),
                branch("master", 2 * WEEK),
            ],
            26 * MINUTE,
        ),
        repository(
            "~/code/lantern",
            vec![
                worktree("~/code/lantern", "feat/thumbnail-batch", 3 * DAY),
                worktree(
                    "~/code/lantern-wt/thumbnail-manifest",
                    "agent/thumbnail-manifest",
                    48 * MINUTE,
                ),
            ],
            Vec::new(),
            48 * MINUTE,
        ),
        repository(
            "~/code/glyph-ui",
            vec![
                worktree("~/code/glyph-ui", "main", 4 * WEEK),
                worktree("~/code/glyph-ui-wt/lint-pass", "chore/lint-pass", 2 * HOUR),
            ],
            vec![branch("feat/gpu-instancing", 3 * WEEK)],
            2 * HOUR,
        ),
        repository(
            "~/dotfiles",
            vec![worktree("~/dotfiles", "main", 6 * WEEK)],
            Vec::new(),
            3 * DAY,
        ),
        repository(
            "~/src/zed",
            vec![worktree("~/src/zed", "main", 5 * WEEK)],
            Vec::new(),
            5 * WEEK,
        ),
    ]
}

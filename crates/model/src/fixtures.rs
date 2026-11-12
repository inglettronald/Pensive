//! Sample projects and changes for mocks and tests: the ones drawn on the design canvases.

mod changes;

pub use changes::changes;

use std::time::{Duration, SystemTime};

use crate::{Branch, Project, ProjectId, Projects, Worktree};

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

/// Six projects with worktrees and branches of every age the palette groups by.
pub fn projects() -> Projects {
    let as_of = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let ago = |seconds: u64| as_of - Duration::from_secs(seconds);

    let project = |id, name: &str, root: &str, opened| Project {
        id,
        name: name.to_string().into(),
        root: root.into(),
        last_opened: ago(opened),
    };
    let worktree = |project, path: &str, branch: &str, changed| Worktree {
        project,
        path: path.into(),
        branch: Some(branch.to_string().into()),
        last_change: ago(changed),
    };
    let branch = |project, name: &str, committed| Branch {
        project,
        name: name.to_string().into(),
        last_commit: ago(committed),
    };

    Projects {
        projects: vec![
            project(PENSIVE, "pensive", "~/Projects/pensive", 2 * MINUTE),
            project(HARBOR, "harbor", "~/code/harbor", 26 * MINUTE),
            project(LANTERN, "lantern", "~/code/lantern", 48 * MINUTE),
            project(GLYPH_UI, "glyph-ui", "~/code/glyph-ui", 2 * HOUR),
            project(DOTFILES, "dotfiles", "~/dotfiles", 3 * DAY),
            project(ZED, "zed", "~/src/zed", 5 * WEEK),
        ],
        worktrees: vec![
            worktree(
                PENSIVE,
                "~/Projects/pensive",
                "feat/command-palette",
                3 * HOUR,
            ),
            worktree(
                PENSIVE,
                "~/Projects/pensive-wt/palette-scopes",
                "feat/palette-scopes",
                4 * MINUTE,
            ),
            worktree(
                PENSIVE,
                "~/Projects/pensive-wt/fuzzy-ranking",
                "fix/fuzzy-ranking",
                11 * MINUTE,
            ),
            worktree(
                PENSIVE,
                "~/Projects/pensive-wt/gutter-icons",
                "fix/gutter-icons",
                2 * DAY,
            ),
            worktree(HARBOR, "~/code/harbor", "morgan/ui-scaling", 26 * MINUTE),
            worktree(
                HARBOR,
                "~/code/harbor-wt/settings-search",
                "agent/settings-search",
                HOUR + 5 * MINUTE,
            ),
            worktree(
                HARBOR,
                "~/code/harbor-wt/tooltip-clip",
                "fix/tooltip-clip",
                5 * HOUR,
            ),
            worktree(LANTERN, "~/code/lantern", "feat/thumbnail-batch", 3 * DAY),
            worktree(
                LANTERN,
                "~/code/lantern-wt/thumbnail-manifest",
                "agent/thumbnail-manifest",
                48 * MINUTE,
            ),
            worktree(GLYPH_UI, "~/code/glyph-ui", "main", 4 * WEEK),
            worktree(
                GLYPH_UI,
                "~/code/glyph-ui-wt/lint-pass",
                "chore/lint-pass",
                2 * HOUR,
            ),
            worktree(DOTFILES, "~/dotfiles", "main", 6 * WEEK),
            worktree(ZED, "~/src/zed", "main", 5 * WEEK),
        ],
        branches: vec![
            branch(PENSIVE, "feat/command-palette", 3 * HOUR),
            branch(PENSIVE, "master", 2 * WEEK),
            branch(HARBOR, "morgan/ui-scaling", 26 * MINUTE),
            branch(HARBOR, "morgan/gui-tooltips", DAY),
            branch(HARBOR, "morgan/fix-locale-fallback", 3 * DAY),
            branch(HARBOR, "fix/crash-on-resize", WEEK),
            branch(HARBOR, "hotfix/1.8.2", 2 * WEEK),
            branch(HARBOR, "master", 2 * WEEK),
            branch(GLYPH_UI, "feat/gpu-instancing", 3 * WEEK),
        ],
        as_of: Some(as_of),
    }
}

//! Sample projects for mocks and tests: the ones drawn on the command palette's design canvas.

use std::time::{Duration, SystemTime};

use crate::{Branch, Project, ProjectId, Projects, Worktree};

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

pub const PENSIVE: ProjectId = ProjectId(0);
pub const MULTIVER: ProjectId = ProjectId(1);
pub const CDN_ASSETS: ProjectId = ProjectId(2);
pub const GAMEUI: ProjectId = ProjectId(3);
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
            project(MULTIVER, "multiver", "~/work/multiver", 26 * MINUTE),
            project(CDN_ASSETS, "cdn-assets", "~/work/cdn-assets", 48 * MINUTE),
            project(GAMEUI, "gameui", "~/work/gameui", 2 * HOUR),
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
            worktree(
                MULTIVER,
                "~/work/multiver",
                "dulkir/hud-scaling",
                26 * MINUTE,
            ),
            worktree(
                MULTIVER,
                "~/work/multiver-wt/settings-search",
                "agent/settings-search",
                HOUR + 5 * MINUTE,
            ),
            worktree(
                MULTIVER,
                "~/work/multiver-wt/tooltip-clip",
                "fix/tooltip-clip",
                5 * HOUR,
            ),
            worktree(
                CDN_ASSETS,
                "~/work/cdn-assets",
                "feat/cosmetics-batch",
                3 * DAY,
            ),
            worktree(
                CDN_ASSETS,
                "~/work/cdn-assets-wt/cosmetics-manifest",
                "agent/cosmetics-manifest",
                48 * MINUTE,
            ),
            worktree(GAMEUI, "~/work/gameui", "main", 4 * WEEK),
            worktree(
                GAMEUI,
                "~/work/gameui-wt/lint-pass",
                "chore/lint-pass",
                2 * HOUR,
            ),
            worktree(DOTFILES, "~/dotfiles", "main", 6 * WEEK),
            worktree(ZED, "~/src/zed", "main", 5 * WEEK),
        ],
        branches: vec![
            branch(PENSIVE, "feat/command-palette", 3 * HOUR),
            branch(PENSIVE, "master", 2 * WEEK),
            branch(MULTIVER, "dulkir/hud-scaling", 26 * MINUTE),
            branch(MULTIVER, "dulkir/gui-tooltips", DAY),
            branch(MULTIVER, "dulkir/fix-locale-fallback", 3 * DAY),
            branch(MULTIVER, "fix/crash-on-resize", WEEK),
            branch(MULTIVER, "hotfix/1.8.2", 2 * WEEK),
            branch(MULTIVER, "master", 2 * WEEK),
            branch(GAMEUI, "feat/gpu-instancing", 3 * WEEK),
        ],
        as_of: Some(as_of),
    }
}

//! Pensive's own commands, as the palette lists them.

use components::IconName;
use gpui::SharedString;

use super::{Action, Keyword};

pub(crate) struct Command {
    pub icon: IconName,
    pub name: SharedString,
    pub kbd: Option<&'static str>,
    pub group: &'static str,
    pub action: Action,
}

/// Where a command shows when the field is empty. Settings shows nowhere until it's searched for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Listed {
    /// On the landing screen, under Start.
    Start,
    /// In a project window, under the projects.
    Project,
    /// Only in the Commands tab.
    Tab,
    /// Only when the query matches it.
    Searched,
}

/// The commands available with this project open, or on the landing screen without one. Most
/// run nothing yet: each feature wires up its command as it lands.
pub(crate) fn commands(current_branch: Option<&SharedString>) -> Vec<(Listed, Command)> {
    let command = |icon, name: &str, kbd, group, action| Command {
        icon,
        name: name.to_string().into(),
        kbd,
        group,
        action,
    };
    let mut commands = Vec::new();
    if let Some(branch) = current_branch {
        commands.extend([
            (
                Listed::Project,
                command(
                    IconName::GitBranch,
                    "Switch branch…",
                    Some("Ctrl B"),
                    "Git",
                    Action::Narrow(Keyword::Branch),
                ),
            ),
            (
                Listed::Project,
                command(
                    IconName::GitMerge,
                    &format!("Merge into {branch}…"),
                    None,
                    "Git",
                    Action::None,
                ),
            ),
            (
                Listed::Tab,
                command(
                    IconName::GitCommitHorizontal,
                    "Commit…",
                    None,
                    "Git",
                    Action::None,
                ),
            ),
            (
                Listed::Tab,
                command(IconName::RefreshCw, "Fetch", None, "Git", Action::None),
            ),
            (
                Listed::Tab,
                command(IconName::ArrowDownToLine, "Pull", None, "Git", Action::None),
            ),
            (
                Listed::Tab,
                command(IconName::ArrowUpFromLine, "Push", None, "Git", Action::None),
            ),
            (
                Listed::Tab,
                command(
                    IconName::Archive,
                    "Stash changes",
                    None,
                    "Git",
                    Action::None,
                ),
            ),
            (
                Listed::Tab,
                command(
                    IconName::FolderGit2,
                    "New worktree…",
                    None,
                    "Git",
                    Action::None,
                ),
            ),
        ]);
    }
    let start = if current_branch.is_some() {
        Listed::Tab
    } else {
        Listed::Start
    };
    commands.extend([
        (
            start,
            command(
                IconName::Folder,
                "Open folder…",
                Some("Ctrl O"),
                "Projects",
                Action::None,
            ),
        ),
        (
            if current_branch.is_some() {
                Listed::Project
            } else {
                Listed::Start
            },
            command(
                IconName::Download,
                "Clone repository…",
                Some("Ctrl Shift O"),
                "Projects",
                Action::None,
            ),
        ),
        (
            start,
            command(
                IconName::Plus,
                "New repository…",
                None,
                "Projects",
                Action::None,
            ),
        ),
        (
            Listed::Searched,
            command(
                IconName::Settings,
                "Settings",
                Some("Ctrl ,"),
                "Pensive",
                Action::None,
            ),
        ),
    ]);
    commands
}

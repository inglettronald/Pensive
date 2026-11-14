//! Turns projects, settings and work into palette rows, and ranks them against the query.

use std::ops::Range;

use components::{IconName, KeyHint, Leading, PaletteEntry, TrailingText};
use gpui::SharedString;
use model::{Project, Work, WorkKind};
use settings::ControlState;

use crate::{age::age, fuzzy::Fuzzy, setting_control::setting_control};

use super::{
    Action, Keyword,
    commands::Command,
    results::{Item, Query, SettingRow},
};

/// Something the query can match, turned into a row once its matches are known.
pub(super) enum Candidate<'a> {
    Project(&'a Project),
    /// Work, with its project name as the detail.
    WorkInProject(Work),
    /// Work, with its worktree path as the detail.
    WorkAtPath(Work),
    Command(Command),
    Setting(&'a SettingRow),
}

impl Candidate<'_> {
    fn label(&self) -> &str {
        match self {
            Self::Project(project) => &project.name,
            Self::WorkInProject(work) | Self::WorkAtPath(work) => &work.name,
            Self::Command(command) => &command.name,
            Self::Setting(row) => row.setting.name(),
        }
    }

    fn item(self, query: &Query, matches: Vec<Range<usize>>) -> Item {
        match self {
            Self::Project(project) => project_item(query, project, matches),
            Self::WorkInProject(work) => {
                let project = query
                    .projects
                    .project(work.project)
                    .map(|project| project.name.clone());
                work_item(query, work, project, matches)
            }
            Self::WorkAtPath(work) => {
                let path = match &work.kind {
                    WorkKind::Linked(path) => Some(path.display().to_string().into()),
                    WorkKind::Main | WorkKind::Branch => None,
                };
                work_item(query, work, path, matches)
            }
            Self::Command(command) => {
                let mut entry = PaletteEntry::new(command.name)
                    .icon(command.icon)
                    .matches(matches);
                if let Some(kbd) = command.kbd {
                    entry = entry.kbd(kbd);
                }
                Item {
                    entry,
                    action: command.action,
                    secondary: Action::None,
                    narrow: None,
                }
            }
            Self::Setting(row) => setting_item(row, matches),
        }
    }
}

/// Every candidate in order when the query is empty, otherwise the matches, best first.
pub(super) fn rank<'a>(
    query: &Query,
    candidates: impl IntoIterator<Item = Candidate<'a>>,
) -> Vec<Item> {
    if query.text.is_empty() {
        return candidates
            .into_iter()
            .map(|candidate| candidate.item(query, Vec::new()))
            .collect();
    }
    let mut fuzzy = Fuzzy::new(query.text);
    let mut matched: Vec<_> = candidates
        .into_iter()
        .filter_map(|candidate| {
            let (score, ranges) = fuzzy.score(candidate.label())?;
            Some((score, candidate, ranges))
        })
        .collect();
    matched.sort_by_key(|(score, _, _)| std::cmp::Reverse(*score));
    matched
        .into_iter()
        .map(|(_, candidate, ranges)| candidate.item(query, ranges))
        .collect()
}

fn open_hints(verb: &str) -> [KeyHint; 2] {
    [
        KeyHint::new("Enter", verb.to_string()),
        KeyHint::new("Ctrl Enter", "New window"),
    ]
}

pub(super) fn project_item(query: &Query, project: &Project, matches: Vec<Range<usize>>) -> Item {
    let when = if query.current == Some(project.id) {
        "This window".into()
    } else {
        age(query.projects.now(), project.last_opened)
    };
    let mut entry = PaletteEntry::new(project.name.clone())
        .leading(Leading::Tile(project.initial()))
        .matches(matches)
        .detail(project.repository.root.display().to_string())
        .when(when)
        .hints(open_hints("Open"));
    if let Some(branch) = query.projects.current_branch(project.id) {
        entry = entry.trailing(
            TrailingText::new(branch.clone())
                .code()
                .color(|colors| colors.text_muted),
        );
    }
    Item {
        entry,
        action: Action::Open {
            project: project.id,
            new_window: false,
        },
        secondary: Action::Open {
            project: project.id,
            new_window: true,
        },
        narrow: Some(Keyword::Project(Some(project.id))),
    }
}

/// A setting with its control. Clicking the control changes the setting too.
fn setting_item(row: &SettingRow, matches: Vec<Range<usize>>) -> Item {
    let setting = row.setting;
    let id = setting.key();
    let control = setting_control(setting, &row.state);
    let hint = match &row.state {
        ControlState::Toggle(_) => KeyHint::new("Enter", "Toggle"),
        _ => KeyHint::new("←→", "Change"),
    };
    Item {
        entry: PaletteEntry::new(setting.name())
            .description(setting.description())
            .matches(matches)
            .control(control)
            .hints([hint]),
        action: Action::Step(id),
        secondary: Action::None,
        narrow: None,
    }
}

/// Switching to work opens its project. Checking out the branch waits on the Git backend.
fn work_item(
    query: &Query,
    work: Work,
    detail: Option<SharedString>,
    matches: Vec<Range<usize>>,
) -> Item {
    let icon = match work.kind {
        WorkKind::Linked(_) => IconName::FolderGit2,
        WorkKind::Main | WorkKind::Branch => IconName::GitBranch,
    };
    let mut entry = PaletteEntry::new(work.name.clone())
        .icon(icon)
        .code()
        .matches(matches)
        .when(age(query.projects.now(), work.last_change))
        .hints(open_hints("Switch"));
    if let Some(detail) = detail {
        entry = entry.detail(detail);
    }
    Item {
        entry,
        action: Action::Open {
            project: work.project,
            new_window: false,
        },
        secondary: Action::Open {
            project: work.project,
            new_window: true,
        },
        narrow: None,
    }
}

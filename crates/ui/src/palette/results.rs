//! What the palette lists for a query: the sections, their rows, and what each row does.

use std::{ops::Range, time::Duration};

use components::{
    IconName, KeyHint, Leading, PaletteEntry, SegmentedControl, Select, Toggle, TrailingText,
};
use gpui::{IntoElement, SharedString};
use model::{Project, ProjectId, Projects, Work, WorkKind};
use settings::{AnySetting, ControlState};

use crate::age::age;

use super::{
    Action, Keyword, Tab,
    commands::{Command, Listed, commands},
    fuzzy::Fuzzy,
    options::days_label,
};

const BEST_MATCHES: usize = 6;
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// Everything the results depend on.
pub(crate) struct Query<'a> {
    pub projects: &'a Projects,
    /// The project open in this window, if any.
    pub current: Option<ProjectId>,
    pub tab: Tab,
    pub keyword: Option<&'a Keyword>,
    pub text: &'a str,
    pub settings: &'a [SettingRow],
    /// How far back the Active tab looks.
    pub active_days: u32,
}

/// A setting, the section it's listed under, and what its control shows now.
pub(crate) struct SettingRow {
    pub section: &'static str,
    pub setting: &'static dyn AnySetting,
    pub state: ControlState,
}

pub(crate) struct Item {
    pub entry: PaletteEntry,
    pub action: Action,
    /// What Ctrl Enter does.
    pub secondary: Action,
    /// The keyword → applies.
    pub narrow: Option<Keyword>,
}

pub(crate) struct Section {
    pub label: SharedString,
    pub aside: Option<SharedString>,
    pub items: Vec<Item>,
}

#[derive(Default)]
pub(crate) struct Results {
    pub empty: Option<(SharedString, SharedString)>,
    pub sections: Vec<Section>,
}

impl Results {
    pub fn into_items(self) -> impl Iterator<Item = Item> {
        self.sections.into_iter().flat_map(|section| section.items)
    }
}

pub(crate) fn results(query: &Query) -> Results {
    match (query.keyword, query.tab) {
        (Some(Keyword::Project(Some(id))), _) => in_project(query, *id),
        (Some(Keyword::Project(None)), _) => one_section(
            "Projects",
            rank(
                query,
                query.projects.recent().into_iter().map(Candidate::Project),
            ),
        ),
        (Some(Keyword::Setting), _) => setting_results(query),
        (Some(Keyword::Branch), _) => one_section(
            "Branches and worktrees",
            rank(
                query,
                query
                    .projects
                    .work()
                    .into_iter()
                    .map(Candidate::WorkInProject),
            ),
        ),
        (None, Tab::All) if query.text.is_empty() => all_empty(query),
        (None, Tab::All) => match clone_url(query.text) {
            Some(repo) => clone(query, repo),
            None => all_search(query),
        },
        (None, Tab::Active) => active(query),
        (None, Tab::Commands) => commands_tab(query),
    }
}

fn one_section(label: &str, items: Vec<Item>) -> Results {
    Results {
        empty: None,
        sections: section(label, items).into_iter().collect(),
    }
}

/// A section, or nothing when it has no rows.
fn section(label: &str, items: Vec<Item>) -> Option<Section> {
    (!items.is_empty()).then(|| Section {
        label: label.to_string().into(),
        aside: None,
        items,
    })
}

fn all_empty(query: &Query) -> Results {
    let listed = |wanted| {
        available_commands(query)
            .into_iter()
            .filter(move |(listed, _)| *listed == wanted)
            .map(|(_, command)| Candidate::Command(command))
    };
    let sections = match query.current {
        None => vec![
            section(
                "Recent",
                rank(
                    query,
                    query.projects.recent().into_iter().map(Candidate::Project),
                ),
            ),
            section("Start", rank(query, listed(Listed::Start))),
        ],
        Some(current) => {
            let mut projects = query.projects.recent();
            projects.sort_by_key(|project| project.id == current);
            vec![
                section(
                    "Projects",
                    rank(query, projects.into_iter().map(Candidate::Project)),
                ),
                section("Commands", rank(query, listed(Listed::Project))),
            ]
        }
    };
    Results {
        empty: None,
        sections: sections.into_iter().flatten().collect(),
    }
}

fn all_search(query: &Query) -> Results {
    let candidates = query
        .projects
        .recent()
        .into_iter()
        .map(Candidate::Project)
        .chain(
            query
                .projects
                .work()
                .into_iter()
                .map(Candidate::WorkInProject),
        )
        .chain(
            available_commands(query)
                .into_iter()
                .map(|(_, command)| Candidate::Command(command)),
        )
        .chain(query.settings.iter().map(Candidate::Setting));
    let mut best = rank(query, candidates);
    best.truncate(BEST_MATCHES);
    let narrow = |keyword: Keyword, name: &str, detail: &str| Item {
        entry: PaletteEntry::new(format!("{name} {}", query.text))
            .icon(IconName::Search)
            .code()
            .detail(detail.to_string()),
        action: Action::Narrow(keyword),
        secondary: Action::None,
        narrow: None,
    };
    let narrow_to = vec![
        narrow(Keyword::Branch, "branch:", "Branches and worktrees"),
        narrow(Keyword::Project(None), "project:", "Projects"),
        narrow(Keyword::Setting, "setting:", "Settings"),
    ];
    Results {
        empty: best.is_empty().then(|| {
            (
                format!("Nothing matches \"{}\"", query.text).into(),
                "Searched projects, branches, worktrees, commands and settings".into(),
            )
        }),
        sections: [
            section("Best matches", best),
            section("Narrow to", narrow_to),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

fn in_project(query: &Query, id: ProjectId) -> Results {
    let name = query
        .projects
        .project(id)
        .map(|project| project.name.clone())
        .unwrap_or_default();
    let work = query
        .projects
        .work()
        .into_iter()
        .filter(|work| work.project == id)
        .map(Candidate::WorkAtPath);
    let matches = rank(query, work);
    let create = (!query.text.is_empty()).then(|| Item {
        entry: PaletteEntry::new(format!("Create branch {} in {name}", query.text))
            .icon(IconName::Plus)
            .kbd("Ctrl N"),
        action: Action::None,
        secondary: Action::None,
        narrow: None,
    });

    if matches.is_empty() && !query.text.is_empty() {
        let search_everywhere = Item {
            entry: PaletteEntry::new(format!("Search every project for \"{}\"", query.text))
                .icon(IconName::Search)
                .hints([KeyHint::new("Enter", "Search")]),
            action: Action::ClearKeyword,
            secondary: Action::None,
            narrow: None,
        };
        return Results {
            empty: Some((
                format!("Nothing in {name} matches \"{}\"", query.text).into(),
                "Searched branches and worktrees".into(),
            )),
            sections: section(
                "Try",
                [search_everywhere].into_iter().chain(create).collect(),
            )
            .into_iter()
            .collect(),
        };
    }
    Results {
        empty: None,
        sections: [
            section(&format!("Branches and worktrees in {name}"), matches),
            section("Commands", create.into_iter().collect()),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

fn setting_results(query: &Query) -> Results {
    let mut sections: Vec<(&str, Vec<Candidate>)> = Vec::new();
    for row in query.settings {
        let section = row.section;
        match sections.iter_mut().find(|(label, _)| *label == section) {
            Some((_, rows)) => rows.push(Candidate::Setting(row)),
            None => sections.push((section, vec![Candidate::Setting(row)])),
        }
    }
    let sections: Vec<Section> = sections
        .into_iter()
        .filter_map(|(label, rows)| section(label, rank(query, rows)))
        .collect();
    Results {
        empty: sections.is_empty().then(|| {
            (
                format!("No setting matches \"{}\"", query.text).into(),
                "Searched every setting".into(),
            )
        }),
        sections,
    }
}

/// "this week" for the default of 7 days.
fn within(days: u32) -> String {
    match days {
        1 => "today".into(),
        7 => "this week".into(),
        days => format!("in the last {}", days_label(days)),
    }
}

fn active(query: &Query) -> Results {
    let days = query.active_days;
    let work = query.projects.active(DAY * days);
    let total = work.len();
    let projects = {
        let mut ids: Vec<_> = work.iter().map(|work| work.project).collect();
        ids.sort_by_key(|id| id.0);
        ids.dedup();
        ids.len()
    };
    let age = |work: &Work| {
        query
            .projects
            .as_of
            .and_then(|as_of| as_of.duration_since(work.last_change).ok())
            .unwrap_or_default()
    };
    // Active keeps work up to `days` old, so Earlier only shows when that's longer than a week.
    let group = |work: &Work| match age(work) {
        age if age < Duration::from_secs(60 * 60) => 0,
        age if age < DAY => 1,
        age if age < DAY * 7 || days <= 7 => 2,
        _ => 3,
    };
    let mut sections: Vec<Section> = ["Last hour", "Today", "This week", "Earlier"]
        .into_iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let work = work
                .iter()
                .filter(|work| group(work) == index)
                .cloned()
                .map(Candidate::WorkInProject);
            section(label, rank(query, work))
        })
        .collect();
    if let Some(first) = sections.first_mut().filter(|_| query.text.is_empty()) {
        first.aside = Some(format!("{total} across {projects} projects").into());
    }
    Results {
        empty: sections.is_empty().then(|| {
            if query.text.is_empty() {
                (
                    format!("Nothing changed {}", within(days)).into(),
                    "Worktrees and branches show here when they change".into(),
                )
            } else {
                (
                    format!("No recent work matches \"{}\"", query.text).into(),
                    format!("Searched work from the last {}", days_label(days)).into(),
                )
            }
        }),
        sections,
    }
}

fn commands_tab(query: &Query) -> Results {
    let mut groups: Vec<(&str, Vec<Candidate>)> = Vec::new();
    for (listed, command) in available_commands(query) {
        if listed == Listed::Searched && query.text.is_empty() {
            continue;
        }
        match groups.iter_mut().find(|(group, _)| *group == command.group) {
            Some((_, commands)) => commands.push(Candidate::Command(command)),
            None => groups.push((command.group, vec![Candidate::Command(command)])),
        }
    }
    Results {
        empty: None,
        sections: groups
            .into_iter()
            .filter_map(|(group, commands)| section(group, rank(query, commands)))
            .collect(),
    }
}

fn clone(query: &Query, repo: Repo) -> Results {
    let clone = Item {
        entry: PaletteEntry::new(format!("Clone {}", repo.path))
            .icon(IconName::Download)
            .detail(format!("into ~/Projects/{}", repo.name))
            .hints([
                KeyHint::new("Enter", "Clone"),
                KeyHint::new("Ctrl Enter", "Clone into…"),
            ]),
        action: Action::None,
        secondary: Action::None,
        narrow: None,
    };
    let on_disk = query
        .projects
        .recent()
        .into_iter()
        .filter(|project| project.name == repo.name)
        .map(|project| project_item(query, project, Vec::new()))
        .collect();
    Results {
        empty: None,
        sections: [
            section("Clone", vec![clone]),
            section("Already on disk", on_disk),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

fn available_commands(query: &Query) -> Vec<(Listed, Command)> {
    commands(
        query
            .current
            .and_then(|current| query.projects.current_branch(current)),
    )
}

/// Something the query can match, turned into a row once its matches are known.
enum Candidate<'a> {
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
fn rank<'a>(query: &Query, candidates: impl IntoIterator<Item = Candidate<'a>>) -> Vec<Item> {
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

fn project_item(query: &Query, project: &Project, matches: Vec<Range<usize>>) -> Item {
    let when = if query.current == Some(project.id) {
        "This window".into()
    } else {
        age(query.projects.as_of, project.last_opened)
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
    let change = KeyHint::new("←→", "Change");
    let (control, hint) = match &row.state {
        ControlState::Toggle(on) => (
            Toggle::new(id, *on)
                .on_change(move |_, _, cx| setting.step(1, cx))
                .into_any_element(),
            KeyHint::new("Enter", "Toggle"),
        ),
        ControlState::Segmented { labels, selected } => (
            SegmentedControl::new(id, labels.clone(), *selected)
                .on_select(move |index, _, cx| setting.choose(index, cx))
                .into_any_element(),
            change,
        ),
        ControlState::Select { label } => (
            Select::new(id, label.clone())
                .on_click(move |_, cx| setting.step(1, cx))
                .into_any_element(),
            change,
        ),
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
        .when(age(query.projects.as_of, work.last_change))
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

/// A repository named by a pasted clone URL.
struct Repo {
    /// `owner/name`
    path: String,
    name: String,
}

fn clone_url(text: &str) -> Option<Repo> {
    let text = text.trim();
    let rest = ["https://", "http://", "ssh://"]
        .iter()
        .find_map(|scheme| text.strip_prefix(scheme))
        .and_then(|rest| rest.split_once('/').map(|(_, path)| path))
        .or_else(|| {
            text.strip_prefix("git@")
                .and_then(|rest| rest.split_once(':').map(|(_, path)| path))
        })?;
    let path = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut segments = path.rsplit('/');
    let name = segments.next().filter(|name| !name.is_empty())?;
    let owner = segments.next().filter(|owner| !owner.is_empty())?;
    Some(Repo {
        path: format!("{owner}/{name}"),
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use model::fixtures::{self, HARBOR, PENSIVE};

    use super::*;
    use crate::palette::options::{ACTIVE_DAYS, SCRIM, SHOW_TABS};

    fn labels(results: Results) -> Vec<String> {
        results
            .sections
            .into_iter()
            .map(|section| format!("{} ({})", section.label, section.items.len()))
            .collect()
    }

    fn query<'a>(
        projects: &'a Projects,
        tab: Tab,
        keyword: Option<&'a Keyword>,
        text: &'a str,
    ) -> Query<'a> {
        Query {
            projects,
            current: Some(PENSIVE),
            tab,
            keyword,
            text,
            settings: &[],
            active_days: 7,
        }
    }

    #[test]
    fn active_groups_recent_work_by_age() {
        let projects = fixtures::projects();
        let results = results(&query(&projects, Tab::Active, None, ""));
        assert_eq!(
            results.sections[0].aside.as_deref(),
            Some("13 across 4 projects")
        );
        assert_eq!(
            labels(results),
            ["Last hour (4)", "Today (4)", "This week (5)"]
        );
    }

    #[test]
    fn the_setting_keyword_lists_settings_under_their_sections() {
        let projects = fixtures::projects();
        let rows: Vec<SettingRow> = [&SHOW_TABS as &dyn AnySetting, &SCRIM, &ACTIVE_DAYS]
            .into_iter()
            .map(|setting| SettingRow {
                section: "Palette",
                setting,
                state: ControlState::Toggle(true),
            })
            .collect();
        let keyword = Keyword::Setting;
        let search = |text| Query {
            settings: &rows,
            ..query(&projects, Tab::All, Some(&keyword), text)
        };
        assert_eq!(labels(results(&search(""))), ["Palette (3)"]);
        assert_eq!(labels(results(&search("scrim"))), ["Palette (1)"]);
        let first = results(&search("scrim")).into_items().next().unwrap();
        assert_eq!(first.action, Action::Step("palette.scrim"));
    }

    #[test]
    fn active_only_looks_back_as_far_as_the_setting_says() {
        let projects = fixtures::projects();
        let three_days = Query {
            active_days: 3,
            ..query(&projects, Tab::Active, None, "")
        };
        assert_eq!(
            labels(results(&three_days)),
            ["Last hour (4)", "Today (4)", "This week (4)"]
        );
    }

    #[test]
    fn settings_only_shows_when_searched_for() {
        let projects = fixtures::projects();
        let commands = |text| labels(results(&query(&projects, Tab::Commands, None, text)));
        assert!(!commands("").contains(&"Pensive (1)".to_string()));
        assert_eq!(commands("sett"), ["Pensive (1)"]);
    }

    #[test]
    fn a_project_keyword_lists_only_that_projects_work() {
        let projects = fixtures::projects();
        let keyword = Keyword::Project(Some(HARBOR));
        let results = results(&query(&projects, Tab::All, Some(&keyword), "fix"));
        assert_eq!(
            labels(results),
            ["Branches and worktrees in harbor (4)", "Commands (1)"]
        );
    }

    #[test]
    fn nothing_matching_in_a_project_offers_to_search_everywhere() {
        let projects = fixtures::projects();
        let keyword = Keyword::Project(Some(HARBOR));
        let results = results(&query(&projects, Tab::All, Some(&keyword), "qwerty"));
        assert!(results.empty.is_some());
        let first = results.into_items().next().unwrap();
        assert_eq!(first.action, Action::ClearKeyword);
    }

    #[test]
    fn clone_urls_name_their_repository() {
        for url in [
            "https://github.com/zed-industries/zed.git",
            "https://github.com/zed-industries/zed",
            "git@github.com:zed-industries/zed.git",
        ] {
            let repo = clone_url(url).unwrap();
            assert_eq!(repo.path, "zed-industries/zed");
            assert_eq!(repo.name, "zed");
        }
        assert!(clone_url("zed").is_none());
    }
}

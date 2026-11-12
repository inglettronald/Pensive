//! What the palette lists for a query: the sections, their rows, and what each row does.

use std::{
    ops::Range,
    time::{Duration, SystemTime},
};

use components::{IconName, KeyHint, Leading, PaletteEntry, TrailingText};
use gpui::SharedString;
use model::{Project, ProjectId, Projects, Work, WorkKind};

use super::{
    Action, Keyword, Tab,
    commands::{Command, Listed, commands},
    fuzzy::Fuzzy,
};

/// How far back the Active tab looks. Becomes a setting once settings exist.
const ACTIVE_WINDOW: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const BEST_MATCHES: usize = 6;

/// Everything the results depend on.
pub(crate) struct Query<'a> {
    pub projects: &'a Projects,
    /// The project open in this window, if any.
    pub current: Option<ProjectId>,
    pub tab: Tab,
    pub keyword: Option<&'a Keyword>,
    pub text: &'a str,
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
        );
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
    ];
    Results {
        empty: best.is_empty().then(|| {
            (
                format!("Nothing matches \"{}\"", query.text).into(),
                "Searched projects, branches, worktrees and commands".into(),
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

fn active(query: &Query) -> Results {
    let work = query.projects.active(ACTIVE_WINDOW);
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
    let group = |work: &Work| match age(work).as_secs() {
        age if age < 60 * 60 => 0,
        age if age < 24 * 60 * 60 => 1,
        _ => 2,
    };
    let mut sections: Vec<Section> = ["Last hour", "Today", "This week"]
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
                    "Nothing changed this week".into(),
                    "Worktrees and branches show here when they change".into(),
                )
            } else {
                (
                    format!("No recent work matches \"{}\"", query.text).into(),
                    "Searched work from the last 7 days".into(),
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
}

impl Candidate<'_> {
    fn label(&self) -> &str {
        match self {
            Self::Project(project) => &project.name,
            Self::WorkInProject(work) | Self::WorkAtPath(work) => &work.name,
            Self::Command(command) => &command.name,
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
        .detail(project.root.display().to_string())
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

/// How long before `as_of` something happened, as the palette prints it: `4m`, `2h`, `3d`, `5w`.
fn age(as_of: Option<SystemTime>, time: SystemTime) -> SharedString {
    let Some(seconds) = as_of
        .and_then(|as_of| as_of.duration_since(time).ok())
        .map(|age| age.as_secs())
    else {
        return SharedString::default();
    };
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    let text = match seconds {
        s if s < HOUR => format!("{}m", (s / MINUTE).max(1)),
        s if s < DAY => format!("{}h", s / HOUR),
        s if s < WEEK => format!("{}d", s / DAY),
        s if s < 52 * WEEK => format!("{}w", s / WEEK),
        s => format!("{}y", s / (52 * WEEK)),
    };
    text.into()
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

    #[test]
    fn ages_use_the_largest_whole_unit() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000_000);
        let ago = |seconds| age(Some(now), now - Duration::from_secs(seconds));
        assert_eq!(ago(30), "1m");
        assert_eq!(ago(26 * 60), "26m");
        assert_eq!(ago(3 * 60 * 60), "3h");
        assert_eq!(ago(3 * 24 * 60 * 60), "3d");
        assert_eq!(ago(5 * 7 * 24 * 60 * 60), "5w");
    }
}

//! Sample changes for the pensive project: the files and commits drawn on the app shell canvas.
//!
//! Line counts match the canvas. Each file's versions are an excerpt, so a file can count more
//! lines than its diff shows.

use std::path::Path;

use gpui::SharedString;

use crate::{
    Base, Changes, Commit, FileChange, FileKey, FileStatus, FileVersions, Source, Staging, Symbol,
    SymbolKind, Upstream, Version,
};

/// Seven uncommitted files, two staged and one partly staged, then two unpushed commits and one
/// pushed.
pub fn changes() -> Changes {
    let changes = |samples: Vec<Sample>| samples.into_iter().map(|sample| sample.change).collect();
    Changes {
        base: Some(Base {
            branch: "master".into(),
        }),
        upstream: Some(Upstream {
            remote: "origin".into(),
            behind: 1,
        }),
        uncommitted: changes(uncommitted()),
        commits: vec![
            Commit {
                sha: OPEN_PALETTE.into(),
                title: "feat: open the command palette with Ctrl K".into(),
                description: Some(
                    "On the landing screen the palette fills the page, so the same keys work \
                     before and after a project is open."
                        .into(),
                ),
                pushed: false,
                files: changes(open_palette()),
            },
            Commit {
                sha: ADD_PROJECTS.into(),
                title: "feat: add projects, worktrees and branches".into(),
                description: Some("Projects are cached, so reading them never runs Git.".into()),
                pushed: false,
                files: changes(add_projects()),
            },
            Commit {
                sha: "ef032a6".into(),
                title: "feat: add CommandPalette component".into(),
                description: None,
                pushed: true,
                files: Vec::new(),
            },
        ],
    }
}

/// Both versions of the sample file `key` names. As in Git, the old version is the one at
/// `renamed_from` when that's given, so a rename read without it has no old version.
pub(crate) fn versions(key: &FileKey, renamed_from: Option<&Path>) -> Option<FileVersions> {
    let samples = samples(&key.source);
    let old_path = renamed_from.unwrap_or(&key.path);
    let old = samples
        .iter()
        .find(|sample| sample.old_path() == old_path)
        .map_or(Version::Missing, |sample| sample.versions.old.clone());
    let new = samples
        .into_iter()
        .find(|sample| sample.change.path == key.path)?
        .versions
        .new;
    Some(FileVersions { old, new })
}

const OPEN_PALETTE: &str = "5e35ea9";
const ADD_PROJECTS: &str = "6284693";

/// A file as the sidebar lists it, and its two versions.
struct Sample {
    change: FileChange,
    versions: FileVersions,
}

impl Sample {
    fn old_path(&self) -> &Path {
        self.change
            .renamed_from
            .as_deref()
            .unwrap_or(&self.change.path)
    }
}

fn samples(source: &Source) -> Vec<Sample> {
    match source {
        Source::Uncommitted => uncommitted(),
        Source::Commit(sha) if sha == OPEN_PALETTE => open_palette(),
        Source::Commit(sha) if sha == ADD_PROJECTS => add_projects(),
        Source::Commit(_) => Vec::new(),
    }
}

fn uncommitted() -> Vec<Sample> {
    vec![
        file(
            "crates/components/src/palette.rs",
            FileStatus::Modified,
            (4, 1),
            Staging::Partly,
        )
        .symbol(SymbolKind::Type, "PaletteRow", (2, 0))
        .symbol(SymbolKind::Function, "PaletteRow::render", (2, 1))
        .lines(PALETTE_ROW),
        file(
            "crates/components/src/palette/row.rs",
            FileStatus::Added,
            (38, 0),
            Staging::Staged,
        )
        .symbol(SymbolKind::Type, "Row", (24, 0))
        .symbol(SymbolKind::Function, "Row::render", (14, 0))
        .added(&[
            "//! One result row in the palette.",
            "",
            "pub struct Row {",
            "    pub label: SharedString,",
            "    pub detail: Option<SharedString>,",
            "}",
        ]),
        file(
            "crates/components/src/palette/scope.rs",
            FileStatus::Added,
            (52, 0),
            Staging::Staged,
        )
        .symbol(SymbolKind::Type, "Scope", (18, 0))
        .symbol(SymbolKind::Function, "Scope::parse", (34, 0))
        .added(&[
            "//! A keyword that narrows the palette to one kind of result.",
            "",
            "pub enum Scope {",
            "    Project(Option<ProjectId>),",
            "    Branch,",
            "}",
        ]),
        file(
            "crates/theme/src/colors.rs",
            FileStatus::Modified,
            (1, 1),
            Staging::Unstaged,
        )
        .symbol(SymbolKind::Type, "ThemeColors", (1, 1))
        .lines(&[
            (' ', "    /// Window background and diff body."),
            (' ', "    pub surface_app: Hsla,"),
            ('-', "    /// Sidebar, headers and status bar."),
            ('+', "    /// Sidebar, title bar and view headers."),
            (' ', "    pub surface_panel: Hsla,"),
        ]),
        file(
            "crates/ui/src/palette/fuzzy.rs",
            FileStatus::Deleted,
            (0, 64),
            Staging::Unstaged,
        )
        .removed(&[
            "//! Fuzzy matching for palette rows.",
            "",
            "pub fn matches(query: &str, label: &str) -> Vec<Range<usize>> {",
        ]),
        file(
            "crates/ui/src/root.rs",
            FileStatus::Modified,
            (12, 3),
            Staging::Unstaged,
        )
        .symbol(SymbolKind::Function, "Pensive::render", (9, 2))
        .symbol(SymbolKind::Function, "mocks", (3, 1))
        .lines(&root()),
        file(
            "docs/design.md",
            FileStatus::Modified,
            (6, 0),
            Staging::Unstaged,
        )
        .symbol(SymbolKind::Heading, "Navigation", (4, 0))
        .symbol(SymbolKind::Heading, "Command palette", (2, 0))
        .lines(&[
            (
                ' ',
                "Breadcrumbs separate levels with a chevron, never a slash.",
            ),
            ('+', ""),
            ('+', "## Window"),
            ('+', ""),
            ('+', "A window shows one project."),
        ]),
    ]
}

/// The files of the newest commit.
fn open_palette() -> Vec<Sample> {
    vec![
        file(
            "crates/components/src/palette.rs",
            FileStatus::Modified,
            (2, 1),
            Staging::Staged,
        )
        .symbol(SymbolKind::Function, "PaletteRow::render", (2, 1))
        .lines(PALETTE_ROW),
        file(
            "crates/ui/src/lib.rs",
            FileStatus::Modified,
            (4, 0),
            Staging::Staged,
        )
        .symbol(SymbolKind::Function, "init", (4, 0))
        .lines(&[
            (' ', "pub fn init(cx: &mut App) {"),
            ('+', "    text_input::bind_keys(cx);"),
            ('+', "    palette::bind_keys(cx);"),
            ('+', "    root::bind_keys(cx);"),
            ('+', ""),
            (' ', "}"),
        ]),
        file(
            "crates/ui/src/palette/mod.rs",
            FileStatus::Added,
            (79, 0),
            Staging::Staged,
        )
        .symbol(SymbolKind::Type, "PaletteView", (61, 0))
        .symbol(SymbolKind::Function, "PaletteView::reset", (18, 0))
        .added(&[
            "//! The palette's view: query, tab, keyword and selection.",
            "",
            "pub struct PaletteView {",
        ]),
        file(
            "crates/ui/src/root.rs",
            FileStatus::Modified,
            (21, 2),
            Staging::Staged,
        )
        .symbol(SymbolKind::Function, "Pensive::open_palette", (12, 0))
        .symbol(SymbolKind::Function, "Pensive::render", (9, 2))
        .lines(&[
            (' ', "impl Pensive {"),
            (
                '+',
                "    pub fn open_palette(&mut self, tab: Tab, cx: &mut Context<Self>) {",
            ),
            ('+', "        self.palette_open = true;"),
            ('+', "        cx.notify();"),
            ('+', "    }"),
        ]),
    ]
}

/// The files of the second commit, which no canvas opens.
fn add_projects() -> Vec<Sample> {
    vec![
        file(
            "crates/model/src/projects.rs",
            FileStatus::Renamed,
            (4, 2),
            Staging::Staged,
        )
        .renamed_from("crates/model/src/workspace.rs")
        .symbol(SymbolKind::Type, "Project", (3, 1))
        .lines(&[
            ('-', "//! The workspaces Pensive has open."),
            (
                '+',
                "//! Projects, with the worktrees and branches of each.",
            ),
            (' ', ""),
            ('-', "pub struct Workspace {"),
            ('+', "pub struct Project {"),
            (' ', "    pub root: PathBuf,"),
            ('+', "    pub worktrees: Vec<Worktree>,"),
            ('+', "    pub branches: Vec<Branch>,"),
            (' ', "}"),
        ]),
    ]
}

/// Two changes far enough apart that the unchanged lines between and around them fold.
fn root() -> Vec<(char, String)> {
    let unchanged = |lines: std::ops::Range<usize>| {
        lines.map(|line| (' ', format!("            .child(self.section({line}, cx))")))
    };
    let changed = |lines: &[(char, &str)]| {
        lines
            .iter()
            .map(|&(mark, text)| (mark, text.to_owned()))
            .collect::<Vec<_>>()
    };
    let mut lines: Vec<(char, String)> = unchanged(0..30).collect();
    lines.extend(changed(&[
        (' ', "        let theme = cx.theme();"),
        ('+', "        let density = cx.density();"),
        (' ', "        let root = div()"),
        ('-', "            .id(\"pensive\")"),
        ('+', "            .id(\"window\")"),
        ('+', "            .flex_col()"),
    ]));
    lines.extend(unchanged(30..75));
    lines.extend(changed(&[
        ('-', "        Mock::new(\"root\", \"landing\", landing),"),
        (
            '+',
            "        Mock::new(\"root\", \"landing\", landing).size(WINDOW),",
        ),
    ]));
    lines.extend(unchanged(75..100));
    lines
}

/// The change the canvas draws in the main view.
const PALETTE_ROW: &[(char, &str)] = &[
    (' ', "pub struct PaletteRow {"),
    (' ', "    pub label: SharedString,"),
    (' ', "    pub detail: Option<SharedString>,"),
    ('+', "    pub matches: Vec<Range<usize>>,"),
    ('+', "    pub status: RowStatus,"),
    (' ', "}"),
    (' ', ""),
    (' ', "impl RenderOnce for PaletteRow {"),
    (
        ' ',
        "    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {",
    ),
    (' ', "        let theme = cx.theme();"),
    ('+', "        let density = cx.density();"),
    (' ', "        h_flex()"),
    ('-', "            .h(px(28.))"),
    ('+', "            .h(density.row_height)"),
    (' ', "            .gap(density.row_gap)"),
    (' ', "            .child(self.icon)"),
    (
        ' ',
        "            .child(highlighted(self.label, &self.matches))",
    ),
    (' ', "            .child(self.status)"),
    (' ', "    }"),
    (' ', "}"),
];

/// A file whose change is staged, unstaged or both. Unstaged, a new file is untracked.
fn file(
    path: &str,
    status: FileStatus,
    (added, removed): (usize, usize),
    staging: Staging,
) -> FileChange {
    let unstaged = match status {
        FileStatus::Added => FileStatus::Untracked,
        status => status,
    };
    let (index, worktree) = match staging {
        Staging::Staged => (Some(status), None),
        Staging::Partly => (Some(status), Some(unstaged)),
        Staging::Unstaged => (None, Some(unstaged)),
    };
    FileChange {
        path: path.into(),
        renamed_from: None,
        index,
        worktree,
        added,
        removed,
        symbols: Vec::new(),
    }
}

trait Build: Sized {
    fn renamed_from(self, path: &str) -> Self;
    fn symbol(self, kind: SymbolKind, name: &str, counts: (usize, usize)) -> Self;
    /// Versions from lines marked `+` for added, `-` for removed or a space for unchanged.
    fn lines(self, lines: &[(char, impl AsRef<str>)]) -> Sample;

    fn added(self, lines: &[&str]) -> Sample {
        let lines: Vec<_> = lines.iter().map(|&text| ('+', text)).collect();
        self.lines(&lines)
    }

    fn removed(self, lines: &[&str]) -> Sample {
        let lines: Vec<_> = lines.iter().map(|&text| ('-', text)).collect();
        self.lines(&lines)
    }
}

impl Build for FileChange {
    fn renamed_from(mut self, path: &str) -> Self {
        self.renamed_from = Some(path.into());
        self
    }

    fn symbol(mut self, kind: SymbolKind, name: &str, (added, removed): (usize, usize)) -> Self {
        self.symbols.push(Symbol {
            kind,
            name: SharedString::from(name.to_string()),
            added,
            removed,
        });
        self
    }

    fn lines(self, lines: &[(char, impl AsRef<str>)]) -> Sample {
        let side = |skip: char, missing: bool| {
            if missing {
                return Version::Missing;
            }
            Version::Text(
                lines
                    .iter()
                    .filter(|(mark, _)| *mark != skip)
                    .map(|(_, text)| format!("{}\n", text.as_ref()))
                    .collect(),
            )
        };
        let status = self.status();
        let new_file = matches!(status, FileStatus::Added | FileStatus::Untracked);
        let versions = FileVersions {
            old: side('+', new_file),
            new: side('-', status == FileStatus::Deleted),
        };
        Sample {
            change: self,
            versions,
        }
    }
}

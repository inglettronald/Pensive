//! Sample changes for each project's main worktree. Pensive's are the files and commits drawn on
//! the app shell canvas, and the other projects have smaller sets.
//!
//! Pensive's line counts match the canvas. Each file's versions are an excerpt, so a file can count
//! more lines than its diff shows.

mod other_projects;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::SharedString;

use super::{DOTFILES, GLYPH_UI, HARBOR, LANTERN, PENSIVE, ZED};
use crate::{
    Base, Changes, Commit, FileChange, FileKey, FileOrigin, FileSource, FileStatus, FileVersions,
    ProjectId, Source, Staging, Symbol, SymbolKind, Upstream, Version,
};

/// Pensive's: seven uncommitted files, two staged and one partly staged, then two unpushed commits
/// and one pushed.
pub fn changes() -> Changes {
    pensive().changes()
}

/// The sample changes in the main worktree at `worktree`, if it's a sample project's.
pub(crate) fn changes_in(worktree: &Path) -> Option<Changes> {
    Some(samples_in(worktree)?.changes())
}

/// Whether `worktree` is a sample project's main worktree.
pub(crate) fn has_samples(worktree: &Path) -> bool {
    samples_in(worktree).is_some()
}

/// Both versions of the sample file `key` names. As in Git, the old version is the one at
/// `renamed_from` when that's given, so a rename read without it has no old version.
pub(crate) fn versions(
    worktree: &Path,
    key: &FileKey,
    renamed_from: Option<&Path>,
) -> Option<FileVersions> {
    let samples = samples_in(worktree)?;
    let mut commits = samples.commits.into_iter();
    let (mut files, new_origin) = match &key.source {
        Source::Uncommitted => (samples.uncommitted, FileOrigin::Worktree),
        Source::Commit(sha) => {
            let commit = commits.find(|sample| sample.commit.sha == *sha)?;
            (commit.files, FileOrigin::Commit(Some(sha.to_string())))
        }
    };
    let old_revision = commits.next().map(|sample| sample.commit.sha.to_string());
    if key.path == Path::new(super::SYNTAX_SAMPLE) {
        files.push(syntax_sample());
    }
    let old_path = renamed_from.unwrap_or(&key.path);
    let old = files
        .iter()
        .find(|sample| sample.old_path() == old_path)
        .map_or(Version::Missing, |sample| sample.versions.0.clone());
    let new = files
        .into_iter()
        .find(|sample| sample.change.path == key.path)?
        .versions
        .1;
    Some(FileVersions {
        old,
        new,
        old_source: FileSource {
            path: old_path.to_path_buf(),
            origin: FileOrigin::Commit(old_revision),
        },
        new_source: FileSource {
            path: key.path.clone(),
            origin: new_origin,
        },
    })
}

/// The files the samples leave in `worktree`: each commit applied from the oldest, then the
/// uncommitted changes.
pub(crate) fn on_disk(worktree: &Path) -> Vec<PathBuf> {
    let Some(samples) = samples_in(worktree) else {
        return Vec::new();
    };
    let committed = samples
        .commits
        .iter()
        .rev()
        .flat_map(|commit| &commit.files);
    let mut files = BTreeSet::new();
    for sample in committed.chain(&samples.uncommitted) {
        files.remove(sample.old_path());
        if sample.change.status() != FileStatus::Deleted {
            files.insert(sample.change.path.clone());
        }
    }
    files.into_iter().collect()
}

const OPEN_PALETTE: &str = "5e35ea9";
const ADD_PROJECTS: &str = "6284693";

/// A main worktree's changes, with both versions of every file.
struct WorktreeSamples {
    base: Option<Base>,
    upstream: Option<Upstream>,
    uncommitted: Vec<Sample>,
    /// The base's commits from where the branch started, newest first.
    earlier: Vec<SampleCommit>,
    commits: Vec<SampleCommit>,
}

impl WorktreeSamples {
    fn changes(self) -> Changes {
        let changes = |samples: Vec<Sample>| -> Vec<FileChange> {
            samples.into_iter().map(|sample| sample.change).collect()
        };
        let commits = |samples: Vec<SampleCommit>| {
            samples
                .into_iter()
                .map(|SampleCommit { commit, files }| Commit {
                    files: changes(files).into(),
                    ..commit
                })
                .collect()
        };
        Changes {
            base: self.base,
            upstream: self.upstream,
            uncommitted: changes(self.uncommitted),
            commits: commits(self.commits),
            earlier: commits(self.earlier),
        }
    }
}

/// A commit, with its files kept apart until they're turned into changes.
struct SampleCommit {
    commit: Commit,
    files: Vec<Sample>,
}

impl SampleCommit {
    fn new(sha: &str, title: &str, pushed: bool, files: Vec<Sample>) -> Self {
        Self {
            commit: Commit {
                sha: SharedString::from(sha.to_string()),
                title: SharedString::from(title.to_string()),
                description: None,
                pushed,
                files: Arc::default(),
            },
            files,
        }
    }

    fn described(mut self, description: &str) -> Self {
        self.commit.description = Some(SharedString::from(description.to_string()));
        self
    }
}

/// A file as the sidebar lists it, and its two versions.
struct Sample {
    change: FileChange,
    versions: (Version, Version),
}

impl Sample {
    fn old_path(&self) -> &Path {
        self.change
            .renamed_from
            .as_deref()
            .unwrap_or(&self.change.path)
    }
}

fn samples_in(worktree: &Path) -> Option<WorktreeSamples> {
    let index = super::repositories()
        .iter()
        .position(|(repository, _)| repository.root == worktree)?;
    match ProjectId(index) {
        PENSIVE => Some(pensive()),
        HARBOR => Some(other_projects::harbor()),
        LANTERN => Some(other_projects::lantern()),
        GLYPH_UI => Some(other_projects::glyph_ui()),
        DOTFILES => Some(other_projects::dotfiles()),
        ZED => Some(other_projects::zed()),
        _ => None,
    }
}

fn base(branch: &str) -> Option<Base> {
    Some(Base {
        branch: SharedString::from(branch.to_string()),
    })
}

fn origin(behind: usize) -> Option<Upstream> {
    Some(Upstream {
        remote: "origin".into(),
        behind,
    })
}

fn pensive() -> WorktreeSamples {
    WorktreeSamples {
        base: base("master"),
        upstream: origin(1),
        uncommitted: uncommitted(),
        earlier: [
            (
                "532d38b",
                "Merge pull request #84 from feat/shared-tree-row",
            ),
            (
                "7edb2ff",
                "refactor: one tree row for the Changes and Files panels",
            ),
            (
                "35b32bc",
                "refactor: one tree builder for the Changes and Files panels",
            ),
            (
                "7b1f208",
                "Merge pull request #82 from feat/zoom-and-code-size",
            ),
            (
                "a14c0de",
                "feat: zoom the whole window, and size code apart from it",
            ),
            (
                "3f9e21b",
                "feat: save open files a second after typing stops",
            ),
            (
                "c07d5a4",
                "fix: keep the caret in view while typing at the end",
            ),
        ]
        .into_iter()
        .map(|(sha, title)| SampleCommit::new(sha, title, true, Vec::new()))
        .collect(),
        commits: vec![
            SampleCommit::new(
                OPEN_PALETTE,
                "feat: open the command palette with Ctrl K",
                false,
                open_palette(),
            )
            .described(
                "On the landing screen the palette fills the page, so the same keys work before \
                 and after a project is open.",
            ),
            SampleCommit::new(
                ADD_PROJECTS,
                "feat: add projects, worktrees and branches",
                false,
                add_projects(),
            )
            .described("Projects are cached, so reading them never runs Git."),
            SampleCommit::new(
                "ef032a6",
                "feat: add CommandPalette component",
                true,
                Vec::new(),
            ),
        ],
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
        file(
            "crates/model/src/recent.rs",
            FileStatus::Deleted,
            (0, 3),
            Staging::Staged,
        )
        .removed(&[
            "//! The folders opened most recently.",
            "",
            "pub struct Recent(Vec<PathBuf>);",
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

fn syntax_sample() -> Sample {
    let mut lines = vec![
        (' ', "public class Greeting {"),
        (' ', "    // Keep the response short."),
        ('-', "    private final Object prefix = 404;"),
        ('+', "    private final Object prefix = \"café\";"),
        ('+', "    private final int limit = 12;"),
        (' ', ""),
        (' ', "    /*"),
        (' ', "     * The default is used by the preview."),
    ];
    lines.extend(std::iter::repeat_n((' ', "     * More context."), 110));
    lines.extend([
        (' ', "     * End of the preview notes."),
        (' ', "     */"),
        (' ', "    public Object message() {"),
        ('-', "        return send(\"old\", 12);"),
        ('+', "        return send(\"new\", 24);"),
        (' ', "    }"),
        (' ', "}"),
    ]);
    file(
        super::SYNTAX_SAMPLE,
        FileStatus::Modified,
        (3, 2),
        Staging::Unstaged,
    )
    .lines(&lines)
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
        let versions = (
            side('+', new_file),
            side('-', status == FileStatus::Deleted),
        );
        Sample {
            change: self,
            versions,
        }
    }
}

//! Sample changes for the pensive project: the files and commits drawn on the app shell canvas.
//!
//! Line counts match the canvas. Hunks are excerpts, so a file can count more lines than it shows.

use gpui::SharedString;

use crate::{
    Changes, Commit, FileChange, FileStatus, Hunk, HunkLine, LineChange, Symbol, SymbolKind,
    Upstream,
};

/// Seven uncommitted files with three staged, two unpushed commits and one pushed.
pub fn changes() -> Changes {
    Changes {
        base: Some(("master".into(), 3)),
        upstream: Some(Upstream {
            remote: "origin".into(),
            ahead: 2,
            behind: 1,
        }),
        uncommitted: vec![
            file("crates/components/src/palette.rs", FileStatus::Modified, (4, 1), true)
                .symbol(SymbolKind::Type, "PaletteRow", (2, 0))
                .symbol(SymbolKind::Function, "PaletteRow::render", (2, 1))
                .hunk(palette_row()),
            file("crates/components/src/palette/row.rs", FileStatus::Added, (38, 0), true)
                .symbol(SymbolKind::Type, "Row", (24, 0))
                .symbol(SymbolKind::Function, "Row::render", (14, 0))
                .hunk(added(
                    "@@ -0,0 +1,38 @@",
                    &[
                        "//! One result row in the palette.",
                        "",
                        "pub struct Row {",
                        "    pub label: SharedString,",
                        "    pub detail: Option<SharedString>,",
                        "}",
                    ],
                )),
            file("crates/components/src/palette/scope.rs", FileStatus::Added, (52, 0), true)
                .symbol(SymbolKind::Type, "Scope", (18, 0))
                .symbol(SymbolKind::Function, "Scope::parse", (34, 0))
                .hunk(added(
                    "@@ -0,0 +1,52 @@",
                    &[
                        "//! A keyword that narrows the palette to one kind of result.",
                        "",
                        "pub enum Scope {",
                        "    Project(Option<ProjectId>),",
                        "    Branch,",
                        "}",
                    ],
                )),
            file("crates/theme/src/colors.rs", FileStatus::Modified, (1, 1), false)
                .symbol(SymbolKind::Type, "ThemeColors", (1, 1))
                .hunk(theme_colors()),
            file("crates/ui/src/palette/fuzzy.rs", FileStatus::Deleted, (0, 64), false).hunk(
                removed(
                    "@@ -1,64 +0,0 @@",
                    &[
                        "//! Fuzzy matching for palette rows.",
                        "",
                        "pub fn matches(query: &str, label: &str) -> Vec<Range<usize>> {",
                    ],
                ),
            ),
            file("crates/ui/src/root.rs", FileStatus::Modified, (12, 3), false)
                .symbol(SymbolKind::Function, "Pensive::render", (9, 2))
                .symbol(SymbolKind::Function, "mocks", (3, 1))
                .hunk(hunk(
                    "@@ -186,6 +186,8 @@ impl Render for Pensive",
                    186,
                    &[
                        (' ', "        let theme = cx.theme();"),
                        ('+', "        let density = cx.density();"),
                        (' ', "        let root = div()"),
                        ('-', "            .id(\"pensive\")"),
                        ('+', "            .id(\"window\")"),
                        ('+', "            .flex_col()"),
                    ],
                )),
            file("docs/design.md", FileStatus::Modified, (6, 0), false)
                .symbol(SymbolKind::Heading, "Navigation", (4, 0))
                .symbol(SymbolKind::Heading, "Command palette", (2, 0))
                .hunk(hunk(
                    "@@ -59,3 +59,7 @@ ## Navigation",
                    59,
                    &[
                        (' ', "Breadcrumbs separate levels with a chevron, never a slash."),
                        ('+', ""),
                        ('+', "## Window"),
                        ('+', ""),
                        ('+', "A window shows one project."),
                    ],
                )),
        ],
        commits: vec![
            Commit {
                sha: "5e35ea9".into(),
                title: "feat: open the command palette with Ctrl K".into(),
                description: Some(
                    "On the landing screen the palette fills the page, so the same keys work \
                     before and after a project is open."
                        .into(),
                ),
                pushed: false,
                files: vec![
                    file("crates/components/src/palette.rs", FileStatus::Modified, (2, 1), true)
                        .symbol(SymbolKind::Function, "PaletteRow::render", (2, 1))
                        .hunk(palette_row()),
                    file("crates/ui/src/lib.rs", FileStatus::Modified, (4, 0), true)
                        .symbol(SymbolKind::Function, "init", (4, 0))
                        .hunk(hunk(
                            "@@ -8,4 +8,8 @@ pub fn init",
                            8,
                            &[
                                (' ', "pub fn init(cx: &mut App) {"),
                                ('+', "    text_input::bind_keys(cx);"),
                                ('+', "    palette::bind_keys(cx);"),
                                ('+', "    root::bind_keys(cx);"),
                                ('+', ""),
                                (' ', "}"),
                            ],
                        )),
                    file("crates/ui/src/palette/mod.rs", FileStatus::Added, (79, 0), true)
                        .symbol(SymbolKind::Type, "PaletteView", (61, 0))
                        .symbol(SymbolKind::Function, "PaletteView::reset", (18, 0))
                        .hunk(added(
                            "@@ -0,0 +1,79 @@",
                            &[
                                "//! The palette's view: query, tab, keyword and selection.",
                                "",
                                "pub struct PaletteView {",
                            ],
                        )),
                    file("crates/ui/src/root.rs", FileStatus::Modified, (21, 2), true)
                        .symbol(SymbolKind::Function, "Pensive::open_palette", (12, 0))
                        .symbol(SymbolKind::Function, "Pensive::render", (9, 2))
                        .hunk(hunk(
                            "@@ -60,4 +60,9 @@ impl Pensive",
                            60,
                            &[
                                ('+', "    pub fn open_palette(&mut self, tab: Tab, cx: &mut Context<Self>) {"),
                                ('+', "        self.palette_open = true;"),
                                ('+', "        cx.notify();"),
                                ('+', "    }"),
                            ],
                        )),
                ],
            },
            Commit {
                sha: "6284693".into(),
                title: "feat: add projects, worktrees and branches".into(),
                description: Some("Projects are cached, so reading them never runs Git.".into()),
                pushed: false,
                files: Vec::new(),
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

fn file(
    path: &str,
    status: FileStatus,
    (added, removed): (usize, usize),
    staged: bool,
) -> FileChange {
    FileChange {
        path: path.into(),
        status,
        added,
        removed,
        staged,
        symbols: Vec::new(),
        hunks: Vec::new(),
    }
}

trait Build {
    fn symbol(self, kind: SymbolKind, name: &str, counts: (usize, usize)) -> Self;
    fn hunk(self, hunk: Hunk) -> Self;
}

impl Build for FileChange {
    fn symbol(mut self, kind: SymbolKind, name: &str, (added, removed): (usize, usize)) -> Self {
        self.symbols.push(Symbol {
            kind,
            name: SharedString::from(name.to_string()),
            added,
            removed,
        });
        self
    }

    fn hunk(mut self, hunk: Hunk) -> Self {
        self.hunks.push(hunk);
        self
    }
}

/// A hunk whose new-file numbers count up from `start`. `+` adds, `-` removes, a space is context.
fn hunk(header: &str, start: usize, lines: &[(char, &str)]) -> Hunk {
    let mut number = start;
    Hunk {
        header: header.to_string().into(),
        lines: lines
            .iter()
            .map(|&(mark, text)| {
                let change = match mark {
                    '+' => LineChange::Added,
                    '-' => LineChange::Removed,
                    _ => LineChange::Context,
                };
                let line_number = (change != LineChange::Removed).then(|| {
                    number += 1;
                    number - 1
                });
                HunkLine {
                    change,
                    number: line_number,
                    text: text.to_string().into(),
                    changed: Vec::new(),
                }
            })
            .collect(),
    }
}

fn added(header: &str, lines: &[&str]) -> Hunk {
    let lines: Vec<_> = lines.iter().map(|&text| ('+', text)).collect();
    hunk(header, 1, &lines)
}

fn removed(header: &str, lines: &[&str]) -> Hunk {
    let lines: Vec<_> = lines.iter().map(|&text| ('-', text)).collect();
    hunk(header, 1, &lines)
}

/// The hunk the canvas draws in the main view.
fn palette_row() -> Hunk {
    let mut hunk = hunk(
        "@@ -12,14 +12,15 @@ pub struct PaletteRow",
        12,
        &[
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
        ],
    );
    hunk.lines[12].changed.push(15..22);
    hunk.lines[13].changed.push(15..33);
    hunk
}

fn theme_colors() -> Hunk {
    let mut hunk = hunk(
        "@@ -18,7 +18,7 @@ pub struct ThemeColors",
        18,
        &[
            (' ', "    /// Window background and diff body."),
            (' ', "    pub surface_app: Hsla,"),
            ('-', "    /// Sidebar, headers and status bar."),
            ('+', "    /// Sidebar, title bar and view headers."),
            (' ', "    pub surface_panel: Hsla,"),
        ],
    );
    hunk.lines[2].changed.push(17..39);
    hunk.lines[3].changed.push(17..43);
    hunk
}

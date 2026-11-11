//! One mock per state on the design canvas, built from the same sample projects and branches.

use std::iter::once;

use gpui::{AnyElement, AnyView, App, div, prelude::*, px, size};
use mock::Mock;
use theme::DensityPreset;

use super::{CommandPalette, KeyHint, PaletteChip, PaletteEntry, PaletteSection, TrailingText};
use crate::{IconName, SegmentedControl, Select, Toggle};

const TABS: [&str; 3] = ["All", "Active", "Commands"];
const ALL: Option<usize> = Some(0);
const ACTIVE: Option<usize> = Some(1);
const COMMANDS: Option<usize> = Some(2);

pub(crate) fn mocks() -> Vec<Mock> {
    let size = size(px(640.), px(560.));
    let calm = |name, render| Mock::component("command_palette", name, render).size(size);
    let dense = |name, build| Mock::new("command_palette", name, build).size(size);
    vec![
        calm("landing", || framed(landing())),
        calm("active", || framed(active())),
        calm("project", || framed(project())),
        calm("typing", || framed(typing())),
        calm("scoped", || framed(scoped())),
        calm("clone", || framed(clone())),
        calm("no_results", || framed(no_results())),
        calm("settings", || framed(settings())),
        calm("merge", || framed(merge())),
        calm("commands", || framed(commands())),
        calm("find_settings", || framed(find_settings())),
        calm("trailing", || framed(trailing())),
        dense("project_dense", |_, cx| in_dense(cx, || framed(project()))),
        dense("settings_dense", |_, cx| {
            in_dense(cx, || framed(settings()))
        }),
    ]
}

fn framed(palette: CommandPalette) -> AnyElement {
    div()
        .flex()
        .justify_center()
        .pt_8()
        .child(palette)
        .into_any_element()
}

fn in_dense(cx: &mut App, render: fn() -> AnyElement) -> AnyView {
    cx.set_global(DensityPreset::Dense.density());
    mock::element_view(cx, render)
}

fn open() -> Vec<KeyHint> {
    vec![
        KeyHint::new("Enter", "Open"),
        KeyHint::new("Ctrl Enter", "New window"),
    ]
}

fn switch() -> Vec<KeyHint> {
    vec![
        KeyHint::new("Enter", "Switch"),
        KeyHint::new("Ctrl Enter", "New window"),
    ]
}

fn project_entry(tile: &str, name: &str, path: &str, branch: &str, when: &str) -> PaletteEntry {
    PaletteEntry::new(name.to_string())
        .tile(tile.to_string())
        .detail(path.to_string())
        .trailing(
            TrailingText::new(branch.to_string())
                .code()
                .color(|colors| colors.text_muted),
        )
        .when(when.to_string())
        .hints(open())
}

fn pensive() -> PaletteEntry {
    project_entry(
        "P",
        "pensive",
        "~/Projects/pensive",
        "feat/command-palette",
        "2m",
    )
}
fn multiver() -> PaletteEntry {
    project_entry(
        "M",
        "multiver",
        "~/work/multiver",
        "dulkir/hud-scaling",
        "26m",
    )
}
fn cdn_assets() -> PaletteEntry {
    project_entry(
        "C",
        "cdn-assets",
        "~/work/cdn-assets",
        "feat/cosmetics-batch",
        "48m",
    )
}
fn gameui() -> PaletteEntry {
    project_entry("G", "gameui", "~/work/gameui", "main", "2h")
}
fn dotfiles() -> PaletteEntry {
    project_entry("D", "dotfiles", "~/dotfiles", "main", "3d")
}

fn work(icon: IconName, branch: &str, project: &str, when: &str) -> PaletteEntry {
    PaletteEntry::new(branch.to_string())
        .icon(icon)
        .code()
        .detail(project.to_string())
        .when(when.to_string())
        .hints(switch())
}

fn command(icon: IconName, name: &str) -> PaletteEntry {
    PaletteEntry::new(name.to_string()).icon(icon)
}

fn landing() -> CommandPalette {
    CommandPalette::new()
        .tabs(TABS, ALL)
        .placeholder("Search, or paste a clone URL")
        .section(PaletteSection::new(
            "Recent",
            [pensive(), multiver(), cdn_assets(), gameui(), dotfiles()],
        ))
        .section(PaletteSection::new(
            "Start",
            [
                command(IconName::Folder, "Open folder…").kbd("Ctrl O"),
                command(IconName::Download, "Clone repository…").kbd("Ctrl Shift O"),
                command(IconName::Plus, "New repository…"),
            ],
        ))
        .selected(0)
}

fn active() -> CommandPalette {
    CommandPalette::new()
        .overlay()
        .tabs(TABS, ACTIVE)
        .placeholder("Filter recent work")
        .section(
            PaletteSection::new(
                "Last hour",
                [
                    work(IconName::FolderGit2, "feat/palette-scopes", "pensive", "4m"),
                    work(IconName::FolderGit2, "fix/fuzzy-ranking", "pensive", "11m"),
                    work(IconName::GitBranch, "dulkir/hud-scaling", "multiver", "26m"),
                    work(
                        IconName::FolderGit2,
                        "agent/cosmetics-manifest",
                        "cdn-assets",
                        "48m",
                    ),
                ],
            )
            .aside("7 across 4 projects"),
        )
        .section(PaletteSection::new(
            "Today",
            [
                work(IconName::FolderGit2, "chore/lint-pass", "gameui", "2h"),
                work(IconName::GitBranch, "feat/command-palette", "pensive", "3h"),
                work(IconName::FolderGit2, "fix/tooltip-clip", "multiver", "5h"),
            ],
        ))
        .selected(0)
}

fn project() -> CommandPalette {
    CommandPalette::new()
        .overlay()
        .tabs(TABS, ALL)
        .placeholder("Search, or paste a clone URL")
        .section(PaletteSection::new(
            "Projects",
            [
                multiver(),
                cdn_assets(),
                gameui(),
                dotfiles(),
                project_entry(
                    "P",
                    "pensive",
                    "~/Projects/pensive",
                    "feat/command-palette",
                    "This window",
                ),
            ],
        ))
        .section(PaletteSection::new(
            "Commands",
            [
                command(IconName::GitBranch, "Switch branch…").kbd("Ctrl B"),
                command(IconName::GitMerge, "Merge into feat/command-palette…"),
                command(IconName::Download, "Clone repository…").kbd("Ctrl Shift O"),
            ],
        ))
        .selected(0)
}

fn typing() -> CommandPalette {
    CommandPalette::new()
        .overlay()
        .tabs(TABS, ALL)
        .query("gui")
        .section(PaletteSection::new(
            "Best matches",
            [
                work(IconName::GitBranch, "dulkir/gui-tooltips", "multiver", "1d")
                    .matches(once(7..10)),
                gameui().matches([0..1, 4..6]),
                work(IconName::FolderGit2, "fix/gutter-icons", "pensive", "2d")
                    .matches([4..6, 11..12]),
                work(IconName::GitBranch, "feat/gpu-instancing", "gameui", "3w").matches([
                    5..6,
                    7..8,
                    9..10,
                ]),
            ],
        ))
        .section(PaletteSection::new(
            "Narrow to",
            [
                command(IconName::Search, "branch: gui")
                    .code()
                    .detail("Branches and worktrees"),
                command(IconName::Search, "project: gui")
                    .code()
                    .detail("Projects"),
            ],
        ))
        .selected(0)
}

fn scoped() -> CommandPalette {
    let branch = |icon, name: &str, matched: std::ops::Range<usize>, when: &str| {
        PaletteEntry::new(name.to_string())
            .icon(icon)
            .code()
            .matches(once(matched))
            .when(when.to_string())
    };
    CommandPalette::new()
        .overlay()
        .tabs(TABS, None)
        .chip(PaletteChip::new("project:").value("multiver").removable())
        .query("fix")
        .section(PaletteSection::new(
            "Branches and worktrees in multiver",
            [
                branch(IconName::FolderGit2, "fix/tooltip-clip", 0..3, "5h")
                    .detail("~/work/multiver-wt/tooltip-clip")
                    .hints(switch()),
                branch(IconName::GitBranch, "fix/crash-on-resize", 0..3, "1w"),
                branch(
                    IconName::GitBranch,
                    "dulkir/fix-locale-fallback",
                    7..10,
                    "3d",
                ),
                branch(IconName::GitBranch, "hotfix/1.8.2", 3..6, "2w"),
            ],
        ))
        .section(PaletteSection::new(
            "Commands",
            [command(IconName::Plus, "Create branch fix in multiver").kbd("Ctrl N")],
        ))
        .selected(0)
}

fn clone() -> CommandPalette {
    CommandPalette::new()
        .overlay()
        .tabs(TABS, ALL)
        .query("https://github.com/zed-industries/zed.git")
        .section(PaletteSection::new(
            "Clone",
            [command(IconName::Download, "Clone zed-industries/zed")
                .detail("into ~/Projects/zed")
                .hints([
                    KeyHint::new("Enter", "Clone"),
                    KeyHint::new("Ctrl Enter", "Clone into…"),
                ])],
        ))
        .section(PaletteSection::new(
            "Already on disk",
            [project_entry("Z", "zed", "~/src/zed", "main", "5w")],
        ))
        .selected(0)
}

fn no_results() -> CommandPalette {
    CommandPalette::new()
        .overlay()
        .tabs(TABS, None)
        .chip(PaletteChip::new("project:").value("multiver").removable())
        .query("qwerty")
        .empty(
            "Nothing in multiver matches \"qwerty\"",
            "Searched branches, worktrees and commands",
        )
        .section(PaletteSection::new(
            "Try",
            [
                command(IconName::Search, "Search every project for \"qwerty\"")
                    .hints([KeyHint::new("Enter", "Search")]),
                command(IconName::Plus, "Create branch qwerty in multiver").kbd("Ctrl N"),
            ],
        ))
        .selected(0)
}

fn settings() -> CommandPalette {
    let show_in_settings = || KeyHint::new("Ctrl Enter", "Show in Settings");
    let toggle = |id: &'static str, name: &str, description: &str, on: bool| {
        PaletteEntry::new(name.to_string())
            .description(description.to_string())
            .control(Toggle::new(id, on))
            .hints([KeyHint::new("Enter", "Toggle"), show_in_settings()])
    };
    CommandPalette::new()
        .overlay()
        .tabs(TABS, None)
        .chip(PaletteChip::new("setting:"))
        .placeholder("Search settings")
        .section(PaletteSection::new(
            "Diff",
            [
                PaletteEntry::new("Line numbers")
                    .description("Which line numbers the diff shows. Ctrl L cycles them.")
                    .control(SegmentedControl::new(
                        "line-numbers",
                        ["Off", "New file", "Both"],
                        1,
                    ))
                    .hints([KeyHint::new("←→", "Change"), show_in_settings()]),
                PaletteEntry::new("Change marker")
                    .description("How changed lines are marked")
                    .control(Select::new("change-marker", "Edge bar")),
                toggle(
                    "line-fill",
                    "Line fill",
                    "A faint fill behind changed lines",
                    true,
                ),
                toggle(
                    "gutter-tint",
                    "Gutter tint",
                    "Tint the line number gutter on changed lines",
                    false,
                ),
                toggle(
                    "word-highlight",
                    "Changed-word highlight",
                    "A stronger fill on the words that changed",
                    true,
                ),
            ],
        ))
        .section(PaletteSection::new(
            "Appearance",
            [PaletteEntry::new("Density")
                .description("Calm has larger text and 28 px rows")
                .control(Select::new("density", "Calm"))],
        ))
        .selected(0)
}

fn merge() -> CommandPalette {
    let target = |icon, name: &str, description: &str, when: &str| {
        PaletteEntry::new(name.to_string())
            .icon(icon)
            .code()
            .description(description.to_string())
            .when(when.to_string())
    };
    CommandPalette::new()
        .overlay()
        .tabs(TABS, None)
        .chip(PaletteChip::new("merge:"))
        .placeholder("Branch to merge in")
        .section(PaletteSection::new(
            "Into feat/command-palette",
            [
                target(
                    IconName::FolderGit2,
                    "feat/palette-scopes",
                    "6 commits, merges cleanly",
                    "4m",
                )
                .hints([
                    KeyHint::new("Enter", "Merge"),
                    KeyHint::new("Ctrl Enter", "Squash and merge"),
                ]),
                target(
                    IconName::FolderGit2,
                    "fix/fuzzy-ranking",
                    "2 commits, conflicts in palette.rs",
                    "11m",
                ),
                target(
                    IconName::GitBranch,
                    "master",
                    "3 commits, fast-forward",
                    "2h",
                ),
            ],
        ))
        .selected(0)
}

fn commands() -> CommandPalette {
    CommandPalette::new()
        .overlay()
        .tabs(TABS, COMMANDS)
        .placeholder("Search commands")
        .section(PaletteSection::new(
            "Recently used",
            [
                command(IconName::GitBranch, "Switch branch…").kbd("Ctrl B"),
                command(IconName::GitMerge, "Merge into feat/command-palette…"),
            ],
        ))
        .section(PaletteSection::new(
            "Git",
            [
                command(IconName::GitCommitHorizontal, "Commit…"),
                command(IconName::RefreshCw, "Fetch"),
                command(IconName::ArrowDownToLine, "Pull"),
                command(IconName::ArrowUpFromLine, "Push"),
                command(IconName::Archive, "Stash changes"),
                command(IconName::FolderGit2, "New worktree…"),
            ],
        ))
        .section(PaletteSection::new(
            "Projects",
            [
                command(IconName::Folder, "Open folder…").kbd("Ctrl O"),
                command(IconName::Download, "Clone repository…").kbd("Ctrl Shift O"),
            ],
        ))
        .selected(0)
}

fn find_settings() -> CommandPalette {
    CommandPalette::new()
        .overlay()
        .tabs(TABS, ALL)
        .query("sett")
        .section(PaletteSection::new(
            "Best matches",
            [
                command(IconName::Settings, "Settings")
                    .matches(once(0..4))
                    .kbd("Ctrl ,")
                    .hints([KeyHint::new("Enter", "Open")]),
                work(
                    IconName::FolderGit2,
                    "agent/settings-search",
                    "multiver",
                    "1h",
                )
                .matches(once(6..10)),
            ],
        ))
        .section(PaletteSection::new(
            "Narrow to",
            [command(IconName::Search, "setting:")
                .code()
                .detail("Search every setting")],
        ))
        .selected(0)
}

/// What a scope can put before the time. Only the gray time is the default.
fn trailing() -> CommandPalette {
    CommandPalette::new()
        .tabs(TABS, ALL)
        .section(PaletteSection::new(
            "Trailing text: only the gray time by default",
            [
                PaletteEntry::new("Default").when("4m"),
                PaletteEntry::new("A scope may add colored text")
                    .trailing(TrailingText::new("+214").color(|colors| colors.diff_added))
                    .trailing(TrailingText::new("−38").color(|colors| colors.diff_removed))
                    .when("4m"),
                PaletteEntry::new("Any meaning color works")
                    .trailing(TrailingText::new("2 conflicts").color(|colors| colors.diff_removed))
                    .when("48m"),
                PaletteEntry::new("Or plain text")
                    .trailing(TrailingText::new("detached"))
                    .when("1w"),
            ],
        ))
        .selected(0)
}

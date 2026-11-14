//! The Files panel's settings: what the tree lists, how it marks Git status, and how a file opens.

use gpui::App;
use serde::{Deserialize, Serialize};
use settings::{AnySetting, Category, Choice, Control, Page, Setting, Settings};

pub(crate) static CATEGORY: Category = Category::new("Files", "The project's tree");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitStatus {
    /// A file's name in its status color, and a dot after a folder with changes inside.
    Color,
    /// A letter after the name, such as M.
    Letter,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IgnoredFiles {
    Dimmed,
    Hidden,
    Normal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    FoldersFirst,
    Mixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenWith {
    OneClick,
    DoubleClick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Guides {
    Always,
    OnHover,
    Off,
}

pub(crate) static GIT_STATUS: Setting<GitStatus> = Setting {
    key: "files.git_status",
    name: "Git status",
    description: "How changed files show in the tree",
    default: GitStatus::Color,
    control: Control::Segmented(|_| {
        vec![
            Choice::new(GitStatus::Color, "Color"),
            Choice::new(GitStatus::Letter, "Letter"),
            Choice::new(GitStatus::Off, "Off"),
        ]
    }),
};

pub(crate) static IGNORED: Setting<IgnoredFiles> = Setting {
    key: "files.ignored",
    name: "Ignored files",
    description: "What .gitignore leaves out, such as target",
    default: IgnoredFiles::Dimmed,
    control: Control::Segmented(|_| {
        vec![
            Choice::new(IgnoredFiles::Dimmed, "Dimmed"),
            Choice::new(IgnoredFiles::Hidden, "Hidden"),
            Choice::new(IgnoredFiles::Normal, "Normal"),
        ]
    }),
};

pub(crate) static DOTFILES: Setting<bool> = Setting {
    key: "files.dotfiles",
    name: "Dotfiles",
    description: "Files and folders whose names start with a dot",
    default: true,
    control: Control::Toggle,
};

pub(crate) static FOLD: Setting<bool> = Setting {
    key: "files.fold_single_child_folders",
    name: "Fold single-child folders",
    description: "A chain of folders that each hold one folder shows as one row",
    default: true,
    control: Control::Toggle,
};

pub(crate) static SORT: Setting<Sort> = Setting {
    key: "files.sort",
    name: "Sort",
    description: "Whether folders come before files",
    default: Sort::FoldersFirst,
    control: Control::Segmented(|_| {
        vec![
            Choice::new(Sort::FoldersFirst, "Folders first"),
            Choice::new(Sort::Mixed, "Mixed"),
        ]
    }),
};

pub(crate) static GUIDES: Setting<Guides> = Setting {
    key: "files.indent_guides",
    name: "Indent guides",
    description: "A line under each open folder's arrow",
    default: Guides::Always,
    control: Control::Segmented(|_| {
        vec![
            Choice::new(Guides::Always, "Always"),
            Choice::new(Guides::OnHover, "On hover"),
            Choice::new(Guides::Off, "Off"),
        ]
    }),
};

pub(crate) static FOLLOW: Setting<bool> = Setting {
    key: "files.follow_open_file",
    name: "Follow the open file",
    description: "Opens the folders down to the file the editor shows",
    default: true,
    control: Control::Toggle,
};

pub(crate) static OPEN_WITH: Setting<OpenWith> = Setting {
    key: "files.open_with",
    name: "Open with",
    description: "How many clicks open a file",
    default: OpenWith::OneClick,
    control: Control::Segmented(|_| {
        vec![
            Choice::new(OpenWith::OneClick, "One click"),
            Choice::new(OpenWith::DoubleClick, "Double click"),
        ]
    }),
};

pub(crate) fn declare_settings(cx: &mut App) {
    let tree: [&'static dyn AnySetting; 6] =
        [&GIT_STATUS, &IGNORED, &DOTFILES, &FOLD, &SORT, &GUIDES];
    Settings::declare(Page::new(&CATEGORY, "Tree"), tree, cx);
    let opening: [&'static dyn AnySetting; 2] = [&FOLLOW, &OPEN_WITH];
    Settings::declare(Page::new(&CATEGORY, "Opening"), opening, cx);
}

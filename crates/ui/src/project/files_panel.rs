//! The Files panel: the project's tree under a header with new file, select the open file and
//! collapse all, and a filter field.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use components::{FileTreeRow, Icon, IconButton, IconName, Kbd, TreeStatus};
use gpui::{AnyElement, App, Context, Focusable, Hsla, Window, div, prelude::*};
use model::{FileKey, FileStatus, Source};
use theme::{ActiveTheme, ThemeColors, Typeset, dp};

use super::{
    Control, ProjectView,
    file_tree::{RowKind, TreeOptions, ancestors, rows},
    files_options::{
        DOTFILES, FOLD, FOLLOW, GIT_STATUS, GUIDES, GitStatus, Guides, IGNORED, OPEN_WITH,
        OpenWith, SORT,
    },
};
use crate::{PANEL, view_callback, view_callback_with};

impl ProjectView {
    pub(super) fn render_files_panel(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = &cx.theme().colors;
        div()
            .id("files-panel")
            .flex()
            .flex_col()
            .flex_none()
            .w(PANEL)
            .border_r_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_panel)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.files_hovered = *hovered;
                cx.notify();
            }))
            .child(self.files_header(cx))
            .child(self.file_filter(window, cx))
            .child(
                div()
                    .id("file-tree")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .pt_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.files_scroll)
                    .children(self.tree_rows(cx)),
            )
    }

    /// The worktree's name, then new file, select the open file and collapse all.
    fn files_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let name = self
            .worktree
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let button = |id: &'static str, icon: IconName| {
            IconButton::new(id, icon).color(|colors| colors.text_faint)
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_0p5()
            .h(density.row_height + dp(12.))
            .pl_4()
            .pr_2()
            .border_b_1()
            .border_color(colors.border_subtle)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .typeset(density.body)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.text_strong)
                    .child(name),
            )
            .child(self.with_notice(
                Control::NewFile,
                button("new-file", IconName::Plus).on_click(self.on_use(Control::NewFile, cx)),
            ))
            .child(
                button("locate", IconName::LocateFixed)
                    .on_click(view_callback(cx, |this: &mut Self, _, cx| {
                        this.locate_open_file(cx)
                    })),
            )
            .child(
                button("collapse", IconName::ChevronsDownUp).on_click(view_callback(
                    cx,
                    |this: &mut Self, _, cx| {
                        this.files_open.clear();
                        cx.notify();
                    },
                )),
            )
    }

    /// The filter, with its icon in the same column as the tree's top-level arrows.
    fn file_filter(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let focused = self.file_filter.focus_handle(cx).is_focused(window);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(density.row_height)
            .mx_2()
            .mt_2()
            .mb_1()
            .pl(dp(7.))
            .pr_2()
            .rounded(density.radius_md)
            .border_1()
            .border_color(if focused {
                colors.border_focus
            } else {
                colors.border_subtle
            })
            .bg(colors.surface_app)
            .typeset(density.small)
            .child(
                Icon::new(IconName::Search)
                    .size(dp(14.))
                    .color(colors.text_faint),
            )
            .child(div().flex_1().min_w_0().child(self.file_filter.clone()))
            .child(Kbd::new("Ctrl P"))
    }

    fn tree_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let projects = self.projects.read(cx);
        let Some(files) = projects.files(&self.worktree) else {
            return Vec::new();
        };
        let options = tree_options(cx);
        let filter = self.file_filter.read(cx).text().to_string();
        let rows = rows(files, &self.files_open, &filter, &options);
        let statuses = self.file_statuses(cx);
        let style = GIT_STATUS.get(cx);
        let guides = match GUIDES.get(cx) {
            Guides::Always => true,
            Guides::OnHover => self.files_hovered,
            Guides::Off => false,
        };
        let selected = self
            .selected
            .as_ref()
            .filter(|key| key.source == Source::Uncommitted)
            .map(|key| key.path.clone());
        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                let status = match &row.kind {
                    RowKind::File => statuses.get(&row.path).copied(),
                    RowKind::Folder { .. } => folder_status(&statuses, &row.path),
                };
                let status = status.and_then(|status| tree_status(status, style));
                let path = row.path.clone();
                match row.kind {
                    RowKind::Folder { open, openable } => FileTreeRow::folder(
                        index, row.name, row.depth, open,
                    )
                    .when(openable, |folder| {
                        folder.on_click(view_callback_with(
                            cx,
                            move |this: &mut Self, _: usize, _, cx| {
                                this.toggle_folder(path.clone(), cx)
                            },
                        ))
                    }),
                    RowKind::File => FileTreeRow::file(index, row.name, row.depth)
                        .selected(selected.as_ref() == Some(&row.path))
                        .on_click(view_callback_with(
                            cx,
                            move |this: &mut Self, clicks: usize, _, cx| {
                                let needed = match OPEN_WITH.get(cx) {
                                    OpenWith::OneClick => 1,
                                    OpenWith::DoubleClick => 2,
                                };
                                if clicks >= needed {
                                    this.open_file(path.clone(), cx)
                                }
                            },
                        )),
                }
                .status(status)
                .ignored(row.ignored)
                .guides(guides)
                .into_any_element()
            })
            .collect()
    }

    /// Each uncommitted file's status, by its path: the worktree's side, or else the index's.
    fn file_statuses(&self, cx: &App) -> HashMap<PathBuf, FileStatus> {
        self.changes(cx)
            .uncommitted
            .iter()
            .filter_map(|file| Some((file.path.clone(), file.worktree.or(file.index)?)))
            .collect()
    }

    fn toggle_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !self.files_open.remove(&path) {
            self.files_open.insert(path);
        }
        cx.notify();
    }

    fn open_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let key = FileKey {
            source: Source::Uncommitted,
            path,
        };
        self.select(key, None, cx);
    }

    /// Opens the folders down to the file the editor shows, and scrolls the tree to it.
    pub(super) fn locate_open_file(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self
            .selected
            .as_ref()
            .filter(|key| key.source == Source::Uncommitted)
            .map(|key| key.path.clone())
        else {
            return;
        };
        self.files_open.extend(ancestors(&path));
        let Some(files) = self.projects.read(cx).files(&self.worktree) else {
            return;
        };
        let options = tree_options(cx);
        let index = rows(files, &self.files_open, "", &options)
            .iter()
            .position(|row| row.path == path);
        if let Some(index) = index {
            self.files_scroll.scroll_to_item(index);
        }
        cx.notify();
    }

    #[cfg(feature = "mocks")]
    pub fn set_file_filter(&mut self, text: &str, cx: &mut Context<Self>) {
        self.file_filter
            .update(cx, |input, cx| input.set_text(text.to_string(), cx));
    }

    /// When following the open file, opens the folders down to the file just selected.
    pub(super) fn follow(&mut self, path: &Path, cx: &App) {
        if FOLLOW.get(cx) {
            self.files_open.extend(ancestors(path));
        }
    }
}

/// The tree's options, from the panel's settings.
fn tree_options(cx: &App) -> TreeOptions {
    TreeOptions {
        dotfiles: DOTFILES.get(cx),
        ignored: IGNORED.get(cx),
        fold: FOLD.get(cx),
        sort: SORT.get(cx),
    }
}

/// The status a folder shows for the changes inside it: modified when any file changed, or added
/// when every change is new.
fn folder_status(statuses: &HashMap<PathBuf, FileStatus>, folder: &Path) -> Option<FileStatus> {
    let inside: Vec<FileStatus> = statuses
        .iter()
        .filter(|(path, _)| path.starts_with(folder))
        .map(|(_, status)| *status)
        .collect();
    if inside.is_empty() {
        None
    } else if inside
        .iter()
        .all(|status| matches!(status, FileStatus::Added | FileStatus::Untracked))
    {
        Some(FileStatus::Added)
    } else {
        Some(FileStatus::Modified)
    }
}

/// How `status` shows in the tree, or nothing with Git status off.
fn tree_status(status: FileStatus, style: GitStatus) -> Option<TreeStatus> {
    let (letter, color): (char, fn(&ThemeColors) -> Hsla) = match status {
        FileStatus::Added => ('A', |colors| colors.diff_added),
        FileStatus::Untracked => ('U', |colors| colors.diff_added),
        FileStatus::Modified => ('M', |colors| colors.diff_modified),
        FileStatus::Renamed => ('R', |colors| colors.diff_modified),
        FileStatus::Deleted => ('D', |colors| colors.diff_removed),
        FileStatus::Conflicted => ('C', |colors| colors.diff_removed),
    };
    match style {
        GitStatus::Color => Some(TreeStatus::Color(color)),
        GitStatus::Letter => Some(TreeStatus::Letter(letter, color)),
        GitStatus::Off => None,
    }
}

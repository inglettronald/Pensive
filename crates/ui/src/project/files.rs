//! The rows of a list of changed files, as a list, a tree or by symbol.

use components::{DiffStat, Icon, IconName, ListRow, RingFill, StatusRing, Tooltip};
use gpui::{AnyElement, App, Context, ElementId, Hsla, Window, div, prelude::*, px};
use model::{FileChange, FileStatus, SymbolKind};
use theme::{ActiveTheme, ThemeColors};

use crate::{view_callback, view_callback_with};

use super::{
    FileKey, FileLayout, ProjectView, Source,
    tree::{TreeRow, tree},
};

impl ProjectView {
    pub(super) fn file_rows(
        &self,
        source: &Source,
        files: &[FileChange],
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let key = |index| FileKey {
            source: source.clone(),
            index,
        };
        match self.layout {
            FileLayout::List => files
                .iter()
                .enumerate()
                .map(|(index, file)| self.file_row(key(index), file, 0, cx).into_any_element())
                .collect(),
            FileLayout::Tree => tree(files.iter().map(|file| file.path.as_path()))
                .into_iter()
                .enumerate()
                .map(|(position, row)| match row {
                    TreeRow::Folder { depth, name } => ListRow::new(("folder", position), name)
                        .depth(depth)
                        .disclosure(true)
                        .muted()
                        .into_any_element(),
                    TreeRow::File { depth, index } => self
                        .file_row(key(index), &files[index], depth, cx)
                        .into_any_element(),
                })
                .collect(),
            FileLayout::Symbols => {
                let mut rows = Vec::new();
                for (index, file) in files.iter().enumerate() {
                    let key = key(index);
                    let open = !self.folded.contains(&key);
                    let toggle = view_callback(cx, {
                        let key = key.clone();
                        move |this: &mut Self, _: &mut Window, cx: &mut Context<Self>| {
                            this.toggle_folded(key.clone(), cx)
                        }
                    });
                    rows.push(
                        self.file_row(key.clone(), file, 0, cx)
                            .disclosure(open)
                            .on_disclosure(toggle)
                            .into_any_element(),
                    );
                    if open {
                        for symbol in 0..file.symbols.len() {
                            rows.push(self.symbol_row(&key, file, symbol, cx));
                        }
                    }
                }
                rows
            }
        }
    }

    fn file_row(
        &self,
        key: FileKey,
        file: &FileChange,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> ListRow {
        let uncommitted = key.source == Source::Uncommitted;
        let index = key.index;
        let ring = StatusRing::new(
            ("ring", index),
            if file.staged {
                RingFill::Full
            } else {
                RingFill::Empty
            },
        )
        .color(status_color(file.status));
        let ring = if uncommitted {
            ring.on_click(view_callback(cx, move |this: &mut Self, _, cx| {
                this.toggle_staged(index, cx)
            }))
        } else {
            ring
        };
        let selected = self.selected.as_ref() == Some(&key) && self.selected_symbol.is_none();
        let row = ListRow::new(element_id("file", &key), file.name())
            .depth(depth)
            .leading(ring)
            .trailing(DiffStat::new(file.added, file.removed))
            .selected(selected)
            .hovered(self.hovered.as_ref() == Some(&key))
            .on_click(view_callback(cx, {
                let key = key.clone();
                move |this: &mut Self, _: &mut Window, cx: &mut Context<Self>| {
                    this.select(key.clone(), None, cx)
                }
            }))
            .on_hover(view_callback_with(cx, {
                let key = key.clone();
                move |this: &mut Self, hovered: bool, _: &mut Window, cx: &mut Context<Self>| {
                    this.hover(key.clone(), hovered, cx)
                }
            }));
        if self.tooltip.as_ref() == Some(&key) {
            row.below(Tooltip::new(path_label(file, cx)).shortcut("Copy", "Ctrl Shift C"))
        } else {
            row
        }
    }

    fn symbol_row(
        &self,
        key: &FileKey,
        file: &FileChange,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let symbol = &file.symbols[index];
        let icon = match symbol.kind {
            SymbolKind::Function => IconName::Braces,
            SymbolKind::Type => IconName::Box,
            SymbolKind::Heading => IconName::Hash,
        };
        let selected = self.selected.as_ref() == Some(key) && self.selected_symbol == Some(index);
        ListRow::new(
            ElementId::Name(format!("{}-symbol-{index}", element_name(key)).into()),
            symbol.name.clone(),
        )
        .depth(1)
        // Past the file's arrow, so the symbol's icon lines up under the file's ring.
        .indent(ListRow::SLOT)
        .compact()
        .muted()
        .when(symbol.kind != SymbolKind::Heading, ListRow::code)
        .leading(
            Icon::new(icon)
                .size(px(12.))
                .color(cx.theme().colors.text_faint),
        )
        .trailing(DiffStat::new(symbol.added, symbol.removed))
        .selected(selected)
        .on_click(view_callback(cx, {
            let key = key.clone();
            move |this: &mut Self, _: &mut Window, cx: &mut Context<Self>| {
                this.select(key.clone(), Some(index), cx)
            }
        }))
        .into_any_element()
    }
}

pub(super) fn status_color(status: FileStatus) -> fn(&ThemeColors) -> Hsla {
    match status {
        FileStatus::Added => |colors| colors.diff_added,
        FileStatus::Modified => |colors| colors.diff_modified,
        FileStatus::Deleted => |colors| colors.diff_removed,
    }
}

/// The file's folder in faint text, then its name.
fn path_label(file: &FileChange, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let folder = file.folder();
    div()
        .flex()
        .font_family(theme.fonts.code.clone())
        .when(!folder.is_empty(), |label| {
            label.child(
                div()
                    .text_color(theme.colors.text_faint)
                    .child(format!("{folder}/")),
            )
        })
        .child(
            div()
                .text_color(theme.colors.text_strong)
                .child(file.name()),
        )
}

fn element_name(key: &FileKey) -> String {
    match &key.source {
        Source::Uncommitted => format!("uncommitted-{}", key.index),
        Source::Commit(sha) => format!("{sha}-{}", key.index),
    }
}

fn element_id(kind: &str, key: &FileKey) -> ElementId {
    ElementId::Name(format!("{}-{kind}", element_name(key)).into())
}

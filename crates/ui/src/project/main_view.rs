//! The main view: where the selected file is, how it's shown, then the file in the editor.

use components::{Breadcrumb, DiffStat, Icon, IconName, SegmentedControl};
use gpui::{AnyElement, App, Context, div, prelude::*};
use model::{FileChange, Source, short_sha};
use strum::VariantArray;
use theme::{ActiveTheme, Typeset, dp};

use super::ProjectView;
use crate::{editor::Mode, view_callback_with};

impl ProjectView {
    pub(super) fn render_main_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let selected = self
            .selected
            .as_ref()
            .and_then(|key| Some((key, self.file(key, cx)?)));
        let Some((key, file)) = selected else {
            return div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .bg(colors.surface_app)
                .typeset(density.small)
                .text_color(colors.text_faint)
                .child("Nothing selected");
        };
        let editor = self.editor.read(cx);
        let mode = editor.mode();
        let unsaved = editor
            .document()
            .is_some_and(|document| document.read(cx).is_dirty());
        let binary = self.projects.read(cx).is_binary(&self.worktree, key);
        let editor = self.editor.clone();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .bg(colors.surface_app)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_2()
                    .h(density.row_height + dp(8.))
                    .pl_3()
                    .pr_2()
                    .border_b_1()
                    .border_color(colors.border_subtle)
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .child(
                        div().flex().flex_1().min_w_0().child(
                            Breadcrumb::new(self.crumbs(&key.source, &file, unsaved, cx))
                                .fade_start(|colors| colors.surface_app),
                        ),
                    )
                    .child(
                        SegmentedControl::icons(
                            "editor-mode",
                            Mode::VARIANTS.iter().map(|mode| mode.icon()),
                            Mode::VARIANTS.iter().position(|&each| each == mode),
                        )
                        .tooltips(Mode::VARIANTS.iter().map(|mode| mode.label()))
                        .on_select(view_callback_with(
                            cx,
                            move |this: &mut Self, index: usize, _, cx| {
                                this.editor.update(cx, |editor, cx| {
                                    editor.set_mode(Mode::VARIANTS[index], cx)
                                })
                            },
                        )),
                    )
                    .child(DiffStat::new(file.added, file.removed)),
            )
            .child(div().flex_1().min_h_0().map(|body| {
                if binary {
                    body.p_3()
                        .typeset(density.small)
                        .text_color(colors.text_faint)
                        .child("Binary file")
                } else {
                    body.child(editor)
                }
            }))
    }

    /// The commit, if the file is in one, its folders, its name with whether it has unsaved
    /// edits, then the selected symbol.
    fn crumbs(
        &self,
        source: &Source,
        file: &FileChange,
        unsaved: bool,
        cx: &App,
    ) -> Vec<AnyElement> {
        let theme = cx.theme();
        let colors = &theme.colors;
        let symbol = self
            .selected_symbol
            .and_then(|index| file.symbols.get(index));
        let mut crumbs = Vec::new();
        if let Source::Commit(sha) = source {
            crumbs.push(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Icon::new(IconName::GitCommitHorizontal)
                            .size(dp(12.))
                            .color(colors.text_faint),
                    )
                    .child(
                        div()
                            .font_family(theme.fonts.code.clone())
                            .text_color(colors.text_muted)
                            .child(short_sha(sha).to_owned()),
                    )
                    .into_any_element(),
            );
        }
        if let Some(folder) = file.path.parent() {
            crumbs.extend(folder.components().map(|folder| {
                div()
                    .child(folder.as_os_str().to_string_lossy().into_owned())
                    .into_any_element()
            }));
        }
        crumbs.push(
            div()
                .flex()
                .items_center()
                .text_color(if symbol.is_some() {
                    colors.text_muted
                } else {
                    colors.text_strong
                })
                .child(file.name())
                .when(unsaved, |name| {
                    name.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(dp(5.))
                            .ml(dp(6.))
                            .text_color(colors.text_muted)
                            .child(div().size(dp(6.)).rounded_full().bg(colors.text_muted))
                            .child("Unsaved"),
                    )
                })
                .into_any_element(),
        );
        if let Some(symbol) = symbol {
            crumbs.push(
                div()
                    .font_family(theme.fonts.code.clone())
                    .text_color(colors.text_strong)
                    .child(symbol.name.clone())
                    .into_any_element(),
            );
        }
        crumbs
    }
}

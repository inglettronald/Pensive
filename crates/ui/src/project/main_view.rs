//! The main view: where the selected file is, then its diff.

use components::{Breadcrumb, Diff, DiffLine, DiffStat, Icon, IconName};
use gpui::{AnyElement, Context, div, prelude::*, px};
use model::{FileChange, LineChange};
use theme::{ActiveTheme, Typeset};

use super::{ProjectView, Source};

impl ProjectView {
    pub(super) fn render_main_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let selected = self
            .selected
            .as_ref()
            .and_then(|key| Some((key.source.clone(), self.file(key, cx)?)));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .bg(colors.surface_app)
            .map(|view| match selected {
                Some((source, file)) => view
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap_2()
                            .h(density.row_height + px(8.))
                            .pl_3()
                            .pr_2()
                            .border_b_1()
                            .border_color(colors.border_subtle)
                            .typeset(density.small)
                            .text_color(colors.text_faint)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(Breadcrumb::new(self.crumbs(&source, &file, cx))),
                            )
                            .child(DiffStat::new(file.added, file.removed)),
                    )
                    .child(
                        div()
                            .id("diff")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(Diff::new(diff_lines(&file))),
                    ),
                None => view.items_center().justify_center().child(
                    div()
                        .typeset(density.small)
                        .text_color(colors.text_faint)
                        .child("Nothing selected"),
                ),
            })
    }

    /// The commit, if the file is in one, its folders, its name, then the selected symbol.
    fn crumbs(&self, source: &Source, file: &FileChange, cx: &Context<Self>) -> Vec<AnyElement> {
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
                            .size(px(12.))
                            .color(colors.text_faint),
                    )
                    .child(
                        div()
                            .font_family(theme.fonts.code.clone())
                            .text_color(colors.text_muted)
                            .child(sha.clone()),
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
                .text_color(if symbol.is_some() {
                    colors.text_muted
                } else {
                    colors.text_strong
                })
                .child(file.name())
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

fn diff_lines(file: &FileChange) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    for hunk in &file.hunks {
        lines.push(DiffLine::hunk(hunk.header.clone()));
        for line in &hunk.lines {
            let diff_line = match (line.change, line.number) {
                (LineChange::Added, Some(number)) => DiffLine::added(number, line.text.clone()),
                (LineChange::Context, Some(number)) => DiffLine::context(number, line.text.clone()),
                _ => DiffLine::removed(line.text.clone()),
            };
            lines.push(
                line.changed
                    .iter()
                    .cloned()
                    .fold(diff_line, DiffLine::changed),
            );
        }
    }
    lines
}

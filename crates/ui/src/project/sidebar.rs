//! The sidebar: pull and push in its header, the branch as a timeline, and the commit box pinned
//! under it.

use components::{
    Button, ButtonStyle, FadeText, Icon, IconButton, IconName, RingFill, SegmentedControl,
    StatusRing, TimelineLine, TimelineNode, TimelineRow,
};
use gpui::{AnyElement, Context, SharedString, Window, div, prelude::*, px};
use model::{Changes, Commit};
use strum::VariantArray;
use theme::{ActiveTheme, Typeset};

use super::{FileLayout, ProjectView, Source};
use crate::{view_callback, view_callback_with};

/// The sidebar's width. It becomes resizable later.
const WIDTH: f32 = 280.;

impl ProjectView {
    pub(super) fn render_sidebar(
        &mut self,
        changes: &Changes,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = &cx.theme().colors;
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(WIDTH))
            .border_r_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_panel)
            .child(self.header(changes, cx))
            .child(
                div()
                    .id("timeline")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .pl_1()
                    .pr_2()
                    .overflow_y_scroll()
                    .child(self.uncommitted(changes, cx))
                    .children(self.uncommitted_files(changes, cx))
                    .children(self.history(changes, cx))
                    .child(
                        TimelineRow::new()
                            .h(px(24.))
                            .above(TimelineLine::Fade)
                            .node_y(px(24.)),
                    ),
            )
            .child(self.commit_box(changes, cx))
    }

    /// How far the branch is from its base, then fetch, pull and push.
    fn header(&self, changes: &Changes, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let upstream = changes.upstream.as_ref();
        let behind = upstream.map_or(0, |upstream| upstream.behind);
        let ahead = upstream.map_or(0, |upstream| upstream.ahead);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .h(density.row_height + px(12.))
            .pl_3()
            .pr_2()
            .border_b_1()
            .border_color(colors.border_subtle)
            .child(
                Icon::new(IconName::Cloud)
                    .size(px(14.))
                    .color(colors.text_faint),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .ml_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .children(
                        changes
                            .base
                            .as_ref()
                            .map(|(base, ahead)| format!("{ahead} ahead of {base}")),
                    ),
            )
            // Fetch, pull and push wait on a Git backend.
            .child(IconButton::new("fetch", IconName::RefreshCw))
            .when(behind > 0, |header| {
                header.child(
                    Button::new("pull", behind.to_string())
                        .icon(IconName::ArrowDownToLine)
                        .small(),
                )
            })
            .when(ahead > 0, |header| {
                header.child(
                    Button::new("push", ahead.to_string())
                        .icon(IconName::ArrowUpFromLine)
                        .small(),
                )
            })
    }

    /// The node that stages everything, the label that folds the files, and the layout switcher.
    fn uncommitted(&self, changes: &Changes, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let total = changes.uncommitted.len();
        let staged = changes.staged_count();
        let fill = match staged {
            0 => RingFill::Empty,
            staged if staged == total => RingFill::Full,
            _ => RingFill::Half,
        };
        let ring = StatusRing::new("stage-all", fill)
            .color(|colors| colors.diff_modified)
            .size(px(14.));
        let ring = if total > 0 {
            ring.on_click(view_callback(cx, |this: &mut Self, _, cx| {
                this.toggle_all_staged(cx)
            }))
        } else {
            ring
        };
        let open = self.uncommitted_open;
        let layout = FileLayout::VARIANTS
            .iter()
            .position(|layout| *layout == self.layout);
        TimelineRow::new()
            .items_center()
            .h(density.row_height + px(8.))
            .below(TimelineLine::Dashed)
            .node(TimelineNode::Mark(ring.into_any_element()))
            .child(
                div()
                    .id("uncommitted")
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_1()
                    .h(density.row_height)
                    .ml_1()
                    .px_1()
                    .rounded(density.radius_md)
                    .hover(|label| label.bg(colors.surface_raised))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.set_uncommitted_open(!open, cx)),
                    )
                    .child(
                        div()
                            .typeset(density.body)
                            .text_color(colors.text_strong)
                            .child("Uncommitted"),
                    )
                    .when(total > 0, |label| {
                        label.child(
                            Icon::new(if open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .size(px(12.))
                            .color(colors.text_faint),
                        )
                    })
                    .when(!open || total == 0, |label| {
                        label.child(
                            div()
                                .ml_0p5()
                                .typeset(density.small)
                                .text_color(colors.text_faint)
                                .child(match total {
                                    0 => "none".to_string(),
                                    1 => "1 file".to_string(),
                                    total => format!("{total} files"),
                                }),
                        )
                    }),
            )
            .child(
                SegmentedControl::icons(
                    "file-layout",
                    FileLayout::VARIANTS.iter().map(|layout| layout.icon()),
                    layout,
                )
                .on_select(view_callback_with(
                    cx,
                    |this: &mut Self, index: usize, _, cx| {
                        this.set_layout(FileLayout::VARIANTS[index], cx)
                    },
                )),
            )
    }

    fn uncommitted_files(&self, changes: &Changes, cx: &mut Context<Self>) -> Option<TimelineRow> {
        if !self.uncommitted_open || changes.uncommitted.is_empty() {
            return None;
        }
        Some(
            TimelineRow::new().line(TimelineLine::Dashed).child(
                div()
                    .id("uncommitted-files")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .children(self.file_rows(&Source::Uncommitted, &changes.uncommitted, cx)),
            ),
        )
    }

    /// HEAD, the branch's commits newest first, and origin where pushed work starts.
    fn history(&self, changes: &Changes, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = vec![marker("HEAD", TimelineLine::Dashed, cx).into_any_element()];
        let mut origin_shown = changes.upstream.is_none();
        for commit in &changes.commits {
            if commit.pushed && !origin_shown {
                let remote = changes
                    .upstream
                    .as_ref()
                    .map(|upstream| upstream.remote.clone());
                rows.push(
                    marker(remote.unwrap_or_default(), TimelineLine::Solid, cx).into_any_element(),
                );
                origin_shown = true;
            }
            rows.push(self.commit_row(commit, cx));
        }
        rows
    }

    fn commit_row(&self, commit: &Commit, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors.clone();
        let density = cx.density().clone();
        let open = self.open_commit.as_ref() == Some(&commit.sha);
        let hovered = self.hovered_commit.as_ref() == Some(&commit.sha);
        let sha = commit.sha.clone();
        let title = div()
            .id(sha.clone())
            .flex()
            .gap_1p5()
            .px_1()
            .rounded(density.radius_md)
            .hover(|title| title.bg(colors.surface_raised))
            .on_click(cx.listener({
                let sha = sha.clone();
                move |this, _, _, cx| this.toggle_commit(sha.clone(), cx)
            }))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.hover_commit(sha.clone(), *hovered, cx)
            }))
            .typeset(density.body)
            .text_color(if commit.pushed {
                colors.text_muted
            } else {
                colors.text
            })
            .map(|title| {
                if open {
                    // Open, the whole title shows, wrapped.
                    title.items_start().min_h(density.row_height).py_1().child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.text_strong)
                            .child(commit.title.clone()),
                    )
                } else {
                    let fade = FadeText::new(commit.title.clone());
                    title
                        .items_center()
                        .h(density.row_height)
                        .child(if hovered {
                            fade.background(|colors| colors.surface_raised)
                        } else {
                            fade
                        })
                        .when(commit.description.is_some(), |title| {
                            title.child(description_chip(cx))
                        })
                        .child(div().flex_1())
                }
            })
            .child(
                div()
                    .flex_none()
                    .font_family(cx.theme().fonts.code.clone())
                    .typeset(density.small)
                    .line_height(density.body.line_height)
                    .text_color(colors.text_faint)
                    .child(commit.sha.clone()),
            );

        TimelineRow::new()
            .line(TimelineLine::Solid)
            .node(if open {
                TimelineNode::Open
            } else {
                TimelineNode::Commit {
                    pushed: commit.pushed,
                }
            })
            .node_y(density.row_height / 2.)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .ml_1()
                    .child(title)
                    .when(open, |body| {
                        body.children(commit.description.clone().map(|description| {
                            div()
                                .px_1()
                                .pb_1p5()
                                .typeset(density.small)
                                .text_color(colors.text_muted)
                                .child(description)
                        }))
                        .child(
                            div()
                                .id(SharedString::from(format!("{}-files", commit.sha)))
                                .flex()
                                .flex_col()
                                .pb_2()
                                .children(self.file_rows(
                                    &Source::Commit(commit.sha.clone()),
                                    &commit.files,
                                    cx,
                                )),
                        )
                    }),
            )
            .into_any_element()
    }

    /// The message box and the Commit button, which stays on screen however long the list gets.
    fn commit_box(&self, changes: &Changes, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let staged = changes.staged_count();
        let label = match staged {
            0 => "Nothing staged".to_string(),
            1 => "Commit 1 file".to_string(),
            staged => format!("Commit {staged} files"),
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_2()
            .p_2()
            .border_t_1()
            .border_color(colors.border_subtle)
            .child(
                div()
                    .h(density.row_height * 2. - px(4.))
                    .px_2()
                    .py_1p5()
                    .rounded(density.radius_md)
                    .border_1()
                    .border_color(colors.border_subtle)
                    .bg(colors.surface_raised)
                    .typeset(density.body)
                    .text_color(colors.text_strong)
                    .child(self.message.clone()),
            )
            .child(
                div().flex().justify_end().child(
                    Button::new("commit", label)
                        .style(ButtonStyle::Outline)
                        .shortcut("Ctrl Enter")
                        .disabled(staged == 0)
                        .on_click(|window: &mut Window, cx| {
                            window.dispatch_action(Box::new(super::CommitStaged), cx)
                        }),
                ),
            )
    }
}

/// A tick on the line with a label, such as HEAD where committed work starts.
fn marker(
    label: impl Into<SharedString>,
    above: TimelineLine,
    cx: &mut Context<ProjectView>,
) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let density = cx.density();
    TimelineRow::new()
        .items_center()
        .h(density.row_height + px(4.))
        .above(above)
        .below(TimelineLine::Solid)
        .node(TimelineNode::Tick)
        .child(
            div()
                .ml_2()
                .font_family(cx.theme().fonts.code.clone())
                .typeset(density.small)
                .text_color(colors.text_muted)
                .child(label.into()),
        )
}

/// Marks a commit whose message has more than its title.
fn description_chip(cx: &mut Context<ProjectView>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(18.))
        .h(px(12.))
        .rounded(cx.density().radius_sm)
        .bg(colors.surface_selected)
        .child(
            Icon::new(IconName::Ellipsis)
                .size(px(12.))
                .color(colors.text_muted),
        )
}

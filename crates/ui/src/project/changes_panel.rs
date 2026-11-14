//! The Changes panel: pull and push in its header, the branch as a timeline, and the commit box
//! pinned under it.

use components::{
    Button, ButtonStyle, FadeText, Icon, IconButton, IconName, RingFill, SegmentedControl,
    StatusRing, TimelineLine, TimelineNode, TimelineRow,
};
use gpui::{AnyElement, App, Context, SharedString, Window, deferred, div, prelude::*};
use model::{Changes, Commit, Staging, short_sha};
use strum::VariantArray;
use theme::{ActiveTheme, Typeset, dp};

use super::{Control, FileLayout, ProjectView, Source};
use crate::{PANEL, notices, view_callback, view_callback_with};

impl ProjectView {
    pub(super) fn render_changes_panel(
        &mut self,
        changes: &Changes,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = &cx.theme().colors;
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(PANEL)
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
                            .h(dp(24.))
                            .above(TimelineLine::Fade)
                            .node_y(dp(24.)),
                    ),
            )
            .child(self.commit_box(changes, cx))
    }

    /// How far the branch is from its base, then fetch, pull and push.
    fn header(&self, changes: &Changes, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let (behind, ahead) = match &changes.upstream {
            Some(upstream) => (upstream.behind, changes.ahead_of_upstream()),
            None => (0, 0),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .h(density.row_height + dp(12.))
            .pl_3()
            .pr_2()
            .border_b_1()
            .border_color(colors.border_subtle)
            .child(
                Icon::new(IconName::Cloud)
                    .size(dp(14.))
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
                    .children(changes.base.as_ref().map(|base| {
                        format!("{} ahead of {}", changes.ahead_of_base(), base.branch)
                    })),
            )
            .child(
                self.with_notice(
                    Control::Fetch,
                    IconButton::new("fetch", IconName::RefreshCw)
                        .on_click(self.on_use(Control::Fetch, cx)),
                ),
            )
            .when(behind > 0, |header| {
                header.child(
                    self.with_notice(
                        Control::Pull,
                        Button::new("pull", behind.to_string())
                            .icon(IconName::ArrowDownToLine)
                            .small()
                            .on_click(self.on_use(Control::Pull, cx)),
                    ),
                )
            })
            .when(ahead > 0, |header| {
                header.child(
                    self.with_notice(
                        Control::Push,
                        Button::new("push", ahead.to_string())
                            .icon(IconName::ArrowUpFromLine)
                            .small()
                            .on_click(self.on_use(Control::Push, cx)),
                    ),
                )
            })
    }

    fn on_use(
        &self,
        control: Control,
        cx: &mut Context<Self>,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        view_callback(cx, move |this: &mut Self, _, cx| {
            this.use_control(control, cx)
        })
    }

    /// `control` with its notice in a tooltip, right-aligned to it: below in the header, above at
    /// the bottom of the panel.
    fn with_notice(&self, control: Control, element: impl IntoElement) -> impl IntoElement {
        let notice = self
            .notice
            .as_ref()
            .filter(|(shown, ..)| *shown == control)
            .map(|(_, notice, _)| notices::tooltip(notice));
        div()
            .relative()
            .child(element)
            .children(notice.map(|tooltip| {
                div()
                    .absolute()
                    .right_0()
                    .map(|anchor| {
                        if control == Control::Commit {
                            anchor.bottom_full().mb_1()
                        } else {
                            anchor.top_full().mt_1()
                        }
                    })
                    .child(deferred(tooltip).priority(1))
            }))
    }

    /// The node that stages everything, the label that folds the files, and the layout switcher.
    fn uncommitted(&self, changes: &Changes, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let total = changes.uncommitted.len();
        let fill = match changes.staging() {
            Staging::Unstaged => RingFill::Empty,
            Staging::Partly => RingFill::Half,
            Staging::Staged => RingFill::Full,
        };
        let ring = StatusRing::new("stage-all", fill)
            .color(|colors| colors.diff_modified)
            .size(dp(14.));
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
            .h(density.row_height + dp(8.))
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
                            .size(dp(12.))
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
                .tooltips(FileLayout::VARIANTS.iter().map(|layout| layout.label()))
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
            .ml_1()
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
                    .child(short_sha(&commit.sha).to_owned()),
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
                    .child(title)
                    .when(open, |body| {
                        body.children(commit.description.clone().map(|description| {
                            div()
                                .ml_1()
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
                    .h(density.row_height * 2. - dp(4.))
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
                    self.with_notice(
                        Control::Commit,
                        Button::new("commit", label)
                            .style(ButtonStyle::Outline)
                            .shortcut("Ctrl Enter")
                            .disabled(staged == 0)
                            .on_click(|window: &mut Window, cx| {
                                window.dispatch_action(Box::new(super::CommitStaged), cx)
                            }),
                    ),
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
        .h(density.row_height + dp(4.))
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
        .w(dp(18.))
        .h(dp(12.))
        .rounded(cx.density().radius_sm)
        .bg(colors.surface_selected)
        .child(
            Icon::new(IconName::Ellipsis)
                .size(dp(12.))
                .color(colors.text_muted),
        )
}

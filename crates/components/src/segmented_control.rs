//! A row of mutually exclusive options in a track, labeled with text or icons, with at most one
//! selected. Every option is as wide as the widest, and the selection stretches toward a new
//! option: its leading edge leaves first and the trailing edge follows.

use std::{rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, App, Div, ElementId, SharedString, Window, div, ease_out_quint,
    prelude::*, relative,
};
use theme::{ActiveTheme, Typeset, dp};

use crate::{Callback, Icon, IconName, tooltip::text_tooltip, transition::transition};

/// How long each edge of the selection takes to reach a new option.
const EDGE: Duration = Duration::from_millis(150);
/// How long the trailing edge waits after the leading one leaves.
const LAG: Duration = Duration::from_millis(60);

enum Segment {
    Text(SharedString),
    Icon(IconName),
}

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    options: Vec<Segment>,
    tooltips: Vec<SharedString>,
    selected: Option<usize>,
    compact: bool,
    hovered: Option<usize>,
    on_select: Option<Callback<usize>>,
}

impl SegmentedControl {
    /// A row tall with body text, like the other controls on a settings row.
    pub fn new(
        id: impl Into<ElementId>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: impl Into<Option<usize>>,
    ) -> Self {
        Self {
            id: id.into(),
            options: options
                .into_iter()
                .map(|option| Segment::Text(option.into()))
                .collect(),
            tooltips: Vec::new(),
            selected: selected.into(),
            compact: false,
            hovered: None,
            on_select: None,
        }
    }

    /// Options drawn as icons only, such as views of the same list. Always compact.
    pub fn icons(
        id: impl Into<ElementId>,
        icons: impl IntoIterator<Item = IconName>,
        selected: impl Into<Option<usize>>,
    ) -> Self {
        Self {
            id: id.into(),
            options: icons.into_iter().map(Segment::Icon).collect(),
            tooltips: Vec::new(),
            selected: selected.into(),
            compact: true,
            hovered: None,
            on_select: None,
        }
    }

    /// Small text in a short track, for a header or a search field rather than a settings row.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Names each option in a tooltip once the pointer rests on it, which icon options need.
    pub fn tooltips(mut self, tooltips: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.tooltips = tooltips.into_iter().map(Into::into).collect();
        self
    }

    /// Draws an option as hovered, for a mock. The pointer does the same.
    pub fn hovered(mut self, hovered: Option<usize>) -> Self {
        self.hovered = hovered;
        self
    }

    /// Called with the index of the option that was clicked.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let change = transition(&self.id, self.selected, window, cx);
        let colors = &cx.theme().colors;
        let density = cx.density();
        let count = self.options.len().max(1) as f32;
        let group: SharedString = format!("{}-selection", self.id).into();
        let (lit_fill, lit_edge) = (colors.border_subtle, colors.text_disabled);
        // Spans the options from `start` to `end`, counted in options.
        let place = move |pill: Div, start: f32, end: f32| {
            pill.left(relative(start / count))
                .right(relative(1. - end / count))
        };
        let selection = self.selected.map(|selected| {
            let lit = self.hovered == Some(selected);
            let pill = div()
                .absolute()
                .top_0()
                .bottom_0()
                .rounded(density.radius_sm)
                .border_1()
                .border_color(if lit { lit_edge } else { colors.border })
                .bg(if lit {
                    lit_fill
                } else {
                    colors.surface_selected
                })
                .group_hover(group.clone(), |pill| {
                    pill.bg(lit_fill).border_color(lit_edge)
                });
            match (change.from, change.to) {
                (Some(from), Some(to)) if change.moved() => {
                    let total = EDGE + LAG;
                    let ease = ease_out_quint();
                    pill.with_animation(
                        change.animation_id(&self.id, "selection"),
                        Animation::new(total),
                        move |pill, delta| {
                            let elapsed = total.mul_f32(delta);
                            let progress = |since: Duration| {
                                ease(
                                    (elapsed.saturating_sub(since).as_secs_f32()
                                        / EDGE.as_secs_f32())
                                    .min(1.),
                                )
                            };
                            let (lead, trail) = (progress(Duration::ZERO), progress(LAG));
                            let (from, to) = (from as f32, to as f32);
                            let distance = to - from;
                            let (start, end) = if distance > 0. {
                                (from + distance * trail, from + 1. + distance * lead)
                            } else {
                                (from + distance * lead, from + 1. + distance * trail)
                            };
                            place(pill, start, end)
                        },
                    )
                    .into_any_element()
                }
                _ => place(pill, selected as f32, selected as f32 + 1.).into_any_element(),
            }
        });
        let on_select = self.on_select;
        let mut tooltips = self.tooltips.into_iter();
        let segments = self.options.into_iter().enumerate().map(|(index, option)| {
            let selected = self.selected == Some(index);
            let hovered = !selected && self.hovered == Some(index);
            let segment = div()
                .id(index)
                .flex()
                .items_center()
                .justify_center()
                .h_full()
                .rounded(density.radius_sm)
                .when(selected, |segment| segment.group(group.clone()))
                .when(hovered, |segment| segment.bg(colors.surface_raised))
                .when(!selected, |segment| {
                    segment
                        .hover(|segment| segment.bg(colors.surface_raised).text_color(colors.text))
                });
            match option {
                Segment::Text(text) => segment
                    .when_else(
                        self.compact,
                        |segment| segment.px_2(),
                        |segment| segment.min_w(dp(64.)).px_2p5(),
                    )
                    .whitespace_nowrap()
                    .text_color(if selected {
                        colors.text_strong
                    } else if hovered {
                        colors.text
                    } else {
                        colors.text_muted
                    })
                    .child(text),
                Segment::Icon(icon) => {
                    segment
                        .w(dp(24.))
                        .child(Icon::new(icon).size(dp(14.)).color(if selected {
                            colors.text_strong
                        } else if hovered {
                            colors.text
                        } else {
                            colors.text_faint
                        }))
                }
            }
            .when_some(tooltips.next(), |option, tooltip| {
                option.tooltip(text_tooltip(tooltip))
            })
            .when_some(on_select.clone(), |option, handler| {
                option.on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    handler(index, window, cx)
                })
            })
        });
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .p_0p5()
            .rounded(density.radius_md)
            .border_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_app)
            .when_else(
                self.compact,
                |track| track.typeset(density.small).h(dp(26.)),
                |track| track.typeset(density.body).h(density.row_height),
            )
            .child(
                div()
                    .relative()
                    .grid()
                    .grid_cols(count as u16)
                    .h_full()
                    .children(selection)
                    .children(segments),
            )
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("segmented_control", "gallery", || {
            const OPTIONS: [&str; 3] = ["Off", "New file", "Both"];
            crate::Gallery::new("SegmentedControl")
                .section(
                    "Selected",
                    OPTIONS.iter().enumerate().map(|(index, &label)| {
                        (
                            label,
                            SegmentedControl::new(label, OPTIONS, index).into_any_element(),
                        )
                    }),
                )
                .section(
                    "Hovered",
                    [("Another option", 2), ("The selection", 1)].map(|(label, hovered)| {
                        (
                            label,
                            SegmentedControl::new(label, OPTIONS, 1)
                                .hovered(Some(hovered))
                                .into_any_element(),
                        )
                    }),
                )
                .section(
                    "Compact",
                    [("Tab strip", 0)].map(|(label, index)| {
                        (
                            label,
                            SegmentedControl::new(label, ["All", "Active", "Commands"], index)
                                .compact()
                                .into_any_element(),
                        )
                    }),
                )
                .section(
                    "Icons",
                    [("List", 0), ("Tree", 1), ("Symbols", 2)].map(|(label, index)| {
                        let icons = [IconName::List, IconName::ListTree, IconName::Braces];
                        (
                            label,
                            SegmentedControl::icons(label, icons, index).into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .size(640., 480.),
    ]
}

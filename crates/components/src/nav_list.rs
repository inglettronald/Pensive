//! The list beside the settings rail: a category's pages on a guide line, with the current page
//! highlighted and the highlight sliding when it changes.

use std::{ops::Range, rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, App, ElementId, FontWeight, HighlightStyle, SharedString, StyledText,
    Window, div, ease_out_quint, prelude::*,
};
use theme::{ActiveTheme, Dp, HAIRLINE, Typeset, dp};

use crate::Callback;

/// How long the highlight takes to reach a new page.
const SLIDE: Duration = Duration::from_millis(150);
/// Where the guide runs, from a row's left edge.
const GUIDE: Dp = dp(10.);
/// Where a label starts, past the guide.
const LABEL: Dp = dp(22.);

/// One row of the list.
pub struct NavItem {
    pub label: SharedString,
    /// Byte ranges of `label` that matched a search.
    pub matches: Vec<Range<usize>>,
    /// Faint text at the right edge, such as the category of a search result.
    pub trailing: Option<SharedString>,
}

impl NavItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            matches: Vec::new(),
            trailing: None,
        }
    }
}

#[derive(IntoElement)]
pub struct NavList {
    id: ElementId,
    items: Vec<NavItem>,
    selected: Option<usize>,
    /// The row the highlight slides from, when the selection just changed.
    previous: Option<usize>,
    hovered: Option<usize>,
    on_select: Option<Callback<usize>>,
}

impl NavList {
    pub fn new(id: impl Into<ElementId>, items: Vec<NavItem>) -> Self {
        Self {
            id: id.into(),
            items,
            selected: None,
            previous: None,
            hovered: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Slides the highlight from `previous` to the selected row.
    pub fn previous(mut self, previous: Option<usize>) -> Self {
        self.previous = previous;
        self
    }

    /// Shows the hover state without a pointer, for mocks.
    pub fn hovered(mut self, hovered: Option<usize>) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for NavList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let height = density.row_height;
        let guide = |color| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(GUIDE)
                .w(HAIRLINE)
                .bg(color)
        };
        let highlight = self.selected.map(|to| {
            let top = move |index: usize| height * index as f32;
            let highlight = div()
                .absolute()
                .left_0()
                .right_0()
                .h(height)
                .rounded(density.radius_md)
                .bg(colors.surface_selected)
                .child(guide(colors.text_strong));
            match self.previous.filter(|from| *from != to) {
                Some(from) => highlight
                    .with_animation(
                        ("highlight", from * 1000 + to),
                        Animation::new(SLIDE).with_easing(ease_out_quint()),
                        move |highlight, delta| {
                            highlight.top(top(from) + (top(to) - top(from)) * delta)
                        },
                    )
                    .into_any_element(),
                None => highlight.top(top(to)).into_any_element(),
            }
        });
        let match_style = HighlightStyle {
            color: Some(colors.accent_fg),
            ..Default::default()
        };
        let rows = self.items.into_iter().enumerate().map(|(index, item)| {
            let selected = self.selected == Some(index);
            let on_select = self.on_select.clone();
            div()
                .id(index)
                .relative()
                .flex()
                .items_center()
                .gap_2()
                .h(height)
                .pl(LABEL)
                .pr_2p5()
                .rounded(density.radius_md)
                .when(!selected, |row| {
                    row.child(guide(colors.border_subtle))
                        .when(self.hovered == Some(index), |row| {
                            row.bg(colors.surface_raised)
                        })
                        .hover(|row| row.bg(colors.surface_raised))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .typeset(density.body)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if selected {
                            colors.text_strong
                        } else {
                            colors.text
                        })
                        .child(StyledText::new(item.label).with_highlights(
                            item.matches.into_iter().map(|range| (range, match_style)),
                        )),
                )
                .children(item.trailing.map(|trailing| {
                    div()
                        .flex_none()
                        .typeset(density.small)
                        .text_color(colors.text_faint)
                        .child(trailing)
                }))
                .when_some(on_select, |row, handler| {
                    row.on_click(move |_, window, cx| handler(index, window, cx))
                })
        });
        div()
            .id(self.id)
            .relative()
            .flex()
            .flex_col()
            .mx_2()
            .children(highlight)
            .children(rows)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::AnyElement;

    fn list(selected: Option<usize>, hovered: Option<usize>) -> AnyElement {
        div()
            .w(dp(228.))
            .child(
                NavList::new("pages", ["Theme", "Size", "Fonts"].map(NavItem::new).into())
                    .selected(selected)
                    .hovered(hovered),
            )
            .into_any_element()
    }
    fn results() -> AnyElement {
        let item = |label: &str, matched: Range<usize>, trailing: &str| NavItem {
            label: label.to_string().into(),
            matches: vec![matched],
            trailing: Some(trailing.to_string().into()),
        };
        div()
            .w(dp(228.))
            .child(
                NavList::new(
                    "results",
                    vec![
                        item("Show tabs", 0..4, "Command palette"),
                        item("Toast duration", 0..5, "Notices"),
                    ],
                )
                .selected(Some(0)),
            )
            .into_any_element()
    }

    vec![
        mock::Mock::component("nav_list", "gallery", || {
            crate::Gallery::new("NavList")
                .section(
                    "States",
                    [
                        ("Size selected", list(Some(1), None)),
                        ("Hovering Fonts", list(Some(0), Some(2))),
                        ("Search results", results()),
                    ],
                )
                .into_any_element()
        })
        .size(800., 240.),
    ]
}

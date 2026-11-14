//! One row of a timeline: the line down its left edge, what sits on the line, then the row's
//! content.

use gpui::{
    AnyElement, App, Bounds, Div, Hsla, StyleRefinement, Window, canvas, div, fill,
    linear_color_stop, linear_gradient, point, prelude::*, relative, size,
};
use strum::{IntoStaticStr, VariantArray};
use theme::{ActiveTheme, Dp, dp};

/// The width of the line's column. The content starts after it.
const LINE_COLUMN: Dp = dp(24.);
const LINE: Dp = dp(2.);
const DASH: Dp = dp(3.);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, VariantArray, IntoStaticStr)]
pub enum TimelineLine {
    #[default]
    None,
    Solid,
    /// Work that isn't committed yet.
    Dashed,
    /// The end of what's shown, fading out.
    Fade,
}

pub enum TimelineNode {
    None,
    /// A commit, dimmer once it's pushed.
    Commit {
        pushed: bool,
    },
    /// The commit that's open, ringed.
    Open,
    /// A marker such as HEAD or origin.
    Tick,
    /// Anything else, centered on the line, such as a status ring.
    Mark(AnyElement),
}

/// Lays its content out in a row after the line, and takes styles like a `div`.
#[derive(IntoElement)]
pub struct TimelineRow {
    base: Div,
    above: TimelineLine,
    below: TimelineLine,
    node: TimelineNode,
    node_y: Option<Dp>,
    children: Vec<AnyElement>,
}

impl Default for TimelineRow {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelineRow {
    pub fn new() -> Self {
        Self {
            base: div(),
            above: TimelineLine::None,
            below: TimelineLine::None,
            node: TimelineNode::None,
            node_y: None,
            children: Vec::new(),
        }
    }

    /// The same line above and below the node.
    pub fn line(self, line: TimelineLine) -> Self {
        self.above(line).below(line)
    }

    pub fn above(mut self, line: TimelineLine) -> Self {
        self.above = line;
        self
    }

    pub fn below(mut self, line: TimelineLine) -> Self {
        self.below = line;
        self
    }

    pub fn node(mut self, node: TimelineNode) -> Self {
        self.node = node;
        self
    }

    /// Where the node sits, from the top. Centered unless set, which suits a row that can grow.
    pub fn node_y(mut self, y: Dp) -> Self {
        self.node_y = Some(y);
        self
    }
}

impl Styled for TimelineRow {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for TimelineRow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for TimelineRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let line_color = colors.border;
        let segment = |line: TimelineLine| {
            let base = div().absolute().left(dp(11.)).w(LINE);
            match line {
                TimelineLine::None => None,
                TimelineLine::Solid => Some(base.bg(line_color)),
                TimelineLine::Fade => Some(base.bg(linear_gradient(
                    180.,
                    linear_color_stop(line_color, 0.),
                    linear_color_stop(line_color.opacity(0.), 1.),
                ))),
                TimelineLine::Dashed => Some(base.child(dashes(line_color))),
            }
        };
        let above = segment(self.above).map(|line| match self.node_y {
            Some(y) => line.top_0().h(y),
            None => line.top_0().h(relative(0.5)),
        });
        let below = segment(self.below).map(|line| match self.node_y {
            Some(y) => line.top(y).bottom_0(),
            None => line.top(relative(0.5)).bottom_0(),
        });
        let dot = |size: Dp, color: Hsla| div().size(size).rounded_full().bg(color);
        let node = match self.node {
            TimelineNode::None => None,
            TimelineNode::Commit { pushed } => Some(
                dot(
                    dp(10.),
                    if pushed {
                        colors.text_faint
                    } else {
                        colors.text_strong
                    },
                )
                .into_any_element(),
            ),
            TimelineNode::Open => Some(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(dp(18.))
                    .rounded_full()
                    .border_1()
                    .border_color(colors.text_muted)
                    .bg(colors.surface_panel)
                    .child(dot(dp(12.), colors.text_strong))
                    .into_any_element(),
            ),
            TimelineNode::Tick => Some(
                div()
                    .w(dp(16.))
                    .h(LINE)
                    .bg(colors.text_muted)
                    .into_any_element(),
            ),
            TimelineNode::Mark(mark) => Some(mark),
        };

        let column = div()
            .relative()
            .flex_none()
            .self_stretch()
            .w(LINE_COLUMN)
            .children(above)
            .children(below)
            .children(node.map(|node| {
                // A zero-height row at the node's height, so any node centers on it.
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .h_0()
                    .map(|row| match self.node_y {
                        Some(y) => row.top(y),
                        None => row.top(relative(0.5)),
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(node)
            }));
        self.base
            .flex()
            .flex_none()
            .child(column)
            .children(self.children)
    }
}

/// Short dashes down the whole height. GPUI has no dashed borders, so they're painted.
fn dashes(color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let dash = DASH.to_pixels(window.rem_size());
            let mut y = bounds.top();
            while y < bounds.bottom() {
                let height = dash.min(bounds.bottom() - y);
                window.paint_quad(fill(
                    Bounds::new(point(bounds.left(), y), size(bounds.size.width, height)),
                    color,
                ));
                y += dash * 2.;
            }
        },
    )
    .size_full()
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use crate::{RingFill, StatusRing, gallery::Panel};

    #[derive(Clone, Copy, VariantArray, IntoStaticStr)]
    enum Shown {
        Nothing,
        Commit,
        Pushed,
        Open,
        Tick,
        Ring,
    }

    vec![
        mock::Mock::component("timeline_row", "gallery", || {
            crate::Gallery::new("TimelineRow")
                .matrix(
                    "Line and node",
                    TimelineLine::VARIANTS,
                    Shown::VARIANTS,
                    |line, shown| {
                        let node = match shown {
                            Shown::Nothing => TimelineNode::None,
                            Shown::Commit => TimelineNode::Commit { pushed: false },
                            Shown::Pushed => TimelineNode::Commit { pushed: true },
                            Shown::Open => TimelineNode::Open,
                            Shown::Tick => TimelineNode::Tick,
                            Shown::Ring => TimelineNode::Mark(
                                StatusRing::new("ring", RingFill::Half)
                                    .color(|colors| colors.diff_modified)
                                    .size(dp(14.))
                                    .into_any_element(),
                            ),
                        };
                        Panel::new(
                            LINE_COLUMN + dp(8.),
                            [TimelineRow::new().h(dp(40.)).line(line).node(node)],
                        )
                        .into_any_element()
                    },
                )
                .into_any_element()
        })
        .size(960., 480.),
    ]
}

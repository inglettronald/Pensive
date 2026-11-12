//! The timeline's line beside one row: how the line runs above and below, and what sits on it.

use gpui::{
    AnyElement, App, Bounds, Hsla, Pixels, Window, canvas, div, fill, linear_color_stop,
    linear_gradient, point, prelude::*, px, relative, size,
};
use strum::{IntoStaticStr, VariantArray};
use theme::ActiveTheme;

/// The rail's width. Rows put their content after it.
pub const RAIL_WIDTH: Pixels = px(24.);
const LINE: Pixels = px(2.);
const DASH: Pixels = px(3.);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, VariantArray, IntoStaticStr)]
pub enum Line {
    #[default]
    None,
    Solid,
    /// Work that isn't committed yet.
    Dashed,
    /// The end of what's shown, fading out.
    Fade,
}

pub enum Node {
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

#[derive(IntoElement)]
pub struct Rail {
    above: Line,
    below: Line,
    node: Node,
    node_y: Option<Pixels>,
}

impl Default for Rail {
    fn default() -> Self {
        Self::new()
    }
}

impl Rail {
    pub fn new() -> Self {
        Self {
            above: Line::None,
            below: Line::None,
            node: Node::None,
            node_y: None,
        }
    }

    /// The same line above and below.
    pub fn line(self, line: Line) -> Self {
        self.above(line).below(line)
    }

    pub fn above(mut self, line: Line) -> Self {
        self.above = line;
        self
    }

    pub fn below(mut self, line: Line) -> Self {
        self.below = line;
        self
    }

    pub fn node(mut self, node: Node) -> Self {
        self.node = node;
        self
    }

    /// Where the node sits, from the top. Centered unless set, which suits a row that can grow.
    pub fn node_y(mut self, y: Pixels) -> Self {
        self.node_y = Some(y);
        self
    }
}

impl RenderOnce for Rail {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let line_color = colors.border;
        let segment = |line: Line| {
            let base = div().absolute().left(px(11.)).w(LINE);
            match line {
                Line::None => None,
                Line::Solid => Some(base.bg(line_color)),
                Line::Fade => Some(base.bg(linear_gradient(
                    180.,
                    linear_color_stop(line_color, 0.),
                    linear_color_stop(line_color.opacity(0.), 1.),
                ))),
                Line::Dashed => Some(base.child(dashes(line_color))),
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
        let dot = |size: Pixels, color: Hsla| div().size(size).rounded_full().bg(color);
        let node = match self.node {
            Node::None => None,
            Node::Commit { pushed } => Some(
                dot(
                    px(10.),
                    if pushed {
                        colors.text_faint
                    } else {
                        colors.text_strong
                    },
                )
                .into_any_element(),
            ),
            Node::Open => Some(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(18.))
                    .rounded_full()
                    .border_1()
                    .border_color(colors.text_muted)
                    .bg(colors.surface_panel)
                    .child(dot(px(12.), colors.text_strong))
                    .into_any_element(),
            ),
            Node::Tick => Some(
                div()
                    .w(px(16.))
                    .h(LINE)
                    .bg(colors.text_muted)
                    .into_any_element(),
            ),
            Node::Mark(mark) => Some(mark),
        };

        div()
            .relative()
            .flex_none()
            .self_stretch()
            .w(RAIL_WIDTH)
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
            }))
    }
}

/// Short dashes down the whole height. GPUI has no dashed borders, so they're painted.
fn dashes(color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let mut y = bounds.top();
            while y < bounds.bottom() {
                let height = DASH.min(bounds.bottom() - y);
                window.paint_quad(fill(
                    Bounds::new(point(bounds.left(), y), size(bounds.size.width, height)),
                    color,
                ));
                y += DASH * 2.;
            }
        },
    )
    .size_full()
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use crate::{Fill, StatusRing, gallery::Panel};

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
        mock::Mock::component("rail", "gallery", || {
            crate::Gallery::new("Rail")
                .matrix(
                    "Line and node",
                    Line::VARIANTS,
                    Shown::VARIANTS,
                    |line, shown| {
                        let node = match shown {
                            Shown::Nothing => Node::None,
                            Shown::Commit => Node::Commit { pushed: false },
                            Shown::Pushed => Node::Commit { pushed: true },
                            Shown::Open => Node::Open,
                            Shown::Tick => Node::Tick,
                            Shown::Ring => Node::Mark(
                                StatusRing::new("ring", Fill::Half)
                                    .color(|colors| colors.diff_modified)
                                    .size(px(14.))
                                    .into_any_element(),
                            ),
                        };
                        Panel::new(
                            RAIL_WIDTH + px(8.),
                            [div()
                                .flex()
                                .h(px(40.))
                                .child(Rail::new().line(line).node(node))],
                        )
                        .into_any_element()
                    },
                )
                .into_any_element()
        })
        .size(gpui::size(px(960.), px(480.))),
    ]
}

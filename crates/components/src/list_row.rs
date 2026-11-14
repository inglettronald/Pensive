//! One row of a list or tree: indent guides, a disclosure arrow, a leading mark, a label and
//! trailing content, with hover and selected states.

use std::rc::Rc;

use gpui::{AnyElement, App, ElementId, SharedString, Window, deferred, div, prelude::*};
use theme::{ActiveTheme, Dp, HAIRLINE, Typeset, dp};

use crate::{Callback, Icon, IconName, OnClick as _, callback};

/// The width of a disclosure arrow. A leading mark is centered in the same width, so arrows,
/// marks and guides share one center line.
const MARK: Dp = dp(12.);
/// How far each level of a tree moves in: one disclosure arrow's width.
const INDENT: Dp = MARK;
/// Where a guide sits in its indent column: under the parent's arrow.
const GUIDE_OFFSET: Dp = dp(5.5);

#[derive(IntoElement)]
pub struct ListRow {
    id: ElementId,
    depth: usize,
    indent: Dp,
    disclosure: Option<bool>,
    leading: Option<AnyElement>,
    label: SharedString,
    code: bool,
    muted: bool,
    compact: bool,
    trailing: Option<AnyElement>,
    below: Option<AnyElement>,
    selected: bool,
    hovered: bool,
    on_click: Option<Callback<()>>,
    on_disclosure: Option<Callback<()>>,
    on_hover: Option<Callback<bool>>,
}

impl ListRow {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            depth: 0,
            indent: INDENT,
            disclosure: None,
            leading: None,
            label: label.into(),
            code: false,
            muted: false,
            compact: false,
            trailing: None,
            below: None,
            selected: false,
            hovered: false,
            on_click: None,
            on_disclosure: None,
            on_hover: None,
        }
    }

    /// The slot an arrow or a leading mark takes, including its gap to what follows. Indenting a
    /// row by it lines its leading mark up under its parent's.
    pub const SLOT: Dp = dp(18.);

    /// How many levels in, each with a guide under its parent's arrow.
    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Overrides the 12 px indent, such as to line a child up under its parent's leading mark.
    pub fn indent(mut self, indent: Dp) -> Self {
        self.indent = indent;
        self
    }

    /// An arrow before the leading mark, pointing down when `open`.
    pub fn disclosure(mut self, open: bool) -> Self {
        self.disclosure = Some(open);
        self
    }

    /// A mark before the label, such as a status ring or an icon, in an 18 px slot.
    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }

    /// Draws the label in the code font.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// Small text in a row 4 px shorter, for rows nested under another, such as symbols.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// The label in `text_muted` unless the row is selected, such as for folders.
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// Shown under the label and drawn above every pane, such as a tooltip.
    pub fn below(mut self, below: impl IntoElement) -> Self {
        self.below = Some(below.into_any_element());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Shows the hover state without a pointer, for mocks.
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(callback(handler));
        self
    }

    /// Called when the arrow is clicked, instead of the row's own click.
    pub fn on_disclosure(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_disclosure = Some(callback(handler));
        self
    }

    /// Called when the pointer enters or leaves the row.
    pub fn on_hover(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ListRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let height = if self.compact {
            density.row_height - dp(4.)
        } else {
            density.row_height
        };
        let label_color = if self.selected {
            colors.text_strong
        } else if self.muted {
            colors.text_muted
        } else {
            colors.text
        };
        let label_start = self.indent * self.depth as f32
            + Self::SLOT
                * (usize::from(self.disclosure.is_some()) + usize::from(self.leading.is_some()))
                    as f32;

        div()
            .id(self.id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(height)
            .px_2()
            .rounded(density.radius_md)
            .map(|row| {
                if self.selected {
                    row.bg(colors.surface_selected)
                } else if self.hovered {
                    row.bg(colors.surface_raised)
                } else {
                    row.hover(|row| row.bg(colors.surface_raised))
                }
            })
            .children((0..self.depth).map(|_| {
                div()
                    .relative()
                    .flex_none()
                    .self_stretch()
                    .w(self.indent)
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(GUIDE_OFFSET)
                            .w(HAIRLINE)
                            .bg(colors.border_subtle),
                    )
            }))
            .children(self.disclosure.map(|open| {
                let arrow = div().id("disclosure").flex_none().w(Self::SLOT).child(
                    Icon::new(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .size(MARK)
                    .color(colors.text_faint),
                );
                match self.on_disclosure.clone() {
                    Some(handler) => arrow
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            handler((), window, cx)
                        })
                        .into_any_element(),
                    None => arrow.into_any_element(),
                }
            }))
            .children(self.leading.map(|leading| {
                div().flex_none().w(Self::SLOT).child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(MARK)
                        .child(leading),
                )
            }))
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .when(self.code, |label| {
                        label.font_family(theme.fonts.code.clone())
                    })
                    .typeset(if self.compact {
                        density.small
                    } else {
                        density.body
                    })
                    .text_color(label_color)
                    .child(self.label),
            )
            .child(div().flex_1().min_w_2())
            .children(self.trailing)
            .children(self.below.map(|below| {
                div()
                    .absolute()
                    .top_full()
                    .left(dp(8.) + label_start - dp(2.))
                    .mt_0p5()
                    .child(deferred(below).priority(1))
            }))
            .on_click_callback(self.on_click)
            .when_some(self.on_hover, |row, handler| {
                row.on_hover(move |hovered, window, cx| handler(*hovered, window, cx))
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use crate::{DiffStat, RingFill, StatusRing, Tooltip, gallery::Panel};

    vec![
        mock::Mock::component("list_row", "gallery", || {
            let file = |id: &'static str, name: &'static str, fill| {
                ListRow::new(id, name)
                    .leading(StatusRing::new("ring", fill).color(|colors| colors.diff_modified))
                    .trailing(DiffStat::new(4, 1))
            };
            let column = |rows: Vec<ListRow>| Panel::new(dp(256.), rows).into_any_element();
            crate::Gallery::new("ListRow")
                .section(
                    "States",
                    [
                        (
                            "Resting",
                            column(vec![file("a", "palette.rs", RingFill::Full)]),
                        ),
                        (
                            "Hovered",
                            column(vec![file("b", "palette.rs", RingFill::Empty).hovered(true)]),
                        ),
                        (
                            "Selected",
                            column(vec![file("c", "palette.rs", RingFill::Full).selected(true)]),
                        ),
                    ],
                )
                .section(
                    "Nested, with guides",
                    [
                        (
                            "Folders and files",
                            column(vec![
                                ListRow::new("d", "crates/ui/src").disclosure(true).muted(),
                                ListRow::new("e", "palette")
                                    .depth(1)
                                    .disclosure(true)
                                    .muted(),
                                file("f", "fuzzy.rs", RingFill::Empty).depth(2),
                                file("g", "root.rs", RingFill::Empty).depth(1),
                            ]),
                        ),
                        (
                            "A file's symbols",
                            column(vec![
                                file("h", "root.rs", RingFill::Empty).disclosure(true),
                                ListRow::new("i", "Pensive::render")
                                    .depth(1)
                                    .indent(ListRow::SLOT)
                                    .compact()
                                    .code()
                                    .leading(crate::Icon::new(IconName::Braces).size(dp(12.)))
                                    .trailing(DiffStat::new(9, 2)),
                            ]),
                        ),
                    ],
                )
                .section(
                    "With a tooltip below",
                    [(
                        "Hovered, path shown",
                        div()
                            .h(dp(64.))
                            .child(column(vec![
                                file("j", "row.rs", RingFill::Full).hovered(true).below(
                                    Tooltip::new("crates/components/src/palette/row.rs")
                                        .shortcut("Copy", "Ctrl Shift C"),
                                ),
                            ]))
                            .into_any_element(),
                    )],
                )
                .into_any_element()
        })
        .size(960., 640.),
    ]
}

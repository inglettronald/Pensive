//! A diff line around content the caller draws: its new-file number, an edge bar and a fill.

#[cfg(feature = "mocks")]
use std::ops::Range;

use gpui::{AnyElement, App, Hsla, Window, div, prelude::*};
use theme::{ActiveTheme, ThemeColors, dp};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

impl LineKind {
    /// The meaning hue of an added or removed line.
    pub fn hue(self, colors: &ThemeColors) -> Option<Hsla> {
        match self {
            LineKind::Added => Some(colors.diff_added),
            LineKind::Removed => Some(colors.diff_removed),
            LineKind::Context => None,
        }
    }
}

/// One line of a diff around content the caller draws: its new-file number, an edge bar, and a
/// faint fill when it's added or removed. It's one code line tall.
#[derive(IntoElement)]
pub struct DiffRow {
    kind: LineKind,
    number: Option<usize>,
    content: AnyElement,
}

impl DiffRow {
    pub fn new(kind: LineKind, number: Option<usize>, content: impl IntoElement) -> Self {
        Self {
            kind,
            number,
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for DiffRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let hue = self.kind.hue(colors);
        div()
            .flex()
            .w_full()
            .h(cx.density().code.line_height)
            .child(
                div()
                    .flex_none()
                    .w(dp(44.))
                    .pr_2()
                    .text_right()
                    .text_color(if self.kind == LineKind::Added {
                        colors.diff_added
                    } else {
                        colors.diff_line_number
                    })
                    .children(self.number.map(|number| number.to_string())),
            )
            .child(
                div()
                    .flex_none()
                    .w(dp(2.))
                    .when_some(hue, |bar, hue| bar.bg(hue)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pl_3()
                    .overflow_hidden()
                    .when_some(hue, |code, hue| code.bg(hue.opacity(0.08)))
                    .child(self.content),
            )
    }
}

/// A row of plain text with one range of changed words, for the gallery.
#[cfg(feature = "mocks")]
#[derive(IntoElement)]
struct Sample(LineKind, Option<usize>, &'static str, Option<Range<usize>>);

#[cfg(feature = "mocks")]
impl RenderOnce for Sample {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        use theme::Typeset as _;

        let Sample(kind, number, text, changed) = self;
        let theme = cx.theme();
        let word = gpui::HighlightStyle {
            background_color: kind.hue(&theme.colors).map(|hue| hue.opacity(0.28)),
            ..Default::default()
        };
        let text = gpui::StyledText::new(text).with_highlights(changed.map(|range| (range, word)));
        div()
            .font_family(theme.fonts.code.clone())
            .typeset(cx.density().code)
            .text_color(theme.colors.text)
            .child(DiffRow::new(kind, number, text))
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::{px, size};

    vec![
        mock::Mock::component("diff", "gallery", || {
            crate::Gallery::new("Diff row")
                .section(
                    "Every kind of line",
                    [(
                        "Context, added, and a removed line paired with the one that replaced it",
                        div()
                            .w(dp(640.))
                            .py_1()
                            .whitespace_nowrap()
                            .children([
                                Sample(
                                    LineKind::Context,
                                    Some(21),
                                    "        let theme = cx.theme();",
                                    None,
                                ),
                                Sample(
                                    LineKind::Added,
                                    Some(22),
                                    "        let density = cx.density();",
                                    None,
                                ),
                                Sample(LineKind::Context, Some(23), "        h_flex()", None),
                                Sample(
                                    LineKind::Removed,
                                    None,
                                    "            .h(px(28.))",
                                    Some(15..22),
                                ),
                                Sample(
                                    LineKind::Added,
                                    Some(24),
                                    "            .h(density.row_height)",
                                    Some(15..33),
                                ),
                                Sample(
                                    LineKind::Context,
                                    Some(25),
                                    "            .gap(density.row_gap)",
                                    None,
                                ),
                            ])
                            .into_any_element(),
                    )],
                )
                .into_any_element()
        })
        .size(size(px(800.), px(320.))),
    ]
}

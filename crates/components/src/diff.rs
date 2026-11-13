//! Diff lines: one column of new-file numbers, an edge bar, and fills on changed lines and words.

use std::ops::Range;

use gpui::{
    AnyElement, App, HighlightStyle, Hsla, SharedString, StyledText, Window, div, prelude::*, px,
};
use theme::{ActiveTheme, ThemeColors, Typeset};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
    /// The `@@` line that starts a hunk.
    Hunk,
}

pub struct DiffLine {
    kind: LineKind,
    number: Option<usize>,
    text: SharedString,
    changed: Vec<Range<usize>>,
}

impl DiffLine {
    pub fn context(number: usize, text: impl Into<SharedString>) -> Self {
        Self::new(LineKind::Context, Some(number), text)
    }

    pub fn added(number: usize, text: impl Into<SharedString>) -> Self {
        Self::new(LineKind::Added, Some(number), text)
    }

    /// A removed line has no new-file number.
    pub fn removed(text: impl Into<SharedString>) -> Self {
        Self::new(LineKind::Removed, None, text)
    }

    pub fn hunk(text: impl Into<SharedString>) -> Self {
        Self::new(LineKind::Hunk, None, text)
    }

    fn new(kind: LineKind, number: Option<usize>, text: impl Into<SharedString>) -> Self {
        Self {
            kind,
            number,
            text: text.into(),
            changed: Vec::new(),
        }
    }

    /// A byte range of words that changed, highlighted more strongly than the line. Call it once
    /// per range.
    pub fn changed(mut self, range: Range<usize>) -> Self {
        self.changed.push(range);
        self
    }
}

#[derive(IntoElement)]
pub struct Diff {
    lines: Vec<DiffLine>,
}

impl Diff {
    pub fn new(lines: impl IntoIterator<Item = DiffLine>) -> Self {
        Self {
            lines: lines.into_iter().collect(),
        }
    }
}

impl RenderOnce for Diff {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        div()
            .flex()
            .flex_col()
            .py_1()
            .font_family(theme.fonts.code.clone())
            .typeset(cx.density().code)
            .text_color(colors.text)
            .whitespace_nowrap()
            .children(self.lines.into_iter().map(|line| {
                let word = HighlightStyle {
                    background_color: line.kind.hue(colors).map(|hue| hue.opacity(0.28)),
                    ..Default::default()
                };
                let text = StyledText::new(line.text)
                    .with_highlights(line.changed.into_iter().map(|range| (range, word)));
                DiffRow::new(line.kind, line.number, text)
            }))
    }
}

impl LineKind {
    /// The meaning hue of an added or removed line.
    pub fn hue(self, colors: &ThemeColors) -> Option<Hsla> {
        match self {
            LineKind::Added => Some(colors.diff_added),
            LineKind::Removed => Some(colors.diff_removed),
            LineKind::Context | LineKind::Hunk => None,
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
            .when(self.kind == LineKind::Hunk, |row| {
                row.text_color(colors.text_faint)
            })
            .child(
                div()
                    .flex_none()
                    .w(px(44.))
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
                    .w(px(2.))
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

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("diff", "gallery", || {
            crate::Gallery::new("Diff")
                .section(
                    "Every kind of line",
                    [(
                        "A hunk with context, added, removed and changed words",
                        div()
                            .w(px(640.))
                            .child(Diff::new([
                                DiffLine::hunk("@@ -21,5 +21,6 @@ fn render"),
                                DiffLine::context(21, "        let theme = cx.theme();"),
                                DiffLine::added(22, "        let density = cx.density();"),
                                DiffLine::context(23, "        h_flex()"),
                                DiffLine::removed("            .h(px(28.))").changed(15..22),
                                DiffLine::added(24, "            .h(density.row_height)")
                                    .changed(15..33),
                                DiffLine::context(25, "            .gap(density.row_gap)"),
                            ]))
                            .into_any_element(),
                    )],
                )
                .into_any_element()
        })
        .size(size(px(800.), px(320.))),
    ]
}

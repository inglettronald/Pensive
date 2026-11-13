//! One line of the editor's text: shaped and painted with its selection, cursor and highlights,
//! and recorded so the editor can find text under the pointer.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, Bounds, ElementId, GlobalElementId, Hsla, LayoutId, Pixels, ShapedLine, SharedString,
    Style, TextAlign, TextRun, UnderlineStyle, WeakEntity, Window, fill, point, prelude::*, px,
    relative, size,
};
use theme::ActiveTheme;

use super::Editor;

/// How wide a selected line ending shows.
const NEWLINE_WIDTH: f32 = 6.;
/// How close to the edge the cursor may get before the text scrolls sideways.
const SCROLL_MARGIN: f32 = 24.;

/// Where a line was painted last frame.
pub(super) struct LineLayout {
    pub line: usize,
    /// The byte offset of the line's start in the text.
    pub start: usize,
    pub bounds: Bounds<Pixels>,
    pub shaped: ShapedLine,
}

impl LineLayout {
    /// The text offset closest to `x`, a window position.
    pub fn offset_at(&self, x: Pixels, scroll_x: Pixels) -> usize {
        self.start
            + self
                .shaped
                .closest_index_for_x(x - self.bounds.left() + scroll_x)
    }

    /// The window position of the text offset `offset`, which must be on this line.
    pub fn x_of(&self, offset: usize, scroll_x: Pixels) -> Pixels {
        self.bounds.left() + self.shaped.x_for_index(offset - self.start) - scroll_x
    }
}

pub(super) type Layouts = Rc<RefCell<Vec<LineLayout>>>;

pub(super) struct LineElement {
    pub editor: WeakEntity<Editor>,
    pub line: usize,
    pub start: usize,
    /// Without its line ending.
    pub text: SharedString,
    /// The selected bytes of this line, and whether the selection runs on past its end.
    pub selection: Option<(Range<usize>, bool)>,
    /// Where the cursor is on this line, while the editor has focus.
    pub cursor: Option<usize>,
    /// Bytes with a background, such as words that changed.
    pub highlights: Vec<(Range<usize>, Hsla)>,
    /// Text the input method is composing, underlined.
    pub marked: Option<Range<usize>>,
    pub scroll_x: Pixels,
    /// Whether to scroll sideways so the cursor shows.
    pub autoscroll: bool,
    pub layouts: Layouts,
}

impl IntoElement for LineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for LineElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<ShapedLine>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let runs = runs(
            self.text.len(),
            style.to_run(self.text.len()),
            &self.highlights,
            self.marked.as_ref(),
        );
        Some(
            window
                .text_system()
                .shape_line(self.text.clone(), font_size, &runs, None),
        )
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        shaped: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(shaped) = shaped.take() else {
            return;
        };
        let colors = &cx.theme().colors;
        let (selected_color, cursor_color) = (colors.accent_solid.opacity(0.4), colors.text_strong);
        let x = |index: usize| bounds.left() + shaped.x_for_index(index) - self.scroll_x;
        if let Some((selected, past_end)) = &self.selection {
            let end = x(selected.end) + if *past_end { px(NEWLINE_WIDTH) } else { px(0.) };
            window.paint_quad(fill(
                Bounds::from_corners(
                    point(x(selected.start), bounds.top()),
                    point(end, bounds.bottom()),
                ),
                selected_color,
            ));
        }
        let origin = point(bounds.left() - self.scroll_x, bounds.top());
        let line_height = window.line_height();
        shaped
            .paint_background(origin, line_height, TextAlign::Left, None, window, cx)
            .ok();
        shaped
            .paint(origin, line_height, TextAlign::Left, None, window, cx)
            .ok();
        if let Some(cursor) = self.cursor {
            let cursor_x = x(cursor);
            window.paint_quad(fill(
                Bounds::new(
                    point(cursor_x, bounds.top()),
                    size(px(1.5), bounds.size.height),
                ),
                cursor_color,
            ));
            if self.autoscroll {
                self.scroll_to_show(cursor_x, bounds, cx);
            }
        }
        self.layouts.borrow_mut().push(LineLayout {
            line: self.line,
            start: self.start,
            bounds,
            shaped,
        });
    }
}

impl LineElement {
    /// Moves the text sideways when the cursor at `cursor_x` is outside `bounds`.
    fn scroll_to_show(&self, cursor_x: Pixels, bounds: Bounds<Pixels>, cx: &mut App) {
        let margin = px(SCROLL_MARGIN).min(bounds.size.width / 2.);
        let scroll_x = if cursor_x > bounds.right() - margin {
            self.scroll_x + (cursor_x - (bounds.right() - margin))
        } else if cursor_x < bounds.left() + margin {
            (self.scroll_x - (bounds.left() + margin - cursor_x)).max(px(0.))
        } else {
            return;
        };
        self.editor
            .update(cx, |editor, cx| editor.set_scroll_x(scroll_x, cx))
            .ok();
    }
}

/// Runs of `base` over a line of `len` bytes, with backgrounds from `highlights`, which mustn't
/// overlap, and `marked` underlined.
fn runs(
    len: usize,
    base: TextRun,
    highlights: &[(Range<usize>, Hsla)],
    marked: Option<&Range<usize>>,
) -> Vec<TextRun> {
    let mut edges = vec![0, len];
    for (range, _) in highlights {
        edges.extend([range.start, range.end]);
    }
    if let Some(marked) = marked {
        edges.extend([marked.start, marked.end]);
    }
    edges.retain(|&edge| edge <= len);
    edges.sort_unstable();
    edges.dedup();
    edges
        .windows(2)
        .map(|pair| {
            let span = pair[0]..pair[1];
            let background = highlights
                .iter()
                .find(|(range, _)| range.start <= span.start && span.end <= range.end)
                .map(|(_, color)| *color);
            let underline = marked
                .filter(|marked| marked.start <= span.start && span.end <= marked.end)
                .map(|_| UnderlineStyle {
                    color: Some(base.color),
                    thickness: px(1.),
                    wavy: false,
                });
            TextRun {
                len: span.len(),
                background_color: background,
                underline,
                ..base.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use gpui::{Hsla, font};

    use super::*;

    fn base() -> TextRun {
        TextRun {
            len: 0,
            font: font("Mono"),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn highlights_split_the_line_into_runs() {
        let red = Hsla::red();
        let runs = runs(10, base(), &[(2..5, red)], Some(&(4..6)));
        let lens: Vec<_> = runs.iter().map(|run| run.len).collect();
        assert_eq!(lens, [2, 2, 1, 1, 4]);
        assert_eq!(runs[1].background_color, Some(red));
        assert!(runs[2].underline.is_some() && runs[2].background_color.is_some());
        assert!(runs[3].underline.is_some() && runs[3].background_color.is_none());
    }

    #[test]
    fn an_empty_line_has_no_runs() {
        assert!(runs(0, base(), &[], None).is_empty());
    }
}

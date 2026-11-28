//! One line of the editor's text: shaped and painted with its selection, cursor and highlights,
//! and recorded so the editor can find text under the pointer.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, Bounds, ElementId, GlobalElementId, LayoutId, Pixels, ShapedLine, Style, TextAlign,
    WeakEntity, Window, fill, point, prelude::*, relative, size,
};
use theme::{ActiveTheme, CARET, Dp, dp};

use super::{Editor, display_line::DisplayLine};

/// How wide a selected line ending shows.
const NEWLINE_WIDTH: Dp = dp(6.);
/// How close to the edge the cursor may get before the text scrolls sideways.
const SCROLL_MARGIN: Dp = dp(24.);

/// Where a line was painted last frame.
pub(super) struct LineLayout {
    pub line: usize,
    display: Rc<DisplayLine>,
    pub bounds: Bounds<Pixels>,
    shaped: ShapedLine,
}

impl LineLayout {
    /// The text offset closest to `x`, a window position.
    pub fn offset_at(&self, x: Pixels, scroll_x: Pixels) -> usize {
        self.display.source_offset(
            self.shaped
                .closest_index_for_x(x - self.bounds.left() + scroll_x),
        )
    }

    /// The window position of a document offset, clamped to this line.
    pub fn x_of(&self, offset: usize, scroll_x: Pixels) -> Pixels {
        self.bounds.left() + self.shaped.x_for_index(self.display.display_offset(offset)) - scroll_x
    }
}

pub(super) type Layouts = Rc<RefCell<Vec<LineLayout>>>;

pub(super) struct LineElement {
    pub display: Rc<DisplayLine>,
    pub scroll_x: Pixels,
    pub interaction: Option<LineInteraction>,
}

pub(super) struct LineInteraction {
    pub line: usize,
    /// Selected document bytes on this line, and whether the selection continues past it.
    pub selection: Option<(Range<usize>, bool)>,
    /// Where the cursor is on this line, while the editor has focus.
    pub cursor: Option<usize>,
    /// Whether to scroll sideways so the cursor shows.
    pub autoscroll: Option<WeakEntity<Editor>>,
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
        let text = &self.display.text;
        let mut runs = self.display.runs();
        let text_system = window.text_system();
        let mut shaped = text_system.shape_line(text.clone(), font_size, &runs, None);
        if runs.windows(2).any(|pair| {
            pair[0].color != pair[1].color
                || pair[0].underline != pair[1].underline
                || pair[0].strikethrough != pair[1].strikethrough
        }) {
            // Shape across paint-only boundaries but retain real font changes.
            for run in &mut runs {
                run.color = style.color;
                run.underline = None;
                run.strikethrough = None;
            }
            *shaped = text_system.layout_line(text, font_size, &runs, None);
        }
        Some(shaped)
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
        let Some(shaped) = shaped.take() else { return };
        let colors = &cx.theme().colors;
        let (selected_color, cursor_color) = (colors.accent_solid.opacity(0.4), colors.text_strong);
        let x = |index: usize| {
            bounds.left() + shaped.x_for_index(self.display.display_offset(index)) - self.scroll_x
        };
        if let Some((selected, past_end)) = self
            .interaction
            .as_ref()
            .and_then(|line| line.selection.as_ref())
        {
            let newline = NEWLINE_WIDTH.to_pixels(window.rem_size());
            let end = x(selected.end) + if *past_end { newline } else { Pixels::ZERO };
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
        let Some(line) = &self.interaction else {
            return;
        };
        if let Some(cursor) = line.cursor {
            let cursor_x = x(cursor);
            window.paint_quad(fill(
                Bounds::new(
                    point(cursor_x, bounds.top()),
                    size(CARET, bounds.size.height),
                ),
                cursor_color,
            ));
            if let Some(editor) = &line.autoscroll {
                self.scroll_to_show(editor, cursor_x, bounds, window, cx);
            }
        }
        line.layouts.borrow_mut().push(LineLayout {
            line: line.line,
            display: self.display.clone(),
            bounds,
            shaped,
        });
    }
}

impl LineElement {
    /// Moves the text sideways when the cursor at `cursor_x` is outside `bounds`, from the next
    /// frame, since a change made while painting doesn't draw another.
    fn scroll_to_show(
        &self,
        editor: &WeakEntity<Editor>,
        cursor_x: Pixels,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let margin = SCROLL_MARGIN
            .to_pixels(window.rem_size())
            .min(bounds.size.width / 2.);
        let scroll_x = if cursor_x > bounds.right() - margin {
            self.scroll_x + (cursor_x - (bounds.right() - margin))
        } else if cursor_x < bounds.left() + margin {
            (self.scroll_x - (bounds.left() + margin - cursor_x)).max(Pixels::ZERO)
        } else {
            return;
        };
        editor
            .update(cx, |editor, cx| editor.set_scroll_x(scroll_x, cx))
            .ok();
        window.request_animation_frame();
    }
}

#[cfg(test)]
mod tests {
    use gpui::{FontStyle, TestAppContext};

    use super::*;
    use crate::editor::display_line::InlineText;

    #[gpui::test]
    fn clicking_an_inline_hint_or_the_source_after_it(cx: &mut TestAppContext) {
        cx.add_empty_window().update(|window, cx| {
            let base = window.text_style().to_run(3);
            let mut hint_style = base.clone();
            hint_style.font.style = FontStyle::Italic;
            let hint = InlineText {
                anchor: 11,
                text: ": int".into(),
                style: hint_style,
            };
            let display = Rc::new(DisplayLine::compose(10, "a=b".into(), vec![base], &[hint]));
            let mut element = LineElement {
                display: display.clone(),
                scroll_x: Pixels::ZERO,
                interaction: None,
            };
            let bounds = Bounds::default();
            let shaped = element
                .prepaint(None, None, bounds, &mut (), window, cx)
                .unwrap();
            let inside_hint = shaped.x_for_index(3);
            let after_equals = shaped.x_for_index(7);
            let layout = LineLayout {
                line: 0,
                display,
                bounds,
                shaped,
            };
            assert!(layout.x_of(11, Pixels::ZERO) < inside_hint);
            assert_eq!(layout.offset_at(inside_hint, Pixels::ZERO), 11);
            assert_eq!(layout.offset_at(after_equals, Pixels::ZERO), 12);
            assert_eq!(layout.x_of(12, Pixels::ZERO), after_equals);
        });
    }
}

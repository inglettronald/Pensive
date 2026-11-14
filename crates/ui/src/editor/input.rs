//! Connects the editor to the platform's input method, which counts text in UTF-16.

use std::ops::Range;

use gpui::{
    App, Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, point,
};

use super::Editor;
use crate::text_editing::utf16;

impl Editor {
    /// `range`, counted in UTF-16, as bytes.
    fn bytes(&self, range: Option<Range<usize>>, cx: &App) -> Option<Range<usize>> {
        Some(utf16::range_from_utf16(self.text(cx)?, &range?))
    }
}

impl EntityInputHandler for Editor {
    fn accepts_text_input(&self, _window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.editable(cx)
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.text(cx)?;
        let range = utf16::range_from_utf16(text, &range_utf16);
        actual_range.replace(utf16::range_to_utf16(text, &range));
        Some(text.byte_slice(range).to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(self.selection.to_utf16(self.text(cx)?))
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(range) = self.bytes(Some(range_utf16), cx) {
            self.selection.select(range);
            self.goal_column = None;
            cx.notify();
        }
    }

    fn text_length_utf16(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        Some(self.text(cx)?.len_utf16_cu())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let marked = self.selection.marked.as_ref()?;
        Some(utf16::range_to_utf16(self.text(cx)?, marked))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.selection.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.bytes(range_utf16, cx);
        self.insert(range, new_text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable(cx) {
            return;
        }
        let range = self.selection.replaced(self.bytes(range_utf16, cx));
        self.insert(Some(range.clone()), new_text, cx);
        let Some(text) = self.text(cx) else {
            return;
        };
        self.selection
            .mark(text, range.start, new_text, new_selected_range_utf16);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let text = self.text(cx)?;
        let range = utf16::range_from_utf16(text, &range_utf16);
        let line = text.byte_to_line(range.start);
        let layouts = self.layouts.borrow();
        let layout = layouts.iter().find(|layout| layout.line == line)?;
        let end = range.end.min(layout.start + layout.shaped.text.len());
        Some(Bounds::from_corners(
            point(layout.x_of(range.start, self.scroll_x), layout.bounds.top()),
            point(layout.x_of(end, self.scroll_x), layout.bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.offset_at(point)?;
        Some(utf16::to_utf16(self.text(cx)?, offset))
    }
}

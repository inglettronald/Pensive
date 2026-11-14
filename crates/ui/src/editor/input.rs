//! Connects the editor to the platform's input method, which counts text in UTF-16.

use std::ops::Range;

use gpui::{
    App, Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, point,
};

use super::Editor;

impl Editor {
    fn utf16(&self, offset: usize, cx: &App) -> usize {
        self.text(cx).map_or(0, |text| {
            text.char_to_utf16_cu(text.byte_to_char(offset.min(text.len_bytes())))
        })
    }

    fn offset_of_utf16(&self, offset: usize, cx: &App) -> usize {
        self.text(cx).map_or(0, |text| {
            let offset = offset.min(text.len_utf16_cu());
            text.char_to_byte(text.utf16_cu_to_char(offset))
        })
    }

    fn range_utf16(&self, range: &Range<usize>, cx: &App) -> Range<usize> {
        self.utf16(range.start, cx)..self.utf16(range.end, cx)
    }

    fn range_from_utf16(&self, range: &Range<usize>, cx: &App) -> Range<usize> {
        self.offset_of_utf16(range.start, cx)..self.offset_of_utf16(range.end, cx)
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
        let range = self.range_from_utf16(&range_utf16, cx);
        actual_range.replace(self.range_utf16(&range, cx));
        Some(self.text(cx)?.byte_slice(range).to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_utf16(&self.selection, cx),
            reversed: self.reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection = self.range_from_utf16(&range_utf16, cx);
        self.reversed = false;
        cx.notify();
    }

    fn text_length_utf16(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        Some(self.text(cx)?.len_utf16_cu())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|range| self.range_utf16(range, cx))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16.map(|range| self.range_from_utf16(&range, cx));
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
        let range = range_utf16
            .map(|range| self.range_from_utf16(&range, cx))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        self.insert(Some(range.clone()), new_text, cx);
        if new_text.is_empty() {
            return;
        }
        self.marked = Some(range.start..range.start + new_text.len());
        if let Some(selected) = new_selected_range_utf16 {
            let base = self.utf16(range.start, cx);
            self.selection =
                self.range_from_utf16(&(base + selected.start..base + selected.end), cx);
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16, cx);
        let line = self.text(cx)?.byte_to_line(range.start);
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
        Some(self.utf16(offset, cx))
    }
}

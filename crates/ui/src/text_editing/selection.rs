//! Where the cursor is in a text, what's selected, and what the input method is composing.

use std::ops::Range;

use gpui::UTF16Selection;
use model::Rope;

use super::utf16;

/// In byte offsets.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Selection {
    /// The cursor is its end, or its start when `reversed`.
    pub range: Range<usize>,
    pub reversed: bool,
    /// Text the input method is composing.
    pub marked: Option<Range<usize>>,
}

impl Selection {
    /// The cursor at `offset`, with nothing selected or being composed.
    pub fn at(offset: usize) -> Self {
        Self {
            range: offset..offset,
            ..Self::default()
        }
    }

    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.range.start
        } else {
            self.range.end
        }
    }

    /// Puts the cursor at `offset`, or with `select` moves it there and keeps the other end.
    pub fn move_cursor(&mut self, offset: usize, select: bool) {
        if !select {
            self.select(offset..offset);
            return;
        }
        if self.reversed {
            self.range.start = offset;
        } else {
            self.range.end = offset;
        }
        if self.range.end < self.range.start {
            self.reversed = !self.reversed;
            self.range = self.range.end..self.range.start;
        }
    }

    /// Selects `range` with the cursor at its end.
    pub fn select(&mut self, range: Range<usize>) {
        self.range = range;
        self.reversed = false;
    }

    /// What typing replaces: `range` when given, or the text being composed, or the selection.
    pub fn replaced(&self, range: Option<Range<usize>>) -> Range<usize> {
        range.or(self.marked.clone()).unwrap_or(self.range.clone())
    }

    /// After the input method put `inserted` at `start`: marks it as being composed, and selects
    /// `selected`, in UTF-16 from its start, or puts the cursor after it.
    pub fn mark(
        &mut self,
        text: &Rope,
        start: usize,
        inserted: &str,
        selected: Option<Range<usize>>,
    ) {
        let end = start + inserted.len();
        self.marked = (!inserted.is_empty()).then_some(start..end);
        let range = match selected {
            Some(selected) => {
                let base = utf16::to_utf16(text, start);
                let len = inserted.encode_utf16().count();
                let selected = base + selected.start.min(len)..base + selected.end.min(len);
                utf16::range_from_utf16(text, &selected)
            }
            None => end..end,
        };
        self.select(range);
    }

    /// Keeps every offset inside `text` and between characters, for when the text changed
    /// underneath, as reloading does.
    pub fn clamp(&mut self, text: &Rope) {
        let clamp =
            |offset: usize| text.char_to_byte(text.byte_to_char(offset.min(text.len_bytes())));
        self.range = clamp(self.range.start)..clamp(self.range.end);
        self.marked = self
            .marked
            .take()
            .map(|marked| clamp(marked.start)..clamp(marked.end))
            .filter(|marked| !marked.is_empty());
    }

    pub fn to_utf16(&self, text: &Rope) -> UTF16Selection {
        UTF16Selection {
            range: utf16::range_to_utf16(text, &self.range),
            reversed: self.reversed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_back_past_the_anchor_reverses() {
        let mut selection = Selection::default();
        selection.move_cursor(4, false);
        selection.move_cursor(6, true);
        selection.move_cursor(2, true);
        assert_eq!(selection.range, 2..4);
        assert_eq!(selection.cursor(), 2);
    }

    #[test]
    fn a_composition_selects_inside_what_it_inserted() {
        let text = Rope::from_str("a😀bc");
        let mut selection = Selection::default();
        selection.mark(&text, 1, "😀b", Some(2..9));
        assert_eq!(selection.marked, Some(1..6));
        assert_eq!(selection.range, 5..6);
    }
}

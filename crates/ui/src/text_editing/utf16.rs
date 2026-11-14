//! Converts byte offsets in a rope to and from the UTF-16 offsets that input methods count in.

use std::ops::Range;

use model::Rope;

pub(crate) fn to_utf16(text: &Rope, offset: usize) -> usize {
    text.char_to_utf16_cu(text.byte_to_char(offset.min(text.len_bytes())))
}

pub(crate) fn from_utf16(text: &Rope, offset: usize) -> usize {
    text.char_to_byte(text.utf16_cu_to_char(offset.min(text.len_utf16_cu())))
}

pub(crate) fn range_to_utf16(text: &Rope, range: &Range<usize>) -> Range<usize> {
    to_utf16(text, range.start)..to_utf16(text, range.end)
}

pub(crate) fn range_from_utf16(text: &Rope, range: &Range<usize>) -> Range<usize> {
    from_utf16(text, range.start)..from_utf16(text, range.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_outside_the_basic_plane_counts_twice() {
        let text = Rope::from_str("a😀b");
        assert_eq!(to_utf16(&text, 5), 3);
        assert_eq!(from_utf16(&text, 3), 5);
        assert_eq!(range_to_utf16(&text, &(0..6)), 0..4);
        assert_eq!(from_utf16(&text, 99), 6);
    }
}

//! Where the cursor goes in a rope: by character, word or line, in byte offsets.

use model::Rope;
use unicode_segmentation::UnicodeSegmentation;

/// A line's text without its line ending, and the byte offset it starts at.
pub(crate) fn line(text: &Rope, index: usize) -> (usize, String) {
    let start = text.line_to_byte(index);
    let mut line = text.line(index).to_string();
    let trimmed = line.trim_end_matches(['\n', '\r']).len();
    line.truncate(trimmed);
    (start, line)
}

/// The line `offset` is on, its start and its text.
fn line_at(text: &Rope, offset: usize) -> (usize, usize, String) {
    let index = text.byte_to_line(offset);
    let (start, line) = line(text, index);
    (index, start, line)
}

/// The start of the character before `offset`, stepping over a line ending as one.
pub(crate) fn previous(text: &Rope, offset: usize) -> usize {
    let (index, start, line) = line_at(text, offset);
    if offset == start {
        return if index == 0 {
            0
        } else {
            end_of_line(text, index - 1)
        };
    }
    start
        + line
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .rev()
            .find(|&at| start + at < offset)
            .unwrap_or(0)
}

/// The end of the character after `offset`, stepping over a line ending as one.
pub(crate) fn next(text: &Rope, offset: usize) -> usize {
    let (index, start, line) = line_at(text, offset);
    if offset >= start + line.len() {
        return if index + 1 < text.len_lines() {
            text.line_to_byte(index + 1)
        } else {
            offset
        };
    }
    start
        + line
            .grapheme_indices(true)
            .map(|(at, grapheme)| at + grapheme.len())
            .find(|&end| start + end > offset)
            .unwrap_or(line.len())
}

/// The start of the word before `offset`, or the line's start, or the previous line's end.
pub(crate) fn previous_word(text: &Rope, offset: usize) -> usize {
    let (_, start, line) = line_at(text, offset);
    if offset == start {
        return previous(text, offset);
    }
    start
        + line
            .split_word_bound_indices()
            .rev()
            .find(|(at, word)| start + at < offset && !word.trim().is_empty())
            .map(|(at, _)| at)
            .unwrap_or(0)
}

/// The end of the word after `offset`, or the line's end, or the next line's start.
pub(crate) fn next_word(text: &Rope, offset: usize) -> usize {
    let (_, start, line) = line_at(text, offset);
    if offset >= start + line.len() {
        return next(text, offset);
    }
    start
        + line
            .split_word_bound_indices()
            .map(|(at, word)| (at + word.len(), word))
            .find(|(end, word)| start + end > offset && !word.trim().is_empty())
            .map_or(line.len(), |(end, _)| end)
}

/// Where `offset`'s line starts, past its indentation unless the cursor is already there, as
/// editors do on Home.
pub(crate) fn home(text: &Rope, offset: usize) -> usize {
    let (_, start, line) = line_at(text, offset);
    let indented = start + (line.len() - line.trim_start().len());
    if offset == indented { start } else { indented }
}

/// Where `offset`'s line ends, before its line ending.
pub(crate) fn end(text: &Rope, offset: usize) -> usize {
    end_of_line(text, text.byte_to_line(offset))
}

fn end_of_line(text: &Rope, index: usize) -> usize {
    let (start, line) = line(text, index);
    start + line.len()
}

/// How many characters into its line `offset` is.
pub(crate) fn column(text: &Rope, offset: usize) -> usize {
    let (_, start, line) = line_at(text, offset);
    // Between a `\r` and its `\n` is past the line's text.
    line[..(offset - start).min(line.len())].chars().count()
}

/// The offset `column` characters into line `index`, or its end when it's shorter.
pub(crate) fn at_column(text: &Rope, index: usize, column: usize) -> usize {
    let (start, line) = line(text, index);
    start
        + line
            .char_indices()
            .nth(column)
            .map_or(line.len(), |(at, _)| at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rope(text: &str) -> Rope {
        Rope::from_str(text)
    }

    #[test]
    fn characters_step_over_line_endings_as_one() {
        let text = rope("ab\r\ncd");
        assert_eq!(next(&text, 2), 4);
        assert_eq!(previous(&text, 4), 2);
        assert_eq!(previous(&text, 0), 0);
        assert_eq!(next(&text, 6), 6);
    }

    #[test]
    fn a_grapheme_is_one_step() {
        let text = rope("e\u{301}x");
        assert_eq!(next(&text, 0), 3);
        assert_eq!(previous(&text, 3), 0);
    }

    #[test]
    fn words_skip_spaces_and_stop_at_line_edges() {
        let text = rope("let value = 1;\nnext");
        assert_eq!(next_word(&text, 0), 3);
        assert_eq!(next_word(&text, 3), 9);
        assert_eq!(previous_word(&text, 9), 4);
        assert_eq!(next_word(&text, 14), 15);
    }

    #[test]
    fn home_goes_past_indentation_first() {
        let text = rope("    code");
        assert_eq!(home(&text, 6), 4);
        assert_eq!(home(&text, 4), 0);
        assert_eq!(end(&text, 0), 8);
    }

    #[test]
    fn columns_clamp_to_short_lines() {
        let text = rope("long line\nab\n");
        assert_eq!(column(&text, 7), 7);
        assert_eq!(at_column(&text, 1, 7), 12);
        assert_eq!(at_column(&text, 2, 3), 13);
    }
}

//! The state each line of a text starts lexing in, kept current through edits.

use std::{borrow::Cow, ops::Range};

use ropey::Rope;

use super::{State, Token, lex};

#[derive(Clone, Debug, Default)]
pub(crate) struct LineStates(Vec<State>);

impl LineStates {
    pub fn new<'a>(lines: impl IntoIterator<Item = impl Into<Cow<'a, str>>>) -> Self {
        let mut states = vec![State::Code];
        for line in lines {
            let start = *states.last().expect("starts with one");
            states.push(lex(&line.into(), start).1);
        }
        // The last state is where a line after the text would start.
        states.pop();
        Self(states)
    }

    pub fn of_rope(text: &Rope) -> Self {
        Self::new(text.lines())
    }

    /// The tokens of line `index`, whose text is `line`.
    pub fn tokens(&self, index: usize, line: &str) -> Vec<(Range<usize>, Token)> {
        lex(line, self.0.get(index).copied().unwrap_or_default()).0
    }

    /// Follows an edit of `text` that replaced the line breaks after line `first` with `added`
    /// new ones, `removed` before. Lexes again from `first` until a line starts as it did before.
    pub fn edited(&mut self, text: &Rope, first: usize, removed: usize, added: usize) {
        let replaced = (first + 1).min(self.0.len())..(first + 1 + removed).min(self.0.len());
        self.0
            .splice(replaced, std::iter::repeat_n(State::Code, added));
        debug_assert_eq!(self.0.len(), text.len_lines());
        for index in first..self.0.len() - 1 {
            let next = lex(&Cow::from(text.line(index)), self.0[index]).1;
            if index >= first + added && self.0[index + 1] == next {
                break;
            }
            self.0[index + 1] = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replaces `range` of `text` the way a document does, keeping `states` current.
    fn edit(text: &mut Rope, states: &mut LineStates, range: Range<usize>, new: &str) {
        let first = text.byte_to_line(range.start);
        let removed = text.byte_to_line(range.end) - first;
        let start = text.byte_to_char(range.start);
        text.remove(start..text.byte_to_char(range.end));
        text.insert(start, new);
        let added = text.byte_to_line(range.start + new.len()) - first;
        states.edited(text, first, removed, added);
    }

    #[test]
    fn opening_a_comment_carries_until_it_closes() {
        let states = LineStates::of_rope(&Rope::from_str("a\n/* b\nc */\nd\n"));
        assert_eq!(
            states.0,
            [
                State::Code,
                State::Code,
                State::BlockComment,
                State::Code,
                State::Code
            ]
        );
    }

    #[test]
    fn edits_keep_the_states_the_same_as_lexing_from_scratch() {
        let mut text = Rope::from_str("a\nb\nc */\nd\ne\n");
        let mut states = LineStates::of_rope(&text);
        for (range, new) in [
            (0..0, "/* "),
            (3..3, "x\ny\n"),
            (0..3, ""),
            (4..12, "z"),
            (0..0, "\"\"\"\n"),
        ] {
            edit(&mut text, &mut states, range, new);
            assert_eq!(states.0, LineStates::of_rope(&text).0, "after {new:?}");
        }
    }
}

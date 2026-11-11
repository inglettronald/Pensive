//! Fuzzy matching for the palette's query, returning the matched letters as byte ranges.

use std::ops::Range;

use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};

pub(crate) struct Fuzzy {
    matcher: Matcher,
    pattern: Pattern,
}

impl Fuzzy {
    pub fn new(query: &str) -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
            pattern: Pattern::parse(query, CaseMatching::Smart, Normalization::Smart),
        }
    }

    /// The match's score and the byte ranges of `text` it matched, or `None` if it doesn't match.
    pub fn score(&mut self, text: &str) -> Option<(u32, Vec<Range<usize>>)> {
        let mut buffer = Vec::new();
        let mut indices = Vec::new();
        let score = self.pattern.indices(
            Utf32Str::new(text, &mut buffer),
            &mut self.matcher,
            &mut indices,
        )?;
        indices.sort_unstable();
        indices.dedup();

        let chars: Vec<Range<usize>> = text
            .char_indices()
            .map(|(start, ch)| start..start + ch.len_utf8())
            .collect();
        let mut ranges: Vec<Range<usize>> = Vec::new();
        for index in indices {
            let char = chars[index as usize].clone();
            match ranges.last_mut() {
                Some(last) if last.end == char.start => last.end = char.end,
                _ => ranges.push(char),
            }
        }
        Some((score, ranges))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_letters_merge_into_one_range() {
        let (_, ranges) = Fuzzy::new("gui").score("dulkir/gui-tooltips").unwrap();
        assert_eq!(ranges, vec![7..10]);
    }

    #[test]
    fn a_missing_letter_does_not_match() {
        assert!(Fuzzy::new("qwerty").score("multiver").is_none());
    }
}

//! How the editor styles a line's text: a color for each token, and a background on the words
//! that changed.

use std::ops::Range;

use gpui::{HighlightStyle, Hsla, combine_highlights};
use model::Token;
use theme::ThemeColors;

/// Styles for a line with `tokens`, and `words` that changed in `word_color`, in order and
/// without overlaps.
pub(super) fn line_styles(
    tokens: Vec<(Range<usize>, Token)>,
    words: Vec<Range<usize>>,
    word_color: Option<Hsla>,
    colors: &ThemeColors,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let tokens = tokens
        .into_iter()
        .map(|(range, token)| (range, HighlightStyle::color(token_color(token, colors))));
    let words = word_color.into_iter().flat_map(|color| {
        let style = HighlightStyle {
            background_color: Some(color),
            ..Default::default()
        };
        words.iter().map(move |range| (range.clone(), style))
    });
    combine_highlights(tokens, words).collect()
}

fn token_color(token: Token, colors: &ThemeColors) -> Hsla {
    match token {
        Token::Keyword => colors.syntax_keyword,
        Token::Type => colors.syntax_type,
        Token::Function => colors.syntax_function,
        Token::String => colors.syntax_string,
        Token::Number => colors.syntax_number,
        Token::Constant => colors.syntax_constant,
        Token::Comment => colors.syntax_comment,
        Token::Attribute => colors.syntax_attribute,
        Token::Operator => colors.syntax_operator,
        Token::Punctuation => colors.syntax_punctuation,
    }
}

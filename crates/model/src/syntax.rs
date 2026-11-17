//! Splits a line of code into tokens with one set of rules for every language, so any file gets
//! colors without a grammar for it.

mod line_states;

use std::ops::Range;

pub(crate) use line_states::LineStates;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Keyword,
    /// A name that starts with a capital, such as `LineElement`.
    Type,
    /// A name followed by `(`, or a Rust macro.
    Function,
    String,
    Number,
    /// `true`, `None` and the like, and names in capitals, such as `MAX_RUNS`.
    Constant,
    Comment,
    /// `#[derive(Debug)]`, `@dataclass` and `#include`.
    Attribute,
    Operator,
    Punctuation,
}

/// What a line starts inside, carried over from the line before. Only block comments and triple
/// quotes carry, so a stray quote can't color the rest of a file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum State {
    #[default]
    Code,
    BlockComment,
    /// A string opened by three of this quote.
    TripleQuoted(char),
}

/// Keywords of the common languages, all in one list.
const KEYWORDS: &str = "\
    abstract and as async await break case catch chan class const continue crate def default \
    defer del do dyn elif else enum except export extends extern final finally fn for from \
    func function global go goto if impl implements import in instanceof interface is lambda \
    let loop match mod move mut namespace new nonlocal not or override package pass private \
    protected pub public raise ref return select self Self sizeof static struct super switch \
    template this throw trait try type typename typeof unsafe use using var virtual void where \
    while with yield";

const CONSTANTS: &str = "true false True False null nil None undefined";

/// Preprocessor directives, which follow a `#` that would otherwise start a comment.
const DIRECTIVES: &str = "define elif else endif error if ifdef ifndef include pragma undef";

const OPERATORS: &str = "+-*/%=!<>&|^~?:";
const PUNCTUATION: &str = "()[]{},;.";

/// The tokens of `line`, which starts in `state`, and the state the next line starts in. Bytes
/// in no token, such as spaces and plain names, keep the text color.
pub(crate) fn lex(line: &str, state: State) -> (Vec<(Range<usize>, Token)>, State) {
    let mut lexer = Lexer {
        line,
        at: 0,
        tokens: Vec::new(),
    };
    let mut state = match state {
        State::Code => State::Code,
        State::BlockComment => lexer.block_comment(0),
        State::TripleQuoted(quote) => lexer.triple_quoted(0, quote),
    };
    while state == State::Code && lexer.at < line.len() {
        state = lexer.next();
    }
    (lexer.tokens, state)
}

struct Lexer<'a> {
    line: &'a str,
    /// The byte the next token starts at, or after.
    at: usize,
    tokens: Vec<(Range<usize>, Token)>,
}

impl Lexer<'_> {
    /// Reads the token at `self.at`, or steps over a byte that starts none.
    fn next(&mut self) -> State {
        let start = self.at;
        let rest = &self.line[start..];
        let Some(char) = rest.chars().next() else {
            return State::Code;
        };
        if rest.starts_with("//") {
            self.push_rest(start, Token::Comment);
        } else if rest.starts_with("/*") {
            self.at += 2;
            return self.block_comment(start);
        } else if let Some(quote) = ['"', '\'']
            .into_iter()
            .find(|&quote| rest.starts_with(&quote.to_string().repeat(3)))
        {
            self.at += 3;
            return self.triple_quoted(start, quote);
        } else if char == '#' {
            self.hash(start);
        } else if char == '@' && rest[1..].starts_with(is_name_start) {
            self.at += 1;
            self.skip_name();
            self.push(start, Token::Attribute);
        } else if char == '"' || char == '`' {
            self.at += 1;
            self.string(start, char);
        } else if char == '\'' {
            self.single_quote(start);
        } else if char.is_ascii_digit() {
            self.number(start);
        } else if is_name_start(char) {
            self.name(start);
        } else if OPERATORS.contains(char) {
            self.at += rest
                .find(|char| !OPERATORS.contains(char))
                .unwrap_or(rest.len());
            self.push(start, Token::Operator);
        } else if PUNCTUATION.contains(char) {
            self.at += 1;
            self.push(start, Token::Punctuation);
        } else {
            self.at += char.len_utf8();
        }
        State::Code
    }

    fn push(&mut self, start: usize, token: Token) {
        self.tokens.push((start..self.at, token));
    }

    fn push_rest(&mut self, start: usize, token: Token) {
        self.at = self.line.len();
        self.push(start, token);
    }

    fn block_comment(&mut self, start: usize) -> State {
        match self.line[self.at..].find("*/") {
            Some(end) => {
                self.at += end + 2;
                self.push(start, Token::Comment);
                State::Code
            }
            None => {
                self.push_rest(start, Token::Comment);
                State::BlockComment
            }
        }
    }

    fn triple_quoted(&mut self, start: usize, quote: char) -> State {
        match self.line[self.at..].find(&quote.to_string().repeat(3)) {
            Some(end) => {
                self.at += end + 3;
                self.push(start, Token::String);
                State::Code
            }
            None => {
                self.push_rest(start, Token::String);
                State::TripleQuoted(quote)
            }
        }
    }

    /// A string up to the next unescaped `quote`, or to the end of the line.
    fn string(&mut self, start: usize, quote: char) {
        let mut chars = self.line[self.at..].chars();
        while let Some(char) = chars.next() {
            if char == '\\' {
                chars.next();
            } else if char == quote {
                break;
            }
        }
        self.at = self.line.len() - chars.as_str().len();
        self.push(start, Token::String);
    }

    /// A string or a character, or a Rust lifetime such as `'a` in `&'a str`. It's a lifetime
    /// when a name follows that isn't closed by a quote, and either `&` or `<` comes before it
    /// or no other quote comes after it.
    fn single_quote(&mut self, start: usize) {
        self.at += 1;
        let name_start = self.at;
        self.skip_name();
        let named = self.at > name_start;
        let closed = self.line[self.at..].starts_with('\'');
        let before = self.line[..start].trim_end().chars().next_back();
        let quote_after = self.line[self.at..].contains('\'');
        if named && !closed && (matches!(before, Some('&' | '<')) || !quote_after) {
            self.push(start, Token::Keyword);
        } else {
            self.at = name_start;
            self.string(start, '\'');
        }
    }

    /// A `#[...]` or `#![...]` attribute, a preprocessor directive, or a comment.
    fn hash(&mut self, start: usize) {
        let rest = &self.line[start + 1..];
        if rest.starts_with(['[', '!']) {
            let mut depth = 0;
            for (offset, char) in rest.char_indices() {
                match char {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    _ => continue,
                }
                if depth == 0 {
                    self.at = start + 1 + offset + 1;
                    return self.push(start, Token::Attribute);
                }
            }
            return self.push_rest(start, Token::Attribute);
        }
        let word = rest.split(|char| !is_name_char(char)).next().unwrap_or("");
        if listed(DIRECTIVES, word) {
            self.at = start + 1 + word.len();
            self.push(start, Token::Attribute);
        } else {
            self.push_rest(start, Token::Comment);
        }
    }

    /// Digits, and the letters, underscores and decimal points in `0x1f`, `1_000` and `2.5f32`.
    /// A point counts only before a digit, so `0..10` is two numbers.
    fn number(&mut self, start: usize) {
        let rest = &self.line[start..];
        let mut end = rest.len();
        let mut chars = rest.char_indices().peekable();
        while let Some((offset, char)) = chars.next() {
            let decimal =
                char == '.' && chars.peek().is_some_and(|(_, next)| next.is_ascii_digit());
            if !is_name_char(char) && !decimal {
                end = offset;
                break;
            }
        }
        self.at = start + end;
        self.push(start, Token::Number);
    }

    /// A name, colored by what it looks like. A plain name gets no token.
    fn name(&mut self, start: usize) {
        self.skip_name();
        let word = &self.line[start..self.at];
        let after = &self.line[self.at..];
        let token = if listed(CONSTANTS, word) {
            Some(Token::Constant)
        } else if listed(KEYWORDS, word) {
            Some(Token::Keyword)
        } else if is_shouted(word) {
            Some(Token::Constant)
        } else if word.starts_with(char::is_uppercase) {
            Some(Token::Type)
        } else if after.starts_with('(') || after.starts_with("!(") || after.starts_with("![") {
            Some(Token::Function)
        } else {
            None
        };
        if let Some(token) = token {
            self.push(start, token);
        }
    }

    fn skip_name(&mut self) {
        let rest = &self.line[self.at..];
        self.at += rest.find(|char| !is_name_char(char)).unwrap_or(rest.len());
    }
}

/// Whether `word` is one of the space-separated words in `list`.
fn listed(list: &str, word: &str) -> bool {
    list.split_whitespace().any(|listed| listed == word)
}

fn is_name_start(char: char) -> bool {
    char.is_alphabetic() || char == '_' || char == '$'
}

fn is_name_char(char: char) -> bool {
    char.is_alphanumeric() || char == '_' || char == '$'
}

/// Whether `word` is in capitals, such as `MAX_RUNS`. A single capital is a type parameter.
fn is_shouted(word: &str) -> bool {
    word.chars().count() > 1
        && word.chars().any(char::is_uppercase)
        && !word.chars().any(char::is_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each token of `line` as its text, starting in `state`.
    fn tokens_in(line: &str, state: State) -> Vec<(&str, Token)> {
        lex(line, state)
            .0
            .into_iter()
            .map(|(range, token)| (&line[range], token))
            .collect()
    }

    fn tokens(line: &str) -> Vec<(&str, Token)> {
        tokens_in(line, State::Code)
    }

    #[test]
    fn names_are_colored_by_their_shape() {
        use Token::*;
        assert_eq!(
            tokens("let size: Pixels = rem_size() * SCALE;"),
            [
                ("let", Keyword),
                (":", Operator),
                ("Pixels", Type),
                ("=", Operator),
                ("rem_size", Function),
                ("(", Punctuation),
                (")", Punctuation),
                ("*", Operator),
                ("SCALE", Constant),
                (";", Punctuation),
            ]
        );
    }

    #[test]
    fn a_line_comment_runs_to_the_end_and_a_block_comment_to_its_close() {
        use Token::*;
        assert_eq!(
            tokens("a /* b */ c // d"),
            [("/* b */", Comment), ("// d", Comment)]
        );
        assert_eq!(tokens("# note"), [("# note", Comment)]);
    }

    #[test]
    fn a_hash_starts_an_attribute_before_a_bracket_or_a_directive() {
        use Token::*;
        assert_eq!(
            tokens("#[derive(Debug, Clone)] struct"),
            [("#[derive(Debug, Clone)]", Attribute), ("struct", Keyword)]
        );
        assert_eq!(tokens("#include <stdio.h>")[0], ("#include", Attribute));
        assert_eq!(tokens("@dataclass"), [("@dataclass", Attribute)]);
    }

    #[test]
    fn strings_skip_escaped_quotes_and_stop_at_the_line_end() {
        use Token::*;
        assert_eq!(
            tokens(r#"f("a \" b", 'c')"#),
            [
                ("f", Function),
                ("(", Punctuation),
                (r#""a \" b""#, String),
                (",", Punctuation),
                ("'c'", String),
                (")", Punctuation),
            ]
        );
        assert_eq!(lex("x = \"open", State::Code).1, State::Code);
    }

    #[test]
    fn a_quote_before_a_name_is_a_lifetime_unless_it_closes() {
        use Token::*;
        assert_eq!(
            tokens("fn f<'a>(x: &'a str)")
                .into_iter()
                .filter(|(_, token)| *token == Keyword)
                .collect::<Vec<_>>(),
            [("fn", Keyword), ("'a", Keyword), ("'a", Keyword)]
        );
        assert_eq!(tokens("['git', 'clone']")[1], ("'git'", String));
    }

    #[test]
    fn numbers_take_suffixes_and_decimals_but_not_ranges() {
        use Token::*;
        assert_eq!(
            tokens("0x1f 2.5f32 0..10"),
            [
                ("0x1f", Number),
                ("2.5f32", Number),
                ("0", Number),
                (".", Punctuation),
                (".", Punctuation),
                ("10", Number),
            ]
        );
    }

    #[test]
    fn block_comments_and_triple_quotes_carry_to_the_next_line() {
        use Token::*;
        let (_, state) = lex("x /* open", State::Code);
        assert_eq!(state, State::BlockComment);
        assert_eq!(
            tokens_in("still */ y()", state),
            [
                ("still */", Comment),
                ("y", Function),
                ("(", Punctuation),
                (")", Punctuation)
            ]
        );
        let (_, state) = lex(r#"doc = """Starts"#, State::Code);
        assert_eq!(state, State::TripleQuoted('"'));
        assert_eq!(lex("ends\"\"\"", state).1, State::Code);
    }
}

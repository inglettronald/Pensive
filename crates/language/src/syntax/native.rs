mod predicates;

use std::{cmp::Reverse, collections::BTreeSet, ops::Range};

use text::{Rope, TextChange, TextSnapshot};
use tree_sitter::{
    InputEdit, Language, Node, ParseOptions, ParseState, Parser, Point, Query, QueryCursor,
    QueryCursorOptions, QueryCursorState, StreamingIterator, Tree,
};

use crate::{TokenRole, TokenSpan};

use super::{MAX_SPANS, SyntaxCancellation, grammars};
use predicates::Predicate;

const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_PROGRESS_CALLBACKS: usize = 20_000;

pub(super) struct Grammar {
    language: Language,
    query: Query,
    roles: Vec<Option<TokenRole>>,
    predicates: Vec<Vec<Predicate>>,
}

impl Grammar {
    pub(super) fn new(language: Language, highlights: &str) -> Option<Self> {
        let query = Query::new(&language, highlights);
        #[cfg(test)]
        if let Err(error) = &query {
            eprintln!("{error:?}");
        }
        let mut query = query.ok()?;
        let roles = query
            .capture_names()
            .iter()
            .map(|name| token_role(name))
            .collect();
        let predicates = (0..query.pattern_count())
            .map(|index| {
                // Syntax alone supplies no property facts such as local bindings.
                if query
                    .property_predicates(index)
                    .iter()
                    .any(|(_, positive)| *positive)
                {
                    query.disable_pattern(index);
                }
                // QueryCursor only evaluates the built-in text predicates.
                query
                    .general_predicates(index)
                    .iter()
                    .map(Predicate::new)
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            language,
            query,
            roles,
            predicates,
        })
    }
}

pub(super) struct Native {
    grammar: &'static Grammar,
    parser: Parser,
    tree: Option<Tree>,
    markdown: bool,
    virtual_newline: bool,
}

impl Native {
    pub(super) fn new(language: &str) -> Option<Self> {
        let grammar = grammars::grammar(language)?;
        let mut parser = Parser::new();
        parser.set_language(&grammar.language).ok()?;
        Some(Self {
            grammar,
            parser,
            tree: None,
            markdown: language == "markdown",
            virtual_newline: false,
        })
    }

    pub(super) fn parse(
        &mut self,
        snapshot: &TextSnapshot,
        changes: Option<&[TextChange]>,
        cancel: &SyntaxCancellation,
    ) -> bool {
        let rope = snapshot.rope();
        let len = rope.len_bytes();
        if len > MAX_SOURCE_BYTES || cancel.is_cancelled() {
            return false;
        }
        // Markdown's block scanner needs a terminator for the final block.
        // A change in the virtual suffix invalidates incremental reuse.
        let virtual_newline = self.markdown && rope.bytes_at(len).prev() != Some(b'\n');
        let old = if self.virtual_newline == virtual_newline {
            changes.zip(self.tree.as_ref()).map(|(changes, tree)| {
                let mut tree = tree.clone();
                for edit in changes.iter().flat_map(|change| &change.edits) {
                    tree.edit(&InputEdit {
                        start_byte: edit.old.start,
                        old_end_byte: edit.old.end,
                        new_end_byte: edit.new.end,
                        start_position: Point::new(
                            edit.old_points.start.line,
                            edit.old_points.start.column,
                        ),
                        old_end_position: Point::new(
                            edit.old_points.end.line,
                            edit.old_points.end.column,
                        ),
                        new_end_position: Point::new(
                            edit.new_points.end.line,
                            edit.new_points.end.column,
                        ),
                    });
                }
                tree
            })
        } else {
            None
        };
        let mut remaining = MAX_PROGRESS_CALLBACKS;
        let mut progress = |_: &ParseState| out_of_budget(&mut remaining, cancel);
        let tree = self.parser.parse_with_options(
            &mut |byte, _| -> &[u8] {
                if byte < len {
                    chunk_from(rope, byte)
                } else if byte == len && virtual_newline {
                    b"\n"
                } else {
                    b""
                }
            },
            old.as_ref(),
            Some(ParseOptions::new().progress_callback(&mut progress)),
        );
        if cancel.is_cancelled() || tree.is_none() {
            // Tree-sitter can resume an interrupted parse. A future request may
            // have different input, so keep only the last completed tree.
            self.parser.reset();
            return false;
        }
        self.tree = tree;
        self.virtual_newline = virtual_newline;
        true
    }

    pub(super) fn tokens(
        &self,
        snapshot: &TextSnapshot,
        ranges: &[Range<usize>],
        cancel: &SyntaxCancellation,
    ) -> Vec<TokenSpan> {
        let Some(tree) = &self.tree else {
            return Vec::new();
        };
        let rope = snapshot.rope();
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(4096);
        let mut remaining = MAX_PROGRESS_CALLBACKS;
        let mut found = Vec::new();
        for range in ranges.iter().filter(|range| !range.is_empty()) {
            if cancel.is_cancelled() {
                return Vec::new();
            }
            cursor.set_byte_range(range.clone());
            let mut progress = |_: &QueryCursorState| out_of_budget(&mut remaining, cancel);
            {
                let mut captures = cursor.captures_with_options(
                    &self.grammar.query,
                    tree.root_node(),
                    |node: Node<'_>| node_text(rope, node),
                    QueryCursorOptions::new().progress_callback(&mut progress),
                );
                while let Some((matched, index)) = captures.next() {
                    if cancel.is_cancelled() || found.len() == MAX_SPANS {
                        return Vec::new();
                    }
                    let capture = matched.captures[*index];
                    let Some(role) = self.grammar.roles[capture.index as usize] else {
                        continue;
                    };
                    let bytes = capture.node.byte_range();
                    let start = bytes.start.max(range.start);
                    let end = bytes.end.min(range.end);
                    if start < end
                        && text::validate_range(rope, &(start..end)).is_ok()
                        && self.grammar.predicates[matched.pattern_index]
                            .iter()
                            .all(|predicate| predicate.matches(matched, rope))
                    {
                        found.push(Capture {
                            visible: start..end,
                            bytes,
                            role,
                            pattern: matched.pattern_index,
                            index: capture.index,
                        });
                    }
                }
            }
            if remaining == 0 || cancel.is_cancelled() || cursor.did_exceed_match_limit() {
                return Vec::new();
            }
        }
        let spans = flatten(snapshot, &found);
        if spans.len() <= MAX_SPANS {
            spans
        } else {
            Vec::new()
        }
    }
}

fn node_text<'a>(rope: &'a Rope, node: Node<'_>) -> impl Iterator<Item = &'a [u8]> + use<'a> {
    let bytes = node.byte_range();
    let len = rope.len_bytes();
    rope.get_byte_slice(bytes.start.min(len)..bytes.end.min(len))
        .into_iter()
        .flat_map(|slice| slice.chunks().map(str::as_bytes))
        .chain((bytes.end > len).then_some(&b"\n"[..]))
}

fn chunk_from(rope: &Rope, byte: usize) -> &[u8] {
    let (chunk, start, _, _) = rope.chunk_at_byte(byte);
    &chunk.as_bytes()[byte - start..]
}

fn out_of_budget(remaining: &mut usize, cancel: &SyntaxCancellation) -> bool {
    *remaining = remaining.saturating_sub(1);
    *remaining == 0 || cancel.is_cancelled()
}

struct Capture {
    visible: Range<usize>,
    bytes: Range<usize>,
    role: TokenRole,
    pattern: usize,
    index: u32,
}

impl Capture {
    fn precedence(&self) -> (Reverse<usize>, usize, u32, usize) {
        (
            Reverse(self.bytes.len()),
            self.pattern,
            self.index,
            self.bytes.start,
        )
    }
}

/// Narrower captures win, then later patterns. The last tie breakers are
/// intrinsic to the capture so clipping a query cannot change the winner.
fn flatten(snapshot: &TextSnapshot, captures: &[Capture]) -> Vec<TokenSpan> {
    let mut events = Vec::with_capacity(captures.len() * 2);
    for (id, capture) in captures.iter().enumerate() {
        events.push((capture.visible.start, true, id));
        events.push((capture.visible.end, false, id));
    }
    events.sort_unstable();
    let mut active = BTreeSet::new();
    let mut result = Vec::new();
    let mut run: Option<(Range<usize>, TokenRole)> = None;
    let mut previous = 0;
    for (position, enter, id) in events {
        if previous < position
            && let Some(&(_, best)) = active.last()
        {
            let best: usize = best;
            let role = captures[best].role;
            if let Some((bytes, _)) = run
                .as_mut()
                .filter(|(bytes, old_role)| bytes.end == previous && *old_role == role)
            {
                bytes.end = position;
            } else if let Some((bytes, role)) = run.replace((previous..position, role)) {
                result
                    .push(TokenSpan::new(snapshot, bytes, role).expect("validated capture bounds"));
            }
        }
        let priority = (captures[id].precedence(), id);
        if enter {
            active.insert(priority);
        } else {
            active.remove(&priority);
        }
        previous = position;
    }
    if let Some((bytes, role)) = run {
        result.push(TokenSpan::new(snapshot, bytes, role).expect("validated capture bounds"));
    }
    result
}

fn token_role(name: &str) -> Option<TokenRole> {
    let mut parts = name.split('.');
    let root = parts.next()?;
    let modifier = parts.next();
    Some(match root {
        "comment" => TokenRole::Comment,
        "string" if modifier == Some("attribute") => TokenRole::TagValue,
        "string" | "character" => TokenRole::String,
        "number" | "float" => TokenRole::Number,
        "keyword" | "boolean" | "conditional" | "repeat" | "exception" | "include" => {
            TokenRole::Keyword
        }
        "operator" => TokenRole::Operator,
        "variable" => match modifier {
            Some("parameter") => TokenRole::Parameter,
            Some("member") => TokenRole::Property,
            _ => TokenRole::Variable,
        },
        "parameter" => TokenRole::Parameter,
        "property" | "field" => TokenRole::Property,
        "constant" => TokenRole::Constant,
        "attribute" => TokenRole::Annotation,
        "type" if modifier == Some("parameter") => TokenRole::TypeParameter,
        "function" | "method" if modifier == Some("call") => TokenRole::FunctionCall,
        "function" | "method" | "constructor" => TokenRole::Function,
        "tag" if modifier == Some("attribute") => TokenRole::TagAttribute,
        "tag" => TokenRole::Tag,
        "type" | "namespace" | "module" | "label" => TokenRole::Type,
        "markup" | "text" => match name.split('.').nth(1)? {
            "heading" | "title" => TokenRole::Type,
            "raw" | "literal" | "uri" | "link" => TokenRole::String,
            "strong" | "bold" | "italic" | "emphasis" | "list" | "quote" => TokenRole::Keyword,
            _ => return None,
        },
        _ => return None,
    })
}

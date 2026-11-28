mod lexer;
mod parser;
mod source;
mod translation;
mod tree;

use std::{
    collections::BTreeMap,
    ops::Range,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use text::{TextChange, TextSnapshot};

use crate::{TokenRole, TokenSpan};

use super::{MAX_SPANS, SyntaxCancellation};
use lexer::{JavaToken, JavaTokenKind, Lexer};
use source::Source;
use tree::JavaTree;

#[cfg(not(target_arch = "wasm32"))]
const MAX_SOURCE_BYTES: usize = 1024 * 1024;
#[cfg(target_arch = "wasm32")]
const MAX_SOURCE_BYTES: usize = 16 * 1024;
#[cfg(not(target_arch = "wasm32"))]
const PARSE_POLLS: usize = 512;
#[cfg(target_arch = "wasm32")]
const PARSE_POLLS: usize = 12;

#[derive(Default)]
pub(super) struct Java {
    parsed: Option<Parsed>,
}

struct Parsed {
    source: Source,
    tokens: Arc<[JavaToken]>,
    index: Index,
}

#[derive(Default)]
struct Index {
    roles: BTreeMap<usize, TokenRole>,
    bodies: Vec<Range<usize>>,
    recoveries: bool,
}

impl Java {
    pub(super) fn parse(
        &mut self,
        snapshot: &TextSnapshot,
        changes: Option<&[TextChange]>,
        cancel: &SyntaxCancellation,
    ) -> bool {
        if snapshot.rope().len_bytes() > MAX_SOURCE_BYTES || cancel.is_cancelled() {
            return false;
        }
        let Some(source) = Source::new(snapshot.rope().to_string(), cancel) else {
            return false;
        };
        let reuse = changes
            .zip(self.parsed.as_ref())
            .and_then(|(changes, old)| {
                // Escapes and ignorable characters can affect translation across edits.
                if source.mapped() || old.source.mapped() {
                    return None;
                }
                let damage = changed_range(old.source.text.len(), source.text.len(), changes)?;
                Some((old, damage))
            });
        let prefix = reuse.as_ref().map_or(&[][..], |(old, damage)| {
            // Retreat by the scanner's maximum lookahead (e.g. `...` or `"""`).
            let start = damage.start.saturating_sub(3);
            let keep = old.tokens.partition_point(|token| token.bytes.end <= start);
            &old.tokens[..keep]
        });
        let start = prefix.last().map_or(0, |token| token.bytes.end);
        let mut lexer = Lexer::restart(prefix.to_vec(), start);
        if !lexer.advance(&source.text, MAX_SOURCE_BYTES * 3, cancel) {
            return false;
        }
        let tokens: Arc<[JavaToken]> = lexer.tokens.into();
        let index = reuse
            .and_then(|(old, damage)| reparse_body(old, &source, &tokens, &damage, cancel))
            .or_else(|| parse_index(&source, &tokens, None, cancel));
        let Some(index) = index else {
            return false;
        };
        self.parsed = Some(Parsed {
            source,
            tokens,
            index,
        });
        true
    }

    pub(super) fn tokens(
        &self,
        snapshot: &TextSnapshot,
        ranges: &[Range<usize>],
        cancel: &SyntaxCancellation,
    ) -> Vec<TokenSpan> {
        let Some(parsed) = &self.parsed else {
            return Vec::new();
        };
        let mut spans = Vec::new();
        for range in ranges.iter().filter(|range| !range.is_empty()) {
            let first = parsed
                .tokens
                .partition_point(|token| parsed.source.raw_byte(token.bytes.end) <= range.start);
            for token in &parsed.tokens[first..] {
                if cancel.is_cancelled() || spans.len() == MAX_SPANS {
                    return Vec::new();
                }
                let bytes = parsed.source.raw_bytes(token.bytes.clone());
                if bytes.start >= range.end {
                    break;
                }
                let role = match token.kind {
                    JavaTokenKind::Comment => Some(TokenRole::Comment),
                    JavaTokenKind::String | JavaTokenKind::Character => Some(TokenRole::String),
                    JavaTokenKind::Number => Some(TokenRole::Number),
                    JavaTokenKind::Keyword => Some(TokenRole::Keyword),
                    JavaTokenKind::Identifier => {
                        parsed.index.roles.get(&token.bytes.start).copied()
                    }
                    _ => None,
                };
                let clipped = bytes.start.max(range.start)..bytes.end.min(range.end);
                if let Some(role) = role
                    && !clipped.is_empty()
                {
                    spans.push(
                        TokenSpan::new(snapshot, clipped, role).expect("mapped Java token bounds"),
                    );
                }
            }
        }
        spans
    }
}

/// Enclose all sequential edits in the old source's coordinate space.
fn changed_range(old_len: usize, new_len: usize, changes: &[TextChange]) -> Option<Range<usize>> {
    let mut len = old_len;
    let mut prefix = old_len;
    let mut suffix = old_len;
    for edit in changes.iter().flat_map(|change| &change.edits) {
        prefix = prefix.min(edit.old.start);
        suffix = suffix.min(len.checked_sub(edit.old.end)?);
        len = len
            .checked_sub(edit.old.len())?
            .checked_add(edit.new.len())?;
    }
    (len == new_len).then(|| prefix..old_len - suffix)
}

fn reparse_body(
    old: &Parsed,
    source: &Source,
    tokens: &Arc<[JavaToken]>,
    damage: &Range<usize>,
    cancel: &SyntaxCancellation,
) -> Option<Index> {
    let i = old
        .index
        .bodies
        .partition_point(|body| body.start < damage.start)
        .checked_sub(1)?;
    let body = &old.index.bodies[i];
    if damage.end >= body.end {
        return None;
    }
    let delta = source.text.len() as isize - old.source.text.len() as isize;
    let new_body = body.start..body.end.checked_add_signed(delta)?;
    let updated = parse_index(source, tokens, Some(new_body.clone()), cancel)?;
    if updated.recoveries {
        return None;
    }
    // The standalone parser consumed the same delimiters, so declarations
    // outside this body retain their roles and only need an offset shift.
    let roles = old
        .index
        .roles
        .range(..body.start)
        .map(|(&byte, &role)| (byte, role))
        .chain(updated.roles)
        .chain(
            old.index
                .roles
                .range(body.end..)
                .map(|(&byte, &role)| ((byte as isize + delta) as usize, role)),
        )
        .collect();
    let mut bodies = old.index.bodies.clone();
    bodies[i] = new_body;
    for body in &mut bodies[i + 1..] {
        body.start = body.start.checked_add_signed(delta)?;
        body.end = body.end.checked_add_signed(delta)?;
    }
    Some(Index {
        roles,
        bodies,
        recoveries: false,
    })
}

fn parse_index(
    source: &Source,
    tokens: &Arc<[JavaToken]>,
    body: Option<Range<usize>>,
    cancel: &SyntaxCancellation,
) -> Option<Index> {
    let mut parser = parser::parse(source.text.clone(), tokens.clone(), cancel.clone(), body);
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..PARSE_POLLS {
        if cancel.is_cancelled() {
            return None;
        }
        if let Poll::Ready(tree) = parser.as_mut().poll(&mut cx) {
            return index_tree(source, &tree?, cancel);
        }
    }
    None
}

fn index_tree(source: &Source, tree: &JavaTree, cancel: &SyntaxCancellation) -> Option<Index> {
    let mut index = Index::default();
    let roles = &mut index.roles;
    let mut nodes = vec![tree.root_node()];
    while let Some(node) = nodes.pop() {
        if cancel.is_cancelled() {
            return None;
        }
        match node.kind() {
            "ERROR" | "MISSING" | "MISSING_AT_TOKEN" => index.recoveries = true,
            "type_identifier" => {
                roles.insert(node.start_byte(), TokenRole::Type);
            }
            "identifier"
                if node.parent().is_some_and(|parent| {
                    parent.kind() == "annotation"
                        && !source.text[parent.start_byte()..node.start_byte()].contains('(')
                }) =>
            {
                roles.insert(node.start_byte(), TokenRole::Annotation);
            }
            "field_access" => {
                if let Some(field) = node.child_by_field_name("field") {
                    roles.insert(field.start_byte(), TokenRole::Property);
                }
            }
            _ => {}
        }
        let role = match node.kind() {
            "class_declaration"
            | "interface_declaration"
            | "record_declaration"
            | "enum_declaration"
            | "annotation_type_declaration" => Some(TokenRole::Type),
            "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration" => Some(TokenRole::Method),
            "method_invocation" | "method_reference" => Some(TokenRole::MethodCall),
            "annotation" | "marker_annotation" => Some(TokenRole::Annotation),
            _ => None,
        };
        if let Some(role) = role
            && let Some(name) = node.child_by_field_name("name")
        {
            roles.insert(name.start_byte(), role);
        }
        if node.kind() == "method_declaration"
            && node
                .parent()
                .is_some_and(|parent| parent.kind() == "class_body")
            && node
                .parent()
                .and_then(|parent| parent.parent())
                .and_then(|owner| owner.parent())
                .is_some_and(|root| root.kind() == "program")
            && let Some(body) = node.child_by_field_name("body")
        {
            index.bodies.push(body.bytes());
        }
        nodes.extend(node.children().rev());
    }
    if index.recoveries {
        index.bodies.clear();
    }
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use text::{Rope, SourceId, SourceStamp, TextEdit};

    #[test]
    fn incrementally_reparses_a_method_body_and_shifts_later_roles() {
        let original = "class Example { void main() { this.old(); } void more() { more(); } }";
        let start = original.find("this.old").unwrap();
        let stamp = SourceStamp {
            source: SourceId::default(),
            revision: 0,
        };
        let mut rope = Rope::from_str(original);
        let input = TextSnapshot::new(stamp.clone(), &rope);
        let cancel = SyntaxCancellation::default();
        let mut java = Java::default();
        assert!(java.parse(&input, None, &cancel));

        let replacement = "Example.call";
        let range = start..start + "this.old".len();
        let edit = TextEdit::replacement(&rope, range.clone(), replacement).unwrap();
        rope.remove(range.clone());
        rope.insert(start, replacement);
        let change = TextChange {
            before: stamp.clone(),
            after: stamp.next(),
            edits: vec![edit],
        };
        let input = TextSnapshot::new(change.after.clone(), &rope);
        let mut cold = Java::default();
        assert!(cold.parse(&input, None, &cancel));
        let old = java.parsed.as_ref().unwrap();
        let new = cold.parsed.as_ref().unwrap();
        let updated = reparse_body(old, &new.source, &new.tokens, &range, &cancel).unwrap();
        assert_eq!(updated.roles, new.index.roles);
        assert_eq!(updated.bodies, new.index.bodies);
        assert!(java.parse(&input, Some(&[change]), &cancel));
        let range = 0..rope.len_bytes();
        assert_eq!(
            java.tokens(&input, std::slice::from_ref(&range), &cancel),
            cold.tokens(&input, &[range], &cancel)
        );

        let body = &new.index.bodies[0];
        assert!(
            reparse_body(
                new,
                &new.source,
                &new.tokens,
                &(body.start..body.start),
                &cancel
            )
            .is_none()
        );
    }
}

use super::*;
use crate::{LanguageRegistration, Registry, TokenRole};
use text::{Rope, SourceId, TextEdit};

fn stamp() -> SourceStamp {
    SourceStamp {
        source: SourceId::default(),
        revision: 0,
    }
}

fn snapshot(source: &str) -> TextSnapshot {
    TextSnapshot::new(stamp(), &Rope::from_str(source))
}

fn tokens(state: &mut SyntaxState, input: &TextSnapshot, selection: &Selection) -> Vec<TokenSpan> {
    state
        .tokens(
            input,
            selection,
            &[],
            0..input.rope().len_bytes(),
            &SyntaxCancellation::default(),
        )
        .unwrap()
}

fn has_role(source: &str, spans: &[TokenSpan], needle: &str, role: TokenRole) {
    let start = source.find(needle).unwrap();
    let end = start + needle.len();
    assert!(
        spans.iter().any(|span| span.role() == role
            && span.bytes().start <= start
            && span.bytes().end >= end),
        "missing {role:?} for {needle:?}: {spans:?}"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bundled_queries_compile_and_are_reachable_through_the_registry() {
    let registry = Registry::default();
    let mut failed = Vec::new();
    for &id in grammars::LANGUAGES {
        if native::Native::new(id).is_none() {
            eprintln!("incompatible query for {id}");
            failed.push(id);
        }
        let registration = crate::catalog::registrations()
            .into_iter()
            .find(|entry| entry.id == id)
            .unwrap();
        let selection = registry.resolve(&format!("file.{}", registration.extensions[0]));
        assert_eq!(selection.language.as_str(), id);
        assert_eq!(selection.syntax.as_deref(), Some(TREE_SITTER));
        assert_eq!(selection.semantic, None);
    }
    assert!(failed.is_empty(), "incompatible queries: {failed:?}");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bundled_queries_respect_predicates() {
    let registry = Registry::default();
    for (path, source, expected, rejected) in [
        (
            "CMakeLists.txt",
            "set(my_var value FOO)\n",
            ("FOO", TokenRole::Constant),
            ("value", TokenRole::Constant),
        ),
        (
            "main.odin",
            "package main\nCOUNT :: 1\nmain :: proc() { name := COUNT }\n",
            ("COUNT", TokenRole::Constant),
            ("name", TokenRole::Constant),
        ),
        (
            "struct.mm",
            "struct Example { int member; }; int local = 1;\n",
            ("member", TokenRole::Property),
            ("local", TokenRole::Property),
        ),
        (
            "build.groovy",
            "def local = 1\n",
            ("local", TokenRole::Variable),
            ("local", TokenRole::Parameter),
        ),
    ] {
        let spans = tokens(
            &mut SyntaxState::default(),
            &snapshot(source),
            &registry.resolve(path),
        );
        has_role(source, &spans, expected.0, expected.1);
        let start = source.find(rejected.0).unwrap();
        let end = start + rejected.0.len();
        assert!(
            !spans.iter().any(|span| span.role() == rejected.1
                && span.bytes().start < end
                && start < span.bytes().end),
            "{path}: unexpected {:?} for {:?}: {spans:?}",
            rejected.1,
            rejected.0
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn rust_precedence_and_hidden_multiline_context_are_preserved_in_byte_ranges() {
    let source = format!(
        "#[doc = \"déjà\"]\r\nfn read(n: usize) -> bool {{\r\n/*{}\r\n still 🦀 a comment\r\n*/ n == 42 || true\r\n}}",
        " x".repeat(2048)
    );
    let input = snapshot(&source);
    assert!(input.rope().chunks().count() > 1);
    let selection = Registry::default().resolve("main.rs");
    let mut state = SyntaxState::default();
    let spans = tokens(&mut state, &input, &selection);
    for (needle, role) in [
        ("#[doc", TokenRole::Annotation),
        ("\"déjà\"", TokenRole::String),
        ("fn", TokenRole::Keyword),
        ("read", TokenRole::Function),
        ("usize", TokenRole::Keyword),
        ("bool", TokenRole::Keyword),
        ("42", TokenRole::Number),
        ("true", TokenRole::Keyword),
    ] {
        has_role(&source, &spans, needle, role);
    }
    let start = source.find("🦀").unwrap();
    let end = source.find("*/").unwrap();
    let range = start..end;
    let cancel = SyntaxCancellation::default();
    for state in [&mut SyntaxState::default(), &mut state] {
        let spans = state
            .tokens(&input, &selection, &[], range.clone(), &cancel)
            .unwrap();
        assert_eq!(
            spans,
            [TokenSpan::new(&input, range.clone(), TokenRole::Comment).unwrap()]
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn sparse_queries_skip_a_gap_that_would_exhaust_the_capture_budget() {
    let first = "const FIRST: u32 = 1;\n";
    let last = "const LAST: u32 = 2;\n";
    let source = format!("{first}{}{last}", "// hidden\n".repeat(MAX_SPANS + 1));
    let input = snapshot(&source);
    let selection = Registry::default().resolve("main.rs");
    let mut state = SyntaxState::default();
    assert!(tokens(&mut state, &input, &selection).is_empty());
    let last_start = source.len() - last.len();
    let spans = state
        .tokens_for_ranges(
            &input,
            &selection,
            &[],
            &[0..first.len(), last_start..source.len()],
            &SyntaxCancellation::default(),
        )
        .unwrap();
    has_role(&source, &spans, "1", TokenRole::Number);
    assert!(spans.iter().any(|span| {
        span.role() == TokenRole::Number
            && span.bytes().start >= last_start
            && &source[span.bytes()] == "2"
    }));
    assert!(!spans.iter().any(|span| span.role() == TokenRole::Comment));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn xml_attributes_and_markdown_eof_use_source_bytes() {
    let registry = Registry::default();
    for (path, source, expected) in [
        (
            "file.xml",
            "<?xml version='1.0'?>\r\n<book name='café'>plain</book>",
            vec![
                ("<?xml", TokenRole::Tag),
                ("name=", TokenRole::TagAttribute),
                ("'café'", TokenRole::TagValue),
                ("</book>", TokenRole::Tag),
            ],
        ),
        (
            "readme.md",
            "# Final heading",
            vec![("Final heading", TokenRole::Type)],
        ),
    ] {
        let spans = tokens(
            &mut SyntaxState::default(),
            &snapshot(source),
            &registry.resolve(path),
        );
        for (needle, role) in expected {
            has_role(source, &spans, needle, role);
        }
        assert!(
            spans
                .windows(2)
                .all(|pair| pair[0].bytes().end <= pair[1].bytes().start)
        );
        assert!(spans.iter().all(|span| span.bytes().end <= source.len()));
    }
}

#[test]
fn java_highlights_raw_unicode_escape_spelling_and_parser_roles() {
    let source =
        "@Deprecated class Caf\\u00e9 {\r\n void r\\u0075n() { this.value = 1; r\\u0075n(); }\r\n}";
    let input = snapshot(source);
    let selection = Registry::default().resolve("Café.java");
    assert_eq!(selection.syntax.as_deref(), Some(JAVA));
    let spans = tokens(&mut SyntaxState::default(), &input, &selection);
    for (needle, role) in [
        ("Deprecated", TokenRole::Annotation),
        ("Caf\\u00e9", TokenRole::Type),
        ("r\\u0075n", TokenRole::Method),
        ("value", TokenRole::Property),
        ("class", TokenRole::Keyword),
        ("1", TokenRole::Number),
    ] {
        has_role(source, &spans, needle, role);
    }
    let at = source.rfind("r\\u0075n").unwrap();
    let range = at..at + "r\\u0075n".len();
    assert!(
        spans
            .iter()
            .any(|span| span.role() == TokenRole::MethodCall && span.bytes() == range)
    );
}

#[test]
fn java_cold_and_warm_queries_inside_a_hidden_text_block() {
    let source = "record Box(String name) {\r\n String sample() { return \"\"\"\r\n 🦀 class Fake {}\r\n \"\"\"; }\r\n}";
    let start = source.find("🦀").unwrap();
    let end = start + "🦀 class".len();
    let input = snapshot(source);
    let selection = Registry::default().resolve("Box.java");
    let mut state = SyntaxState::default();
    let cancel = SyntaxCancellation::default();
    let spans = state
        .tokens(&input, &selection, &[], start..end, &cancel)
        .unwrap();
    assert_eq!(
        spans,
        [TokenSpan::new(&input, start..end, TokenRole::String).unwrap()]
    );
    let wide = tokens(&mut state, &input, &selection);
    has_role(source, &wide, "Box", TokenRole::Type);
    has_role(source, &wide, "sample", TokenRole::Method);
    assert_eq!(
        state
            .tokens(&input, &selection, &[], start..end, &cancel)
            .unwrap(),
        spans
    );
    assert!(
        state
            .tokens(&input, &selection, &[], start + 1..end, &cancel)
            .is_err()
    );
}

fn change(rope: &mut Rope, before: &SourceStamp, edits: &[(Range<usize>, &str)]) -> TextChange {
    let mut records = Vec::new();
    for (range, inserted) in edits {
        records.push(TextEdit::replacement(rope, range.clone(), inserted).unwrap());
        let start = rope.byte_to_char(range.start);
        let end = rope.byte_to_char(range.end);
        rope.remove(start..end);
        rope.insert(start, inserted);
    }
    TextChange {
        before: before.clone(),
        after: before.next(),
        edits: records,
    }
}

#[test]
fn java_block_comment_spans_reach_eof_until_the_comment_is_closed() {
    let source = "class Example { /* hidden\r\n café class Ignored {}";
    let selection = Registry::default().resolve("Example.java");
    let mut rope = Rope::from_str(source);
    let input = TextSnapshot::new(stamp(), &rope);
    let mut state = SyntaxState::default();
    let spans = tokens(&mut state, &input, &selection);
    let comment = source.find("/*").unwrap()..source.len();
    assert!(spans.contains(&TokenSpan::new(&input, comment.clone(), TokenRole::Comment).unwrap()));

    let eof = source.len();
    let edit = change(&mut rope, input.stamp(), &[(eof..eof, " */ int value; }")]);
    let input = TextSnapshot::new(edit.after.clone(), &rope);
    let spans = state
        .tokens(
            &input,
            &selection,
            &[edit],
            0..rope.len_bytes(),
            &SyntaxCancellation::default(),
        )
        .unwrap();
    let comment = comment.start..eof + " */".len();
    assert!(spans.contains(&TokenSpan::new(&input, comment, TokenRole::Comment).unwrap()));
    has_role(&rope.to_string(), &spans, "int", TokenRole::Keyword);
}

#[test]
fn ordered_batches_and_missed_revisions_match_cold_tokens() {
    let registry = Registry::default();
    let examples = [
        (
            "Example.java",
            "class Example {\r\n void run() { run(); }\r\n}\r\n",
        ),
        #[cfg(not(target_arch = "wasm32"))]
        ("main.rs", "fn main() {\r\n print!(\"café\");\r\n}\r\n"),
    ];
    for (path, source) in examples {
        let selection = registry.resolve(path);
        let mut rope = Rope::from_str(source);
        let before = stamp();
        let input = TextSnapshot::new(before.clone(), &rope);
        let mut warm = SyntaxState::default();
        let mut missed = SyntaxState::default();
        assert!(!tokens(&mut warm, &input, &selection).is_empty());
        tokens(&mut missed, &input, &selection);

        let offset = source.find('{').unwrap() + 1;
        let first = change(
            &mut rope,
            &before,
            &[(offset..offset, " /* café */"), (0..0, "// 🦀\r\n")],
        );
        let second = change(
            &mut rope,
            &first.after,
            &[(3..7, "é"), (0..2, "/*"), (9..9, "*/")],
        );
        let next = TextSnapshot::new(second.after.clone(), &rope);
        let chain = [first, second];
        let current = next.stamp();
        assert_eq!(edit_chain(&before, current, &chain), Some(chain.as_slice()));
        assert!(edit_chain(&before, current, &chain[1..]).is_none());
        let cancel = SyntaxCancellation::default();
        let range = 0..rope.len_bytes();
        let expected = tokens(&mut SyntaxState::default(), &next, &selection);
        assert!(!expected.is_empty());
        assert_eq!(
            warm.tokens(&next, &selection, &chain, range.clone(), &cancel)
                .unwrap(),
            expected
        );
        assert_eq!(
            missed
                .tokens(&next, &selection, &chain[1..], range, &cancel)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn new_source_selection_or_cancelled_work_does_not_publish_old_spans() {
    let registry = Registry::default();
    let selection = registry.resolve("Source.java");
    let before = snapshot("class Old { }");
    let mut state = SyntaxState::default();
    assert!(!tokens(&mut state, &before, &selection).is_empty());
    let other = snapshot(" \t            ");
    assert_eq!(before.stamp().revision, other.stamp().revision);
    assert!(tokens(&mut state, &other, &selection).is_empty());

    let cancel = SyntaxCancellation::default();
    cancel.cancel();
    let range = 0..before.rope().len_bytes();
    assert!(
        state
            .tokens(&before, &selection, &[], range, &cancel)
            .unwrap()
            .is_empty()
    );
    assert_eq!(state.parsed.as_ref(), Some(other.stamp()));
    assert!(!tokens(&mut state, &before, &selection).is_empty());
    assert!(tokens(&mut state, &before, &registry.resolve("Source.unknown")).is_empty());

    let mut registry = registry.clone();
    let mut java = LanguageRegistration::new("java");
    java.extensions.push("java".into());
    java.providers.push(ProviderRegistration {
        id: "uninstalled".into(),
        capabilities: vec![Capability::Syntax],
        priority: 0,
    });
    registry.register(java).unwrap();
    assert!(tokens(&mut state, &before, &registry.resolve("Source.java")).is_empty());
}

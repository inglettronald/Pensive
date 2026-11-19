use std::ops::Range;

use text::{InvalidOffset, TextSnapshot};

/// Theme-independent roles shared by syntax and semantic providers.
/// Source coordinates follow Pensive's UTF-8 byte contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenRole {
    Keyword,
    String,
    Number,
    Comment,
    Type,
    Function,
    FunctionCall,
    Method,
    MethodCall,
    Variable,
    Parameter,
    Property,
    Constant,
    Annotation,
    TypeParameter,
    Operator,
    Plain,
    Tag,
    TagAttribute,
    TagValue,
    CapturedVariable,
}

/// A validated half-open UTF-8 byte range in a particular input snapshot.
/// The producer must retain that snapshot's stamp with its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenSpan {
    bytes: Range<usize>,
    role: TokenRole,
}

impl TokenSpan {
    pub fn new(
        snapshot: &TextSnapshot,
        bytes: Range<usize>,
        role: TokenRole,
    ) -> Result<Self, InvalidOffset> {
        text::validate_range(snapshot.rope(), &bytes)?;
        Ok(Self { bytes, role })
    }

    pub fn bytes(&self) -> Range<usize> {
        self.bytes.clone()
    }

    pub fn role(&self) -> TokenRole {
        self.role
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use text::{Rope, SourceId, SourceStamp};

    #[test]
    fn spans_use_bytes_and_reject_invalid_unicode_boundaries() {
        let snapshot = TextSnapshot::new(
            SourceStamp {
                source: SourceId::default(),
                revision: 0,
            },
            &Rope::from_str("aé😀\r\n"),
        );
        let span = TokenSpan::new(&snapshot, 1..7, TokenRole::String).unwrap();
        assert_eq!(span.bytes(), 1..7);
        assert_eq!(span.role(), TokenRole::String);
        for bytes in [1..2, 3..5, 0..10, Range { start: 7, end: 1 }] {
            assert!(TokenSpan::new(&snapshot, bytes, TokenRole::Plain).is_err());
        }
        assert!(TokenSpan::new(&snapshot, 9..9, TokenRole::Plain).is_ok());
    }
}

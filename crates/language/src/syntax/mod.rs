#[cfg(not(target_arch = "wasm32"))]
mod grammars;
mod java;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(test)]
mod tests;

use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use text::{InvalidOffset, SourceStamp, TextChange, TextSnapshot};

use crate::{Capability, ProviderRegistration, Selection, TokenSpan};

const JAVA: &str = "java-syntax";
#[cfg(not(target_arch = "wasm32"))]
const TREE_SITTER: &str = "tree-sitter";
const MAX_SPANS: usize = 65_536;

pub(super) fn registration(language: &str) -> Option<ProviderRegistration> {
    let id = match language {
        "java" => JAVA,
        #[cfg(not(target_arch = "wasm32"))]
        id if grammars::LANGUAGES.contains(&id) => TREE_SITTER,
        _ => return None,
    };
    Some(ProviderRegistration {
        id: id.into(),
        capabilities: vec![Capability::Syntax],
        priority: 0,
    })
}

#[derive(Clone, Default)]
pub struct SyntaxCancellation(Arc<AtomicBool>);

impl SyntaxCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// One source's reusable parse state. The caller holds one per diff side and
/// discards it when the source context or presence changes.
#[derive(Default)]
pub struct SyntaxState {
    provider: Option<(Selection, Provider)>,
    parsed: Option<SourceStamp>,
}

enum Provider {
    Java(java::Java),
    #[cfg(not(target_arch = "wasm32"))]
    Native(native::Native),
}

impl Provider {
    fn selected(selection: &Selection) -> Option<Self> {
        match selection.provider(Capability::Syntax)? {
            JAVA if selection.language.as_str() == "java" => {
                Some(Self::Java(java::Java::default()))
            }
            #[cfg(not(target_arch = "wasm32"))]
            TREE_SITTER => native::Native::new(selection.language.as_str()).map(Self::Native),
            _ => None,
        }
    }
}

impl SyntaxState {
    /// Parse the complete snapshot and return sorted, disjoint spans clipped to
    /// `range`. An unavailable provider or work limit returns plain text.
    /// Supply transactions in publication order. Reuse requires a continuous
    /// chain from the last completed parse through this snapshot. Cancelled
    /// work cannot replace the last completed parse with a partial one.
    pub fn tokens(
        &mut self,
        snapshot: &TextSnapshot,
        selection: &Selection,
        changes: &[TextChange],
        range: Range<usize>,
        cancel: &SyntaxCancellation,
    ) -> Result<Vec<TokenSpan>, InvalidOffset> {
        self.tokens_for_ranges(snapshot, selection, changes, &[range], cancel)
    }

    /// Query ordered, disjoint UTF-8 ranges against one full parse, with one
    /// work and output budget across all ranges.
    pub fn tokens_for_ranges(
        &mut self,
        snapshot: &TextSnapshot,
        selection: &Selection,
        changes: &[TextChange],
        ranges: &[Range<usize>],
        cancel: &SyntaxCancellation,
    ) -> Result<Vec<TokenSpan>, InvalidOffset> {
        validate_syntax_ranges(snapshot, ranges)?;
        if ranges.iter().all(Range::is_empty) || cancel.is_cancelled() {
            return Ok(Vec::new());
        }
        if self
            .provider
            .as_ref()
            .is_none_or(|(old, _)| old != selection)
        {
            self.provider =
                Provider::selected(selection).map(|provider| (selection.clone(), provider));
            self.parsed = None;
        }
        let Some((_, provider)) = &mut self.provider else {
            return Ok(Vec::new());
        };
        let stamp = snapshot.stamp();
        if self.parsed.as_ref() != Some(stamp) {
            let edits = self
                .parsed
                .as_ref()
                .and_then(|before| edit_chain(before, stamp, changes));
            let complete = match provider {
                Provider::Java(java) => java.parse(snapshot, edits, cancel),
                #[cfg(not(target_arch = "wasm32"))]
                Provider::Native(native) => native.parse(snapshot, edits, cancel),
            };
            if !complete {
                return Ok(Vec::new());
            }
            self.parsed = Some(stamp.clone());
        }
        Ok(match provider {
            Provider::Java(java) => java.tokens(snapshot, ranges, cancel),
            #[cfg(not(target_arch = "wasm32"))]
            Provider::Native(native) => native.tokens(snapshot, ranges, cancel),
        })
    }
}

pub fn validate_syntax_ranges(
    snapshot: &TextSnapshot,
    ranges: &[Range<usize>],
) -> Result<(), InvalidOffset> {
    let mut end = 0;
    for range in ranges {
        if range.start < end {
            return Err(InvalidOffset);
        }
        text::validate_range(snapshot.rope(), range)?;
        end = range.end;
    }
    Ok(())
}

fn edit_chain<'a>(
    before: &SourceStamp,
    after: &SourceStamp,
    changes: &'a [TextChange],
) -> Option<&'a [TextChange]> {
    if before.source != after.source || before.revision >= after.revision {
        return None;
    }
    let start = changes.iter().position(|change| &change.before == before)?;
    let mut expected = before;
    for (i, change) in changes.iter().enumerate().skip(start) {
        if &change.before != expected
            || change.after.source != before.source
            || change.before.revision.checked_add(1) != Some(change.after.revision)
            || change.edits.is_empty()
        {
            return None;
        }
        if &change.after == after {
            return Some(&changes[start..=i]);
        }
        expected = &change.after;
    }
    None
}

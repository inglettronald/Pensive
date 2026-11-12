//! How a notice looks: its tone, its origin as a tag, and as a tooltip on the control it's about.

use components::{Tag, Tone, Tooltip};
use model::{Notice, Origin, Severity};

pub(crate) fn tone(severity: Severity) -> Tone {
    match severity {
        Severity::Success => Tone::Success,
        Severity::Warning => Tone::Warning,
        Severity::Error => Tone::Error,
    }
}

/// Fixture is blue, so a gap in the sample data never reads as a bug in the interface.
pub(crate) fn tag(origin: Origin) -> Tag {
    let tag = Tag::new(origin.label());
    if origin == Origin::Fixture {
        tag.accent()
    } else {
        tag
    }
}

pub(crate) fn tooltip(notice: &Notice) -> Tooltip {
    let tooltip = Tooltip::new(notice.title.clone())
        .tone(tone(notice.severity))
        .tag(tag(notice.origin));
    match &notice.detail {
        Some(detail) => tooltip.detail(detail.clone()),
        None => tooltip,
    }
}

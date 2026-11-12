//! What Pensive tells the user about how something went, and whose fault a failure was.

use gpui::SharedString;

/// From good news to a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Success,
    Warning,
    Error,
}

/// Where a problem came from, so a gap in the sample data never reads as a bug in the interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The interface itself, such as a button with nothing behind it yet.
    Ui,
    Git,
    Config,
    /// A fake backend with no sample data for what was asked.
    Fixture,
}

impl Origin {
    /// The word on its tag.
    pub fn label(self) -> &'static str {
        match self {
            Origin::Ui => "UI",
            Origin::Git => "Git",
            Origin::Config => "Config",
            Origin::Fixture => "Fixture",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub severity: Severity,
    pub origin: Origin,
    pub title: SharedString,
    /// A second line, such as the error's message.
    pub detail: Option<SharedString>,
}

impl Notice {
    /// A warning that `action` has no handler yet, such as "Push isn't wired up yet".
    pub fn not_wired(action: &str) -> Self {
        Self {
            severity: Severity::Warning,
            origin: Origin::Ui,
            title: format!("{action} isn't wired up yet").into(),
            detail: None,
        }
    }
}

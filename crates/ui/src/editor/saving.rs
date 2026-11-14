//! The save setting, and when the editor saves.

use std::time::Duration;

use gpui::{App, Context, Window};
use serde::{Deserialize, Serialize};
use settings::{AnySetting, Choice, Control, Page, Setting, Settings};

use super::{Editor, keys::Save};

/// How long auto-save waits after the last edit.
pub(super) const AUTO_SAVE_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveMode {
    /// A second after typing stops, and on leaving the file or the window.
    Auto,
    /// Only on Ctrl S.
    Manual,
}

pub(crate) static SAVE: Setting<SaveMode> = Setting {
    key: "editor.save",
    name: "Save",
    description: "Auto saves a second after you stop typing and when you leave the file. Manual \
                  saves on Ctrl S",
    default: SaveMode::Auto,
    control: Control::Segmented(|_| {
        vec![
            Choice::new(SaveMode::Auto, "Auto"),
            Choice::new(SaveMode::Manual, "Manual"),
        ]
    }),
};

pub(crate) fn declare_settings(cx: &mut App) {
    let settings: [&'static dyn AnySetting; 1] = [&SAVE];
    Settings::declare(Page::new("Editor", "Saving"), settings, cx);
}

impl Editor {
    pub(super) fn save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(document) = &self.document {
            document.update(cx, |document, cx| document.save(cx));
        }
    }

    /// Saves now when auto-save is on, as leaving the file or the window does.
    pub(super) fn auto_save_now(&mut self, cx: &mut Context<Self>) {
        if SAVE.get(cx) == SaveMode::Auto
            && let Some(document) = &self.document
        {
            document.update(cx, |document, cx| document.save(cx));
        }
    }
}

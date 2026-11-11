//! The UI and code font families, and the font files bundled with the app.

use std::borrow::Cow;

use gpui::{App, SharedString};
use refineable::Refineable;

#[derive(Refineable, Clone, Debug)]
#[refineable(Debug, serde::Deserialize)]
pub struct ThemeFonts {
    /// Interface text.
    pub ui: SharedString,
    /// Code and diffs.
    pub code: SharedString,
}

impl ThemeFonts {
    pub fn default_pair() -> Self {
        Self {
            ui: "Inter".into(),
            code: "JetBrains Mono".into(),
        }
    }
}

macro_rules! bundled {
    ($($path:literal),* $(,)?) => {
        [$(include_bytes!(concat!("../../../assets/fonts/", $path)).as_slice()),*]
    };
}

/// Geist and Geist Mono ship too, so both pairings can be compared in the running app.
const BUNDLED: [&[u8]; 10] = bundled![
    "inter/Inter-Regular.ttf",
    "inter/Inter-Medium.ttf",
    "inter/Inter-SemiBold.ttf",
    "jetbrains-mono/JetBrainsMono-Regular.ttf",
    "jetbrains-mono/JetBrainsMono-Medium.ttf",
    "geist/Geist-Regular.ttf",
    "geist/Geist-Medium.ttf",
    "geist/Geist-SemiBold.ttf",
    "geist-mono/GeistMono-Regular.ttf",
    "geist-mono/GeistMono-Medium.ttf",
];

pub(crate) fn load_bundled(cx: &App) {
    cx.text_system()
        .add_fonts(BUNDLED.iter().map(|font| Cow::Borrowed(*font)).collect())
        .expect("the bundled fonts are valid");
}

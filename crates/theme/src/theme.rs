//! A named set of colors and fonts, and user overrides layered over it.

use gpui::{Global, SharedString};
use refineable::Refineable;
use serde::Deserialize;

use crate::{ThemeColors, ThemeColorsRefinement, ThemeFonts, ThemeFontsRefinement};

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: SharedString,
    pub colors: ThemeColors,
    pub fonts: ThemeFonts,
}

impl Global for Theme {}

/// Roles and fonts a user changes, written in TOML with the field names as keys:
///
/// ```toml
/// [colors]
/// text_muted = "#9a9a9a"
///
/// [fonts]
/// code = "Geist Mono"
/// ```
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThemeOverride {
    pub colors: ThemeColorsRefinement,
    pub fonts: ThemeFontsRefinement,
}

impl Theme {
    pub fn pensive_dark() -> Self {
        Self {
            name: "Pensive Dark".into(),
            colors: ThemeColors::dark(),
            fonts: ThemeFonts::default_pair(),
        }
    }

    pub fn with_override(&self, theme_override: &ThemeOverride) -> Self {
        let mut theme = self.clone();
        theme.colors.refine(&theme_override.colors);
        theme.fonts.refine(&theme_override.fonts);
        theme
    }
}

impl ThemeOverride {
    pub fn from_toml(source: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::rgb;

    #[test]
    fn override_changes_only_the_roles_it_names() {
        let theme_override = ThemeOverride::from_toml(
            r##"
            [colors]
            text_muted = "#9a9a9a"

            [fonts]
            code = "Geist Mono"
            "##,
        )
        .unwrap();
        let base = Theme::pensive_dark();
        let theme = base.with_override(&theme_override);

        assert_eq!(theme.colors.text_muted, rgb(0x9a9a9a).into());
        assert_eq!(theme.colors.text, base.colors.text);
        assert_eq!(theme.fonts.code, SharedString::from("Geist Mono"));
        assert_eq!(theme.fonts.ui, base.fonts.ui);
    }
}

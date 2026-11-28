//! A named set of colors and fonts, the themes Pensive ships, and the user themes layered over
//! them.

use std::collections::BTreeMap;

use gpui::{Global, SharedString};
use refineable::Refineable;
use serde::{Deserialize, de::DeserializeOwned};

use crate::{ThemeColors, ThemeColorsRefinement, ThemeFonts, ThemeFontsRefinement};

/// A theme that ships with Pensive.
pub(crate) struct BuiltIn {
    /// The name `settings.toml` and `extends` use.
    pub(crate) key: &'static str,
    /// The name the app shows.
    pub(crate) name: &'static str,
    colors: fn() -> ThemeColors,
}

/// The themes that ship with Pensive. The first is the default, and the one a user theme extends
/// when it names none.
pub(crate) const BUILT_IN: [BuiltIn; 3] = [
    BuiltIn {
        key: "pensive-dark",
        name: "Pensive Dark",
        colors: ThemeColors::dark,
    },
    BuiltIn {
        key: "pensive-light",
        name: "Pensive Light",
        colors: ThemeColors::light,
    },
    BuiltIn {
        key: "pensive-paper",
        name: "Pensive Paper",
        colors: ThemeColors::paper,
    },
];

impl BuiltIn {
    fn find(key: &str) -> Option<&'static Self> {
        BUILT_IN.iter().find(|built_in| built_in.key == key)
    }

    pub(crate) fn theme(&self) -> Theme {
        Theme {
            name: self.name.into(),
            colors: (self.colors)(),
            fonts: ThemeFonts::default_pair(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: SharedString,
    pub colors: ThemeColors,
    pub fonts: ThemeFonts,
}

impl Global for Theme {}

/// A user theme: the roles and fonts it changes, written in TOML with the field names as keys,
/// over the theme it names in `extends`:
///
/// ```toml
/// extends = "pensive-dark"
///
/// [colors]
/// text_muted = "#9a9a9a"
///
/// [fonts]
/// code = "Geist Mono"
/// ```
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThemeOverride {
    /// The theme this one changes. `pensive-dark` when it's left out.
    pub extends: Option<SharedString>,
    pub colors: ThemeColorsRefinement,
    pub fonts: ThemeFontsRefinement,
}

/// The user's themes, by name.
#[derive(Debug, Default)]
pub struct UserThemes(BTreeMap<SharedString, ThemeOverride>);

impl Global for UserThemes {}

impl Theme {
    pub fn pensive_dark() -> Self {
        BUILT_IN[0].theme()
    }

    pub fn pensive_light() -> Self {
        BUILT_IN[1].theme()
    }

    pub fn pensive_paper() -> Self {
        BUILT_IN[2].theme()
    }

    pub fn with_override(&self, theme_override: &ThemeOverride) -> Self {
        let mut theme = self.clone();
        theme.colors.refine(&theme_override.colors);
        theme.fonts.refine(&theme_override.fonts);
        theme
    }

    /// The theme called `name`: the built-in theme at the end of its `extends` chain, then each
    /// theme it extends, furthest first, then itself. Returns what went wrong, such as a missing
    /// theme or a loop of themes extending each other. The chain stops there, over the default.
    pub fn named(name: &str, themes: &UserThemes) -> (Self, Vec<String>) {
        let mut chain = Vec::new();
        let mut problems = Vec::new();
        let mut next = Some(name);
        let base = loop {
            let Some(name) = next else {
                break &BUILT_IN[0];
            };
            if let Some(built_in) = BuiltIn::find(name) {
                break built_in;
            }
            if chain.iter().any(|(seen, _)| *seen == name) {
                problems.push(format!(
                    "Theme `{name}` extends itself through other themes"
                ));
                break &BUILT_IN[0];
            }
            let Some(theme) = themes.0.get(name) else {
                problems.push(format!("There's no theme named `{name}`"));
                break &BUILT_IN[0];
            };
            chain.push((name, theme));
            next = theme.extends.as_deref();
        };
        let mut theme = chain
            .iter()
            .rev()
            .fold(base.theme(), |theme, (_, theme_override)| {
                theme.with_override(theme_override)
            });
        if !chain.is_empty() {
            theme.name = SharedString::from(name.to_string());
        }
        (theme, problems)
    }
}

impl ThemeOverride {
    /// Reads a theme file. Returns what's wrong with it too: keys that aren't roles or fonts, and
    /// values that don't parse, which are left out.
    pub fn from_toml(source: &str) -> Result<(Self, Vec<String>), toml::de::Error> {
        let mut table: toml::Table = source.parse()?;
        let mut problems = Vec::new();
        if let Some(toml::Value::Table(colors)) = table.get_mut("colors") {
            drop_invalid::<ThemeColorsRefinement>(colors, "colors.", &mut problems);
        }
        if let Some(toml::Value::Table(fonts)) = table.get_mut("fonts") {
            drop_invalid::<ThemeFontsRefinement>(fonts, "fonts.", &mut problems);
        }
        drop_invalid::<Self>(&mut table, "", &mut problems);
        let theme_override = serde_ignored::deserialize(toml::Value::Table(table), |path| {
            problems.push(format!("`{path}` isn't a theme key"))
        })?;
        Ok((theme_override, problems))
    }
}

/// Removes each entry of `entries` that doesn't parse as part of `T`. `prefix` is the path to
/// `entries` in the file, for reporting.
fn drop_invalid<T: DeserializeOwned>(
    entries: &mut toml::Table,
    prefix: &str,
    problems: &mut Vec<String>,
) {
    entries.retain(|key, value| {
        let entry = toml::Table::from_iter([(key.to_string(), value.clone())]);
        match toml::Value::Table(entry).try_into::<T>() {
            Ok(_) => true,
            Err(error) => {
                problems.push(settings::bad_value(
                    &format!("{prefix}{key}"),
                    error.message(),
                ));
                false
            }
        }
    });
}

impl UserThemes {
    /// Replaces the themes with the files given, as each file's name without `.toml` and its
    /// text. A file that isn't valid TOML keeps its last good version. Returns what's wrong with
    /// the files, each problem naming its file.
    pub fn load(&mut self, files: impl IntoIterator<Item = (String, String)>) -> Vec<String> {
        let mut themes = BTreeMap::new();
        let mut problems = Vec::new();
        for (name, source) in files {
            if BuiltIn::find(&name).is_some() {
                problems.push(format!(
                    "{name}.toml is skipped because `{name}` is a built-in theme's name"
                ));
                continue;
            }
            match ThemeOverride::from_toml(&source) {
                Ok((theme, theme_problems)) => {
                    problems.extend(
                        theme_problems
                            .into_iter()
                            .map(|problem| format!("{name}.toml: {problem}")),
                    );
                    themes.insert(name.into(), theme);
                }
                Err(error) => {
                    problems.push(format!("{name}.toml: {error}"));
                    if let Some(last_good) = self.0.remove(name.as_str()) {
                        themes.insert(name.into(), last_good);
                    }
                }
            }
        }
        self.0 = themes;
        problems
    }

    pub fn names(&self) -> impl Iterator<Item = &SharedString> {
        self.0.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::rgb;

    #[test]
    fn override_changes_only_the_roles_it_names() {
        let (theme_override, problems) = ThemeOverride::from_toml(
            r##"
            [colors]
            text_muted = "#9a9a9a"

            [fonts]
            code = "Geist Mono"
            "##,
        )
        .unwrap();
        assert!(problems.is_empty());
        let base = Theme::pensive_dark();
        let theme = base.with_override(&theme_override);

        assert_eq!(theme.colors.text_muted, rgb(0x9a9a9a).into());
        assert_eq!(theme.colors.text, base.colors.text);
        assert_eq!(theme.fonts.code, SharedString::from("Geist Mono"));
        assert_eq!(theme.fonts.ui, base.fonts.ui);
    }

    fn load(themes: &mut UserThemes, files: &[(&str, &str)]) -> Vec<String> {
        themes.load(
            files
                .iter()
                .map(|(name, source)| (name.to_string(), source.to_string())),
        )
    }

    fn themes(files: &[(&str, &str)]) -> (UserThemes, Vec<String>) {
        let mut themes = UserThemes::default();
        let problems = load(&mut themes, files);
        (themes, problems)
    }

    #[test]
    fn a_theme_applies_over_the_themes_it_extends() {
        let (themes, problems) = themes(&[
            (
                "nord",
                "[colors]\ntext = \"#111111\"\ntext_muted = \"#222222\"",
            ),
            (
                "nord-warm",
                "extends = \"nord\"\n[colors]\ntext_muted = \"#333333\"",
            ),
        ]);
        assert!(problems.is_empty());
        let (theme, problems) = Theme::named("nord-warm", &themes);
        assert!(problems.is_empty());
        assert_eq!(theme.name, SharedString::from("nord-warm"));
        assert_eq!(theme.colors.text, rgb(0x111111).into());
        assert_eq!(theme.colors.text_muted, rgb(0x333333).into());
        assert_eq!(
            theme.colors.surface_app,
            Theme::pensive_dark().colors.surface_app
        );
    }

    #[test]
    fn a_theme_can_extend_any_built_in_theme() {
        let (themes, problems) = themes(&[(
            "dusk",
            "extends = \"pensive-light\"\n[colors]\ntext = \"#111111\"",
        )]);
        assert!(problems.is_empty());
        let (theme, problems) = Theme::named("dusk", &themes);
        assert!(problems.is_empty());
        assert_eq!(theme.colors.text, rgb(0x111111).into());
        assert_eq!(theme.colors.surface_app, ThemeColors::light().surface_app);
    }

    #[test]
    fn a_file_named_like_a_built_in_theme_is_skipped() {
        let (themes, problems) = themes(&[("pensive-light", "[colors]\ntext = \"#111111\"")]);
        assert_eq!(problems.len(), 1);
        let (theme, _) = Theme::named("pensive-light", &themes);
        assert_eq!(theme.colors.text, ThemeColors::light().text);
    }

    #[test]
    fn a_loop_of_themes_stops_with_a_problem() {
        let (themes, _) = themes(&[("a", "extends = \"b\""), ("b", "extends = \"a\"")]);
        let (_, problems) = Theme::named("a", &themes);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn a_missing_theme_falls_back_to_the_base() {
        let (theme, problems) = Theme::named("nord", &UserThemes::default());
        assert_eq!(theme.name, Theme::pensive_dark().name);
        assert_eq!(problems, ["There's no theme named `nord`"]);
    }

    #[test]
    fn typos_and_bad_colors_are_reported_and_left_out() {
        let (theme_override, problems) = ThemeOverride::from_toml(
            r##"
            [colors]
            text_mute = "#9a9a9a"
            text = "blue"
            border = "#353535"
            "##,
        )
        .unwrap();
        assert_eq!(problems.len(), 2);
        assert!(problems[0].starts_with("`colors.text` is ignored because"));
        assert_eq!(problems[1], "`colors.text_mute` isn't a theme key");
        assert!(theme_override.colors.text.is_none());
        assert!(theme_override.colors.border.is_some());
    }

    #[test]
    fn a_bad_top_level_value_is_left_out_and_the_rest_applies() {
        let (theme_override, problems) = ThemeOverride::from_toml(
            "extends = 5
[colors]
text = \"#111111\"",
        )
        .unwrap();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].starts_with("`extends` is ignored because"));
        assert!(theme_override.extends.is_none());
        assert!(theme_override.colors.text.is_some());
    }

    #[test]
    fn a_theme_file_that_isnt_valid_toml_keeps_its_last_good_version() {
        let (mut themes, _) = themes(&[("nord", "[colors]\ntext = \"#111111\"")]);
        let problems = load(&mut themes, &[("nord", "[colors")]);
        assert_eq!(problems.len(), 1);
        let (theme, _) = Theme::named("nord", &themes);
        assert_eq!(theme.colors.text, rgb(0x111111).into());
    }
}

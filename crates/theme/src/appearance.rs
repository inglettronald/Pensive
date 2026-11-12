//! The appearance settings, and the theme and density in use built from them.

use gpui::{App, SharedString};
use settings::{AnySetting, Choice, Control, Setting, Settings};
use strum::VariantArray;

use crate::{BASE_THEME, DensityPreset, Theme, UserThemes};

pub static THEME: Setting<SharedString> = Setting {
    key: "theme",
    section: "Appearance",
    name: "Theme",
    description: "The colors, and the fonts unless they're set below",
    default: SharedString::new_static(BASE_THEME),
    control: Control::Select(|cx| {
        let user_themes = cx
            .try_global::<UserThemes>()
            .into_iter()
            .flat_map(UserThemes::names);
        [Choice::new(BASE_THEME.into(), Theme::pensive_dark().name)]
            .into_iter()
            .chain(user_themes.map(|name| Choice::new(name.clone(), name.clone())))
            .collect()
    }),
};

pub static DENSITY: Setting<DensityPreset> = Setting {
    key: "density",
    section: "Appearance",
    name: "Density",
    description: "Calm has larger text and 28 px rows",
    default: DensityPreset::Calm,
    control: Control::Segmented(|_| {
        DensityPreset::VARIANTS
            .iter()
            .map(|preset| Choice::new(*preset, <&str>::from(preset)))
            .collect()
    }),
};

pub static UI_FONT: Setting<Option<SharedString>> = Setting {
    key: "fonts.ui",
    section: "Appearance",
    name: "Interface font",
    description: "Menus, labels and body text",
    default: None,
    control: Control::Select(|_| fonts(["Inter", "Geist"])),
};

pub static CODE_FONT: Setting<Option<SharedString>> = Setting {
    key: "fonts.code",
    section: "Appearance",
    name: "Code font",
    description: "Code and diffs",
    default: None,
    control: Control::Select(|_| fonts(["JetBrains Mono", "Geist Mono"])),
};

/// The theme's font, then the bundled ones. Any installed font works when written in the file.
fn fonts(bundled: [&'static str; 2]) -> Vec<Choice<Option<SharedString>>> {
    [Choice::new(None, "From the theme")]
        .into_iter()
        .chain(bundled.map(|font| Choice::new(Some(font.into()), font)))
        .collect()
}

pub(crate) fn declare(cx: &mut App) {
    let settings: [&'static dyn AnySetting; 4] = [&THEME, &DENSITY, &UI_FONT, &CODE_FONT];
    Settings::declare(settings, cx);
}

/// Installs the theme and density the settings ask for. Returns what went wrong building the
/// theme.
pub fn apply(cx: &mut App) -> Vec<String> {
    cx.default_global::<UserThemes>();
    let (mut theme, problems) = Theme::named(&THEME.get(cx), cx.global::<UserThemes>());
    if let Some(font) = UI_FONT.get(cx) {
        theme.fonts.ui = font;
    }
    if let Some(font) = CODE_FONT.get(cx) {
        theme.fonts.code = font;
    }
    cx.set_global(theme);
    cx.set_global(DENSITY.get(cx).density());
    problems
}

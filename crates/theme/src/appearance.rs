//! The appearance settings, and installing the theme and density they choose.

use gpui::{App, SharedString};
use settings::{AnySetting, Choice, Control, Page, Setting, Settings};
use strum::VariantArray;

use crate::{BASE_THEME, BASE_THEME_NAME, DensityPreset, Theme, UserThemes};

pub(crate) static THEME: Setting<SharedString> = Setting {
    key: "theme",
    name: "Theme",
    description: "The colors, and the fonts unless they're set below",
    default: SharedString::new_static(BASE_THEME),
    control: Control::Select(|cx| {
        let user_themes = cx
            .try_global::<UserThemes>()
            .into_iter()
            .flat_map(UserThemes::names);
        [Choice::new(BASE_THEME.into(), BASE_THEME_NAME)]
            .into_iter()
            .chain(user_themes.map(|name| Choice::new(name.clone(), name.clone())))
            .collect()
    }),
};

pub(crate) static DENSITY: Setting<DensityPreset> = Setting {
    key: "density",
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

pub(crate) static UI_FONT: Setting<Option<SharedString>> = Setting {
    key: "fonts.ui",
    name: "Interface font",
    description: "Menus, labels and body text",
    default: None,
    control: Control::Select(|_| fonts(["Inter", "Geist"])),
};

pub(crate) static CODE_FONT: Setting<Option<SharedString>> = Setting {
    key: "fonts.code",
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
    let page = |name| Page::new("Appearance", name);
    Settings::declare(page("Theme"), [&THEME as &dyn AnySetting], cx);
    Settings::declare(page("Size"), [&DENSITY as &dyn AnySetting], cx);
    let fonts: [&'static dyn AnySetting; 2] = [&UI_FONT, &CODE_FONT];
    Settings::declare(page("Fonts"), fonts, cx);
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

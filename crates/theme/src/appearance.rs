//! The appearance settings, and installing the theme, density and zoom they choose.

use gpui::{App, Pixels, SharedString};
use settings::{AnySetting, Choice, Control, Page, Setting, Settings};
use strum::VariantArray;

use crate::{BASE_THEME, BASE_THEME_NAME, DensityPreset, Theme, UserThemes, dp};

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

/// The zoom levels, in percent.
const ZOOM_LEVELS: [u32; 8] = [80, 90, 100, 110, 125, 150, 175, 200];

pub(crate) static ZOOM: Setting<u32> = Setting {
    key: "zoom",
    name: "Zoom",
    description: "Everything in the window. Ctrl = and Ctrl - change it",
    default: 100,
    control: Control::Stepper(|_| {
        ZOOM_LEVELS
            .iter()
            .map(|zoom| Choice::new(*zoom, format!("{zoom}%")))
            .collect()
    }),
};

/// Code sizes in pixels at 100% zoom. Unset follows Density, so the default is listed as unset
/// at Density's size.
pub(crate) static CODE_SIZE: Setting<Option<u32>> = Setting {
    key: "code_size",
    name: "Code size",
    description: "Code and diffs at 100% zoom. Follows Density until it's set",
    default: None,
    control: Control::Stepper(|cx| {
        let from_density = DENSITY.get(cx).density().code.size.0 as u32;
        (10..=24)
            .map(|size| Choice::new((size != from_density).then_some(size), format!("{size} px")))
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
    let size: [&'static dyn AnySetting; 3] = [&DENSITY, &ZOOM, &CODE_SIZE];
    Settings::declare(page("Size"), size, cx);
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
    let mut density = DENSITY.get(cx).density();
    if let Some(size) = CODE_SIZE.get(cx) {
        let size = size as f32;
        density.code.size = dp(size);
        density.code.line_height = dp((size * 1.5).round());
    }
    cx.set_global(density);
    problems
}

/// The window's rem size at the zoom the settings ask for. Everything sized in `Dp` follows it.
pub fn rem_size(cx: &App) -> Pixels {
    crate::dp::zoomed_rem(ZOOM.get(cx) as f32 / 100.)
}

/// Moves the zoom `step` levels, stopping at the smallest and largest.
pub fn step_zoom(step: isize, cx: &mut App) {
    ZOOM.step(step, cx);
    cx.refresh_windows();
}

pub fn reset_zoom(cx: &mut App) {
    ZOOM.reset(cx);
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;
    use settings::ControlState;

    use super::*;
    use crate::{ActiveTheme, Dp};

    #[gpui::test]
    fn code_size_follows_density_until_it_is_set(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init(cx);
            let label = |cx: &App| match CODE_SIZE.state(cx) {
                ControlState::Stepper { label, .. } => label,
                state => panic!("code size shows {state:?}"),
            };
            assert_eq!(label(cx), "13 px");

            CODE_SIZE.step(1, cx);
            apply(cx);
            assert_eq!(CODE_SIZE.get(cx), Some(14));
            assert_eq!(cx.density().code.line_height, Dp(21.));

            CODE_SIZE.step(-1, cx);
            assert_eq!(CODE_SIZE.get(cx), None);
            DENSITY.set(DensityPreset::Dense, cx);
            assert_eq!(label(cx), "12 px");
        });
    }
}

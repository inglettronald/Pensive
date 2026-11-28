//! The appearance settings, and installing the theme, density and zoom they choose.

use gpui::{App, Pixels, SharedString};
use settings::{AnySetting, Category, Choice, Control, Page, Setting, Settings};
use strum::VariantArray;

use crate::{BUILT_IN, DensityPreset, Theme, UserThemes, dp};

pub static APPEARANCE: Category = Category::new("Appearance", "Theme, size and fonts");

pub(crate) static THEME: Setting<SharedString> = Setting {
    key: "theme",
    name: "Theme",
    description: "The colors, and the fonts unless they're set below",
    default: SharedString::new_static(BUILT_IN[0].key),
    control: Control::Select(|cx| {
        let user_themes = cx
            .try_global::<UserThemes>()
            .into_iter()
            .flat_map(UserThemes::names);
        BUILT_IN
            .iter()
            .map(|built_in| Choice::new(built_in.key.into(), built_in.name))
            .chain(user_themes.map(|name| Choice::new(name.clone(), name.clone())))
            .collect()
    }),
};

pub(crate) static DENSITY: Setting<DensityPreset> = Setting {
    key: "density",
    name: "Density",
    description: "Calm has larger text and 30 px rows",
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

/// Code sizes in pixels at 100% zoom. Unset follows Density, so it's the choice at Density's size,
/// and a file can still name that size.
pub(crate) static CODE_SIZE: Setting<Option<u32>> = Setting {
    key: "code_size",
    name: "Code size",
    description: "Code and diffs at 100% zoom. Follows Density until it's set",
    default: None,
    control: Control::Stepper(|cx| {
        let from_density = DENSITY.get(cx).density().code.size.0 as u32;
        (10..=24)
            .map(|size| {
                let label = format!("{size} px");
                if size == from_density {
                    Choice::new(None, label).also(Some(size))
                } else {
                    Choice::new(Some(size), label)
                }
            })
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
    let page = |name| Page::new(&APPEARANCE, name);
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

/// Sets the density and zoom, as Settings would.
pub fn set_size(density: DensityPreset, zoom: u32, cx: &mut App) {
    DENSITY.set(density, cx);
    ZOOM.set(zoom, cx);
    apply(cx);
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
                ControlState::Stepper { label, .. } => label.to_string(),
                state => panic!("code size shows {state:?}"),
            };
            let size_of = |preset: DensityPreset| preset.density().code.size.0 as u32;
            let calm = size_of(DensityPreset::Calm);
            let dense = size_of(DensityPreset::Dense);
            assert_ne!(
                calm, dense,
                "the presets need different code sizes to tell apart"
            );
            assert_eq!(label(cx), format!("{calm} px"));

            CODE_SIZE.step(1, cx);
            apply(cx);
            assert_eq!(CODE_SIZE.get(cx), Some(calm + 1));
            assert_eq!(cx.density().code.size, Dp((calm + 1) as f32));

            CODE_SIZE.step(-1, cx);
            assert_eq!(CODE_SIZE.get(cx), None);
            DENSITY.set(DensityPreset::Dense, cx);
            assert_eq!(label(cx), format!("{dense} px"));
        });
    }

    #[gpui::test]
    fn the_file_can_name_any_code_size_in_range(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init(cx);
            for density in ["calm", "dense"] {
                for size in [10, 12, 14, 24] {
                    let source = format!("density = \"{density}\"\ncode_size = {size}\n");
                    assert!(Settings::load(&source, cx).is_empty(), "{source}");
                    assert_eq!(CODE_SIZE.get(cx), Some(size));
                }
            }
            assert!(Settings::load("code_size = 14\n", cx).is_empty());
            assert_eq!(
                CODE_SIZE.state(cx),
                ControlState::Stepper {
                    label: "14 px".into(),
                    can_decrease: true,
                    can_increase: true,
                }
            );
            for size in [9, 25] {
                let problems = Settings::load(&format!("code_size = {size}\n"), cx);
                assert_eq!(problems.len(), 1, "{problems:?}");
                assert!(
                    problems[0].contains("expected one of 10, 11"),
                    "{problems:?}"
                );
                assert_eq!(CODE_SIZE.get(cx), None);
            }
        });
    }

    #[gpui::test]
    fn themes_and_fonts_can_be_ones_the_lists_dont_have_yet(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init(cx);
            let source = "theme = \"nord\"\n[fonts]\ncode = \"Fira Code\"\n";
            assert!(Settings::load(source, cx).is_empty());
            assert_eq!(THEME.get(cx), SharedString::from("nord"));
            assert_eq!(CODE_FONT.get(cx), Some("Fira Code".into()));
        });
    }
}

//! The colors, fonts and sizes that components read, with user themes and the appearance
//! settings.

mod appearance;
mod colors;
mod density;
mod dp;
mod fonts;
mod shadow;
mod theme;

use gpui::App;

pub use appearance::{apply, rem_size, reset_zoom, step_zoom};
pub use colors::{Role, ThemeColors, ThemeColorsRefinement, gray};
pub use density::{Density, DensityPreset, TextRole, TextSize, Typeset};
pub use dp::{CARET, Dp, HAIRLINE, dp};
pub use fonts::{ThemeFonts, ThemeFontsRefinement};
pub use shadow::Shadow;
/// Lists every value of `DensityPreset` and `TextRole`, for galleries.
pub use strum::VariantArray;
use theme::{BASE_THEME, BASE_THEME_NAME};
pub use theme::{Theme, ThemeOverride, UserThemes};

/// Loads the bundled fonts, declares the appearance settings, and installs the default theme and
/// density.
pub fn init(cx: &mut App) {
    fonts::load_bundled(cx);
    appearance::declare(cx);
    cx.set_global(Theme::pensive_dark());
    cx.set_global(DensityPreset::Calm.density());
}

/// Reads the theme and density in use. Implemented for `App`, so every `cx` has it.
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
    fn density(&self) -> &Density;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        self.global::<Theme>()
    }

    fn density(&self) -> &Density {
        self.global::<Density>()
    }
}

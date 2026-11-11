//! Views: the screens and panels that read models and own their view state.

mod palette;
mod root;
mod text_input;

use gpui::App;

pub use palette::{Keyword, PaletteEvent, PaletteView, Tab};
pub use root::{Pensive, open_window};

/// Binds every view's keys.
pub fn init(cx: &mut App) {
    text_input::bind_keys(cx);
    palette::bind_keys(cx);
    root::bind_keys(cx);
}

/// Every view's mocks.
#[cfg(feature = "mocks")]
pub fn mocks() -> Vec<mock::Mock> {
    root::mocks()
}

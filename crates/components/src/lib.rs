//! Stateless building blocks that render from props and the theme.

mod command_palette;
#[cfg(feature = "mocks")]
mod gallery;
mod icon;
mod kbd;
mod segmented_control;
mod select;
mod tab_strip;
#[cfg(feature = "mocks")]
mod theme_gallery;
mod toggle;

use std::rc::Rc;

use gpui::{App, Window};

pub use command_palette::{
    CommandPalette, KeyHint, Leading, PaletteChip, PaletteEntry, PaletteSection, TrailingText,
};
#[cfg(feature = "mocks")]
pub use gallery::Gallery;
pub use icon::{Icon, IconName};
pub use kbd::Kbd;
pub use segmented_control::SegmentedControl;
pub use select::Select;
pub use tab_strip::TabStrip;
pub use toggle::Toggle;

/// How a component reports a change: called with the new value. An `Rc`, so a component can
/// share one callback among its children.
pub type Callback<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// Every component's mocks.
#[cfg(feature = "mocks")]
pub fn mocks() -> Vec<mock::Mock> {
    [
        theme_gallery::mocks(),
        command_palette::mocks(),
        icon::mocks(),
        kbd::mocks(),
        toggle::mocks(),
        select::mocks(),
        segmented_control::mocks(),
        tab_strip::mocks(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

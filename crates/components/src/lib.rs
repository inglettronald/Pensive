//! Stateless building blocks that render from props and the theme.

mod breadcrumb;
mod button;
mod command_palette;
mod diff;
mod diff_stat;
mod fade_text;
#[cfg(feature = "mocks")]
mod gallery;
mod icon;
mod icon_button;
mod kbd;
mod list_row;
mod segmented_control;
mod select;
mod status_ring;
mod tab_strip;
mod tag;
#[cfg(feature = "mocks")]
mod theme_gallery;
mod timeline_row;
mod title_bar;
mod toggle;
mod tooltip;

use std::rc::Rc;

use gpui::{App, Window};

pub use breadcrumb::Breadcrumb;
pub use button::{Button, ButtonStyle};
pub use command_palette::{
    CommandPalette, KeyHint, Leading, PaletteChip, PaletteEntry, PaletteSection, TrailingText,
};
pub use diff::{Diff, DiffLine, LineKind};
pub use diff_stat::DiffStat;
pub use fade_text::FadeText;
#[cfg(feature = "mocks")]
pub use gallery::Gallery;
pub use icon::{Icon, IconName};
pub use icon_button::IconButton;
pub use kbd::Kbd;
pub use list_row::ListRow;
pub use segmented_control::SegmentedControl;
pub use select::Select;
pub use status_ring::{RingFill, StatusRing};
pub use tab_strip::TabStrip;
pub use tag::Tag;
pub use timeline_row::{TimelineLine, TimelineNode, TimelineRow};
pub use title_bar::{TitleBar, TitleBarProject};
pub use toggle::Toggle;
pub use tooltip::{Tone, Tooltip};

/// How a component reports a change: called with the new value. An `Rc`, so a component can
/// share one callback among its children.
pub type Callback<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// A [`Callback`] for an event that carries no value, such as a click.
pub fn callback(handler: impl Fn(&mut Window, &mut App) + 'static) -> Callback<()> {
    Rc::new(move |(), window, cx| handler(window, cx))
}

/// Every component's mocks.
#[cfg(feature = "mocks")]
pub fn mocks() -> Vec<mock::Mock> {
    [
        theme_gallery::mocks(),
        command_palette::mocks(),
        icon::mocks(),
        kbd::mocks(),
        tag::mocks(),
        toggle::mocks(),
        select::mocks(),
        segmented_control::mocks(),
        tab_strip::mocks(),
        button::mocks(),
        icon_button::mocks(),
        breadcrumb::mocks(),
        status_ring::mocks(),
        diff_stat::mocks(),
        list_row::mocks(),
        tooltip::mocks(),
        timeline_row::mocks(),
        title_bar::mocks(),
        diff::mocks(),
        fade_text::mocks(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

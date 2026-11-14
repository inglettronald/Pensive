//! Stateless building blocks that render from props and the theme.

mod breadcrumb;
mod button;
mod card;
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
mod nav_list;
mod next_card;
mod notice_list;
mod rail;
mod scroll_to_change;
mod segmented_control;
mod select;
mod setting_row;
mod settings_heading;
mod status_ring;
mod stepper;
mod tab_strip;
mod tag;
#[cfg(feature = "mocks")]
mod theme_gallery;
mod timeline_row;
mod title_bar;
mod toast;
mod toggle;
mod tone;
mod tooltip;
mod transition;

use std::{rc::Rc, sync::Arc};

use gpui::{App, ElementId, SharedString, StatefulInteractiveElement, Window};

pub use breadcrumb::Breadcrumb;
pub use button::{Button, ButtonStyle};
pub use card::Card;
pub use command_palette::{
    CommandPalette, KeyHint, Leading, PaletteChip, PaletteEntry, PaletteSection, TrailingText,
};
pub use diff::{DiffRow, LineKind};
pub use diff_stat::DiffStat;
pub use fade_text::FadeText;
#[cfg(feature = "mocks")]
pub use gallery::Gallery;
pub use icon::{Icon, IconName};
pub use icon_button::IconButton;
pub use kbd::Kbd;
pub use list_row::ListRow;
pub use nav_list::{NavItem, NavList};
pub use next_card::NextCard;
pub use notice_list::{NoticeList, NoticeRow};
pub use rail::{Rail, RailButton, RailItem};
pub use scroll_to_change::ScrollToChange;
pub use segmented_control::SegmentedControl;
pub use select::Select;
pub use setting_row::SettingRow;
pub use settings_heading::{CategoryHeading, PageHeading};
pub use status_ring::{RingFill, StatusRing};
pub use stepper::Stepper;
pub use tab_strip::TabStrip;
pub use tag::Tag;
pub use timeline_row::{TimelineLine, TimelineNode, TimelineRow};
pub use title_bar::{TitleBar, TitleBarProject};
pub use toast::Toast;
pub use toggle::Toggle;
pub use tone::Tone;
pub use tooltip::Tooltip;

/// How a component reports a change: called with the new value. An `Rc`, so a component can
/// share one callback among its children.
pub type Callback<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// A [`Callback`] for an event that carries no value, such as a click.
pub fn callback(handler: impl Fn(&mut Window, &mut App) + 'static) -> Callback<()> {
    Rc::new(move |(), window, cx| handler(window, cx))
}

/// An id for a part of the element with `id`, such as its kept state or one of its animations.
pub(crate) fn child_id(id: &ElementId, part: impl Into<SharedString>) -> ElementId {
    ElementId::NamedChild(Arc::new(id.clone()), part.into())
}

/// Clicks on a component that takes an `on_click` callback.
trait OnClick: StatefulInteractiveElement {
    /// Calls `handler`, if there is one, on a click, and stops the click there, so a control
    /// inside a clickable row doesn't click the row too.
    fn on_click_callback(self, handler: Option<Callback<()>>) -> Self {
        match handler {
            Some(handler) => self.on_click(move |_, window, cx| {
                cx.stop_propagation();
                handler((), window, cx);
            }),
            None => self,
        }
    }
}

impl<E: StatefulInteractiveElement> OnClick for E {}

/// Every component's mocks.
#[cfg(feature = "mocks")]
pub fn mocks() -> Vec<mock::Mock> {
    [
        theme_gallery::mocks(),
        command_palette::mocks(),
        icon::mocks(),
        kbd::mocks(),
        tag::mocks(),
        toast::mocks(),
        notice_list::mocks(),
        toggle::mocks(),
        select::mocks(),
        segmented_control::mocks(),
        stepper::mocks(),
        scroll_to_change::mocks(),
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
        rail::mocks(),
        nav_list::mocks(),
        card::mocks(),
        setting_row::mocks(),
        settings_heading::mocks(),
        next_card::mocks(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "tests click and scroll at window pixels"
)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{
        AnyElement, Context, InteractiveElement, IntoElement, Modifiers, ParentElement, Render,
        Styled, TestAppContext, div, point, px,
    };

    use super::*;

    type Control = fn(Rc<Cell<usize>>) -> AnyElement;

    /// A clickable row with one control in its top-left corner, like a file row with its ring.
    struct Row {
        control: Control,
        control_clicks: Rc<Cell<usize>>,
        row_clicks: Rc<Cell<usize>>,
    }

    impl Render for Row {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let row_clicks = self.row_clicks.clone();
            div()
                .id("row")
                .size_full()
                .on_click(move |_, _, _| row_clicks.set(row_clicks.get() + 1))
                .child((self.control)(self.control_clicks.clone()))
        }
    }

    fn count(clicks: Rc<Cell<usize>>) -> impl Fn(&mut Window, &mut App) + 'static {
        move |_, _| clicks.set(clicks.get() + 1)
    }

    #[gpui::test]
    fn a_control_takes_its_click_from_the_row_around_it(cx: &mut TestAppContext) {
        cx.update(theme::init);
        let controls: [(&str, Control); 6] = [
            ("Button", |clicks| {
                Button::new("control", "Stage")
                    .on_click(count(clicks))
                    .into_any_element()
            }),
            ("IconButton", |clicks| {
                IconButton::new("control", IconName::Plus)
                    .on_click(count(clicks))
                    .into_any_element()
            }),
            ("StatusRing", |clicks| {
                StatusRing::new("control", RingFill::Empty)
                    .on_click(count(clicks))
                    .into_any_element()
            }),
            ("RailButton", |clicks| {
                RailButton::new("control", IconName::Bell, "Notices")
                    .on_click(count(clicks))
                    .into_any_element()
            }),
            ("Stepper", |clicks| {
                let count = count(clicks);
                Stepper::new("control", "7 days")
                    .on_step(move |_, window, cx| count(window, cx))
                    .into_any_element()
            }),
            ("ListRow", |clicks| {
                ListRow::new("control", "src/lib.rs")
                    .on_click(count(clicks))
                    .into_any_element()
            }),
        ];
        for (name, control) in controls {
            let control_clicks = Rc::new(Cell::new(0));
            let row_clicks = Rc::new(Cell::new(0));
            let (_, cx) = cx.add_window_view(|_, _| Row {
                control,
                control_clicks: control_clicks.clone(),
                row_clicks: row_clicks.clone(),
            });
            cx.simulate_click(point(px(4.), px(4.)), Modifiers::none());
            assert_eq!(control_clicks.get(), 1, "{name} wasn't clicked");
            assert_eq!(row_clicks.get(), 0, "{name}'s click reached the row");
        }
    }
}

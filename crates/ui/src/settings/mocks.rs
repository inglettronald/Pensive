//! The settings view's mocks: a window on the sample project with Settings open.

use gpui::{AnyView, App, Context, UpdateGlobal, Window};
use mock::Mock;
use model::fixtures::PENSIVE;
use settings::Settings;
use theme::DensityPreset;

use super::{Pending, SettingsView, Spot};
use crate::mock_window;

/// The Command palette category, second on the rail.
const PALETTE: Spot = Spot {
    category: 1,
    page: 0,
};
/// Appearance's Size page.
const SIZE: Spot = Spot {
    category: 0,
    page: 1,
};

/// A window on pensive with Settings open, set up by `setup`.
fn window(
    setup: fn(&mut SettingsView, &mut Context<SettingsView>),
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    let view = mock_window::pensive(Some(PENSIVE), window, cx);
    view.update(cx, |pensive, cx| {
        pensive.open_settings(window, cx);
        if let Some(settings) = pensive.settings_view() {
            settings.update(cx, setup);
        }
    });
    view.into()
}

fn untouched(_: &mut SettingsView, _: &mut Context<SettingsView>) {}

impl SettingsView {
    /// Shows `spot` as soon as it's laid out, without scrolling there.
    fn jump_to(&mut self, spot: Spot, cx: &mut Context<Self>) {
        self.pending = Some(Pending::Spot(spot));
        cx.notify();
    }

    /// Types `query` while reading `category`.
    fn search_in(&mut self, category: usize, query: &str, cx: &mut Context<Self>) {
        self.current = Spot { category, page: 0 };
        self.search
            .update(cx, |input, cx| input.set_text(query.to_string(), cx));
    }
}

pub(crate) fn mocks() -> Vec<Mock> {
    vec![
        Mock::new("settings", "appearance", |w, cx| window(untouched, w, cx)),
        Mock::new("settings", "rail_hover", |w, cx| {
            window(
                |view, cx| {
                    view.hovered = Some(2);
                    cx.notify();
                },
                w,
                cx,
            )
        }),
        Mock::new("settings", "command_palette", |w, cx| {
            window(|view, cx| view.jump_to(PALETTE, cx), w, cx)
        }),
        Mock::new("settings", "search", |w, cx| {
            window(
                |view, cx| view.search_in(PALETTE.category, "show", cx),
                w,
                cx,
            )
        }),
        Mock::new("settings", "no_results", |w, cx| {
            window(
                |view, cx| view.search_in(PALETTE.category, "qwerty", cx),
                w,
                cx,
            )
        }),
        Mock::new("settings", "size_changed", |w, cx| {
            let view = window(|view, cx| view.jump_to(SIZE, cx), w, cx);
            Settings::update_global(cx, |settings, _| {
                settings.load("zoom = 110\ncode_size = 15\n")
            });
            view
        }),
        Mock::new("settings", "dense", |w, cx| {
            cx.set_global(DensityPreset::Dense.density());
            window(untouched, w, cx)
        }),
    ]
}

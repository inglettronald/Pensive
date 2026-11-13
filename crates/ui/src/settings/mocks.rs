//! The settings view's mocks: a window on the sample project with Settings open.

use gpui::{AnyView, App, AppContext, Context, Window};
use mock::Mock;
use model::fixtures::{self, PENSIVE};
use theme::DensityPreset;

use super::{Pending, SettingsView, Spot};
use crate::root::Pensive;

/// The Command palette category, second on the rail.
const PALETTE: usize = 1;

/// A window on pensive with Settings open, set up by `setup`.
fn window(
    setup: fn(&mut SettingsView, &mut Context<SettingsView>),
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    crate::palette::declare_settings(cx);
    crate::notices::declare_settings(cx);
    let projects = cx.new(|_| fixtures::projects());
    let view = cx.new(|cx| Pensive::new(projects, Some(PENSIVE), window, cx));
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
    /// Shows `category` as soon as it's laid out, without scrolling there.
    fn jump_to(&mut self, category: usize, cx: &mut Context<Self>) {
        self.pending = Some(Pending::Spot(Spot { category, page: 0 }));
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
            window(|view, cx| view.search_in(PALETTE, "show", cx), w, cx)
        }),
        Mock::new("settings", "no_results", |w, cx| {
            window(|view, cx| view.search_in(PALETTE, "qwerty", cx), w, cx)
        }),
        Mock::new("settings", "dense", |w, cx| {
            cx.set_global(DensityPreset::Dense.density());
            window(untouched, w, cx)
        }),
    ]
}

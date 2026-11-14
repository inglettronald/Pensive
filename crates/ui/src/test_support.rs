//! The set-up, windows and input that ui's tests share.

use gpui::{AppContext, Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext};
use model::fixtures;

use crate::root::Pensive;

/// The theme, keys and settings, set up as the app sets them up.
pub(crate) fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::init(cx);
        crate::init(cx);
    });
}

/// A window on the sample pensive project, laid out and idle.
pub(crate) fn pensive_window(cx: &mut TestAppContext) -> (Entity<Pensive>, &mut VisualTestContext) {
    init(cx);
    let projects = cx.new(|_| fixtures::projects());
    let (view, cx) = cx
        .add_window_view(|window, cx| Pensive::new(projects, Some(fixtures::PENSIVE), window, cx));
    cx.run_until_parked();
    (view, cx)
}

/// Moves the pointer to `position` and clicks there.
pub(crate) fn click(cx: &mut VisualTestContext, position: Point<Pixels>) {
    cx.simulate_mouse_move(position, None, Modifiers::none());
    cx.simulate_click(position, Modifiers::none());
}

/// `key` with the platform's main modifier, Cmd on macOS and Ctrl elsewhere, as the app's
/// `secondary-` bindings read it.
pub(crate) fn secondary(key: &str) -> String {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    format!("{modifier}-{key}")
}

/// `key` with the platform's word modifier, Alt on macOS and Ctrl elsewhere, as the text-editing
/// word keys read it.
pub(crate) fn word(key: &str) -> String {
    let modifier = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    format!("{modifier}-{key}")
}

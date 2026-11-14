//! The gear at the foot of every view's rail, which opens Settings, closes it, or opens it in a
//! new window.

use components::{IconName, RailButton};
use gpui::{AnyElement, Context, EventEmitter, div, prelude::*};

use crate::view_callback_with;

/// The gear was clicked. The window opens Settings, or closes it when it's open. With Ctrl (Cmd
/// on macOS) held, it opens Settings in a new window instead.
pub(crate) struct GearClicked {
    pub new_window: bool,
}

/// The gear for the rail of the view behind `cx`. `open` fills it, as Settings does to show
/// where you are. The view keeps whether it's hovered, and `set_hovered` stores it.
pub(crate) fn rail_gear<V: EventEmitter<GearClicked>>(
    open: bool,
    hovered: bool,
    set_hovered: fn(&mut V, bool),
    cx: &mut Context<V>,
) -> AnyElement {
    let view = cx.entity().downgrade();
    div()
        .id("gear")
        .debug_selector(|| "settings".into())
        .child(
            RailButton::new("gear-button", IconName::Settings, "Settings")
                .filled(open)
                .hovered(hovered)
                .on_hover(view_callback_with(cx, move |view: &mut V, on, _, cx| {
                    set_hovered(view, on);
                    cx.notify();
                })),
        )
        .on_click(move |event, _, cx| {
            let new_window = event.modifiers().secondary(cx);
            view.update(cx, |_, cx| cx.emit(GearClicked { new_window }))
                .ok();
        })
        .into_any_element()
}

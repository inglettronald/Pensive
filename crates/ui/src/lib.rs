//! Views: the screens and panels that read models and own their view state.

mod palette;
mod project;
mod root;
mod text_input;
mod window_frame;

use std::rc::Rc;

use gpui::{Action, App, Context, KeyBinding, KeyBindingContextPredicate, Window};

pub use palette::{Keyword, PaletteEvent, PaletteView, Tab};
pub use root::{Pensive, open_window};

/// Binds every view's keys and declares the views' settings.
pub fn init(cx: &mut App) {
    palette::declare_settings(cx);
    text_input::bind_keys(cx);
    palette::bind_keys(cx);
    project::bind_keys(cx);
    root::bind_keys(cx);
}

/// Binds `keys` to `action` in `context`. Unlike `KeyBinding::new`, it reads `secondary-` as Cmd
/// on macOS and Ctrl elsewhere.
fn key_binding(keys: &str, action: impl Action, context: Option<&str>, cx: &App) -> KeyBinding {
    let context = context.map(|context| {
        Rc::new(KeyBindingContextPredicate::parse(context).expect("the key context is valid"))
    });
    KeyBinding::load(keys, Box::new(action), context, false, None, cx)
        .expect("the keystrokes are valid")
}

/// Calls `handler` on the view behind `cx` when a component reports an event that carries no
/// value, such as a click.
fn view_callback<V: 'static>(
    cx: &Context<V>,
    handler: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let view = cx.entity().downgrade();
    move |window, cx| {
        view.update(cx, |view, cx| handler(view, window, cx)).ok();
    }
}

/// [`view_callback`] for events that carry a value, such as a selected index.
fn view_callback_with<V: 'static, T: 'static>(
    cx: &Context<V>,
    handler: impl Fn(&mut V, T, &mut Window, &mut Context<V>) + 'static,
) -> impl Fn(T, &mut Window, &mut App) + 'static {
    let view = cx.entity().downgrade();
    move |value, window, cx| {
        view.update(cx, |view, cx| handler(view, value, window, cx))
            .ok();
    }
}

/// Every view's mocks.
#[cfg(feature = "mocks")]
pub fn mocks() -> Vec<mock::Mock> {
    [root::mocks(), project::mocks()]
        .into_iter()
        .flatten()
        .collect()
}

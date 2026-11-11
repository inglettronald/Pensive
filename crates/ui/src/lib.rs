//! Views: the screens and panels that read models and own their view state.

mod palette;
mod root;
mod text_input;

use std::rc::Rc;

use gpui::{Action, App, KeyBinding, KeyBindingContextPredicate};

pub use palette::{Keyword, PaletteEvent, PaletteView, Tab};
pub use root::{Pensive, open_window};

/// Binds every view's keys.
pub fn init(cx: &mut App) {
    text_input::bind_keys(cx);
    palette::bind_keys(cx);
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

/// Every view's mocks.
#[cfg(feature = "mocks")]
pub fn mocks() -> Vec<mock::Mock> {
    root::mocks()
}

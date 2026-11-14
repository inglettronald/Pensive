//! The actions every view that edits text shares, and the keys bound to them.

use gpui::{App, KeyContext, actions};

use crate::key_binding;

actions!(
    text_editing,
    [
        Left,
        Right,
        SelectLeft,
        SelectRight,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Backspace,
        Delete,
        DeleteWordLeft,
        SelectAll,
        Copy,
        Cut,
        Paste,
        ShowCharacterPalette,
    ]
);

const CONTEXT: &str = "TextEditing";

/// The key context for the view named `view`, which also takes the shared text-editing keys.
pub(crate) fn key_context(view: &'static str) -> KeyContext {
    let mut context = KeyContext::default();
    context.add(view);
    context.add(CONTEXT);
    context
}

pub(crate) fn bind_keys(cx: &mut App) {
    let word = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    let context = Some(CONTEXT);
    cx.bind_keys([
        key_binding("left", Left, context, cx),
        key_binding("right", Right, context, cx),
        key_binding("shift-left", SelectLeft, context, cx),
        key_binding("shift-right", SelectRight, context, cx),
        key_binding(&format!("{word}-left"), WordLeft, context, cx),
        key_binding(&format!("{word}-right"), WordRight, context, cx),
        key_binding(&format!("{word}-shift-left"), SelectWordLeft, context, cx),
        key_binding(&format!("{word}-shift-right"), SelectWordRight, context, cx),
        key_binding(&format!("{word}-backspace"), DeleteWordLeft, context, cx),
        key_binding("home", Home, context, cx),
        key_binding("end", End, context, cx),
        key_binding("shift-home", SelectHome, context, cx),
        key_binding("shift-end", SelectEnd, context, cx),
        key_binding("backspace", Backspace, context, cx),
        key_binding("delete", Delete, context, cx),
        key_binding("secondary-a", SelectAll, context, cx),
        key_binding("secondary-c", Copy, context, cx),
        key_binding("secondary-x", Cut, context, cx),
        key_binding("secondary-v", Paste, context, cx),
        key_binding("ctrl-cmd-space", ShowCharacterPalette, context, cx),
    ]);
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            key_binding("cmd-left", Home, context, cx),
            key_binding("cmd-right", End, context, cx),
        ]);
    }
}

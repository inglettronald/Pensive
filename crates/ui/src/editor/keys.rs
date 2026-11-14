//! The editor's actions, and the keys bound to them.

use gpui::{App, actions};

use crate::key_binding;

actions!(
    editor,
    [
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Top,
        Bottom,
        PageUp,
        PageDown,
        Backspace,
        Delete,
        DeleteWordLeft,
        Newline,
        Indent,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Save,
        ShowCharacterPalette,
    ]
);

pub(super) const CONTEXT: &str = "Editor";

pub(crate) fn bind_keys(cx: &mut App) {
    let word = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    let bindings = [
        key_binding("left", Left, Some(CONTEXT), cx),
        key_binding("right", Right, Some(CONTEXT), cx),
        key_binding("up", Up, Some(CONTEXT), cx),
        key_binding("down", Down, Some(CONTEXT), cx),
        key_binding("shift-left", SelectLeft, Some(CONTEXT), cx),
        key_binding("shift-right", SelectRight, Some(CONTEXT), cx),
        key_binding("shift-up", SelectUp, Some(CONTEXT), cx),
        key_binding("shift-down", SelectDown, Some(CONTEXT), cx),
        key_binding(&format!("{word}-left"), WordLeft, Some(CONTEXT), cx),
        key_binding(&format!("{word}-right"), WordRight, Some(CONTEXT), cx),
        key_binding(
            &format!("{word}-shift-left"),
            SelectWordLeft,
            Some(CONTEXT),
            cx,
        ),
        key_binding(
            &format!("{word}-shift-right"),
            SelectWordRight,
            Some(CONTEXT),
            cx,
        ),
        key_binding(
            &format!("{word}-backspace"),
            DeleteWordLeft,
            Some(CONTEXT),
            cx,
        ),
        key_binding("home", Home, Some(CONTEXT), cx),
        key_binding("end", End, Some(CONTEXT), cx),
        key_binding("shift-home", SelectHome, Some(CONTEXT), cx),
        key_binding("shift-end", SelectEnd, Some(CONTEXT), cx),
        key_binding("secondary-home", Top, Some(CONTEXT), cx),
        key_binding("secondary-end", Bottom, Some(CONTEXT), cx),
        key_binding("pageup", PageUp, Some(CONTEXT), cx),
        key_binding("pagedown", PageDown, Some(CONTEXT), cx),
        key_binding("backspace", Backspace, Some(CONTEXT), cx),
        key_binding("delete", Delete, Some(CONTEXT), cx),
        key_binding("enter", Newline, Some(CONTEXT), cx),
        key_binding("tab", Indent, Some(CONTEXT), cx),
        key_binding("secondary-a", SelectAll, Some(CONTEXT), cx),
        key_binding("secondary-c", Copy, Some(CONTEXT), cx),
        key_binding("secondary-x", Cut, Some(CONTEXT), cx),
        key_binding("secondary-v", Paste, Some(CONTEXT), cx),
        key_binding("secondary-z", Undo, Some(CONTEXT), cx),
        key_binding("secondary-shift-z", Redo, Some(CONTEXT), cx),
        key_binding("secondary-s", Save, Some(CONTEXT), cx),
        key_binding("ctrl-cmd-space", ShowCharacterPalette, Some(CONTEXT), cx),
    ];
    cx.bind_keys(bindings);
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            key_binding("cmd-left", Home, Some(CONTEXT), cx),
            key_binding("cmd-right", End, Some(CONTEXT), cx),
            key_binding("cmd-up", Top, Some(CONTEXT), cx),
            key_binding("cmd-down", Bottom, Some(CONTEXT), cx),
        ]);
    } else {
        cx.bind_keys([key_binding("ctrl-y", Redo, Some(CONTEXT), cx)]);
    }
}

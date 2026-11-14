//! The editor's own actions, and the keys bound to them. Its cursor, selection and clipboard keys
//! are the shared ones in `text_editing`.

use gpui::{App, actions};

use crate::key_binding;

actions!(
    editor,
    [
        Up, Down, SelectUp, SelectDown, Top, Bottom, PageUp, PageDown, Newline, Indent, Undo, Redo,
        Save,
    ]
);

pub(super) const CONTEXT: &str = "Editor";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        key_binding("up", Up, Some(CONTEXT), cx),
        key_binding("down", Down, Some(CONTEXT), cx),
        key_binding("shift-up", SelectUp, Some(CONTEXT), cx),
        key_binding("shift-down", SelectDown, Some(CONTEXT), cx),
        key_binding("secondary-home", Top, Some(CONTEXT), cx),
        key_binding("secondary-end", Bottom, Some(CONTEXT), cx),
        key_binding("pageup", PageUp, Some(CONTEXT), cx),
        key_binding("pagedown", PageDown, Some(CONTEXT), cx),
        key_binding("enter", Newline, Some(CONTEXT), cx),
        key_binding("tab", Indent, Some(CONTEXT), cx),
        key_binding("secondary-z", Undo, Some(CONTEXT), cx),
        key_binding("secondary-shift-z", Redo, Some(CONTEXT), cx),
        key_binding("secondary-s", Save, Some(CONTEXT), cx),
    ]);
    if cfg!(target_os = "macos") {
        cx.bind_keys([
            key_binding("cmd-up", Top, Some(CONTEXT), cx),
            key_binding("cmd-down", Bottom, Some(CONTEXT), cx),
        ]);
    } else {
        cx.bind_keys([key_binding("ctrl-y", Redo, Some(CONTEXT), cx)]);
    }
}

//! Moving the cursor, selecting, deleting and the clipboard, run the same way in every view that
//! edits text.

mod keys;
pub(crate) mod movement;
mod selection;
pub(crate) mod utf16;

use std::ops::Range;

use gpui::{App, ClipboardItem, Context, InteractiveElement, Window};
use model::Rope;

pub(crate) use keys::*;
pub(crate) use selection::Selection;

/// A view that the shared text-editing actions work on.
pub(crate) trait TextEditing: Sized + 'static {
    /// Whether a pasted line break becomes a space.
    const SINGLE_LINE: bool;

    /// The text being edited, if there is any.
    fn rope(&self, cx: &App) -> Option<Rope>;

    fn selection(&self) -> &Selection;

    /// Puts the cursor at `offset`, or with `select` extends the selection there.
    fn move_cursor(
        &mut self,
        offset: usize,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    );

    fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>);

    /// Replaces the bytes in `range`, or what [`Selection::replaced`] picks, with `text`, and
    /// puts the cursor after it.
    fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
}

/// Where a motion takes the cursor, given the text and the cursor.
type Target = fn(&Rope, usize) -> usize;

/// Gives `element` the key context `view` and the shared keys, and runs every shared action on it
/// against the view behind `cx`.
pub(crate) fn attach<V: TextEditing, E: InteractiveElement>(
    element: E,
    view: &'static str,
    cx: &Context<V>,
) -> E {
    use movement::*;
    element
        .key_context(key_context(view))
        .on_action(cx.listener(|view, _: &Left, window, cx| step(view, false, window, cx)))
        .on_action(cx.listener(|view, _: &Right, window, cx| step(view, true, window, cx)))
        .on_action(
            cx.listener(|view, _: &SelectLeft, window, cx| {
                motion(view, true, previous, window, cx)
            }),
        )
        .on_action(
            cx.listener(|view, _: &SelectRight, window, cx| motion(view, true, next, window, cx)),
        )
        .on_action(cx.listener(|view, _: &WordLeft, window, cx| {
            motion(view, false, previous_word, window, cx)
        }))
        .on_action(
            cx.listener(|view, _: &WordRight, window, cx| {
                motion(view, false, next_word, window, cx)
            }),
        )
        .on_action(cx.listener(|view, _: &SelectWordLeft, window, cx| {
            motion(view, true, previous_word, window, cx)
        }))
        .on_action(cx.listener(|view, _: &SelectWordRight, window, cx| {
            motion(view, true, next_word, window, cx)
        }))
        .on_action(cx.listener(|view, _: &Home, window, cx| motion(view, false, home, window, cx)))
        .on_action(cx.listener(|view, _: &End, window, cx| motion(view, false, end, window, cx)))
        .on_action(
            cx.listener(|view, _: &SelectHome, window, cx| motion(view, true, home, window, cx)),
        )
        .on_action(
            cx.listener(|view, _: &SelectEnd, window, cx| motion(view, true, end, window, cx)),
        )
        .on_action(
            cx.listener(|view, _: &Backspace, window, cx| delete_to(view, previous, window, cx)),
        )
        .on_action(cx.listener(|view, _: &Delete, window, cx| delete_to(view, next, window, cx)))
        .on_action(cx.listener(|view, _: &DeleteWordLeft, window, cx| {
            delete_to(view, previous_word, window, cx)
        }))
        .on_action(
            cx.listener(|view: &mut V, _: &SelectAll, window, cx| view.select_all(window, cx)),
        )
        .on_action(cx.listener(copy))
        .on_action(cx.listener(cut))
        .on_action(cx.listener(paste))
        .on_action(
            cx.listener(|_: &mut V, _: &ShowCharacterPalette, window, _| {
                window.show_character_palette()
            }),
        )
}

/// Moves the cursor to `target`, or with `select` extends the selection there.
fn motion<V: TextEditing>(
    view: &mut V,
    select: bool,
    target: Target,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    let Some(text) = view.rope(cx) else {
        return;
    };
    let offset = target(&text, view.selection().cursor());
    view.move_cursor(offset, select, window, cx);
}

/// Left and Right: one character over, or to that end of the selection when there is one.
fn step<V: TextEditing>(view: &mut V, forward: bool, window: &mut Window, cx: &mut Context<V>) {
    let range = view.selection().range.clone();
    if range.is_empty() {
        let target = if forward {
            movement::next
        } else {
            movement::previous
        };
        motion(view, false, target, window, cx);
    } else {
        let edge = if forward { range.end } else { range.start };
        view.move_cursor(edge, false, window, cx);
    }
}

/// Deletes the selection, or from the cursor to `target` when nothing is selected.
fn delete_to<V: TextEditing>(
    view: &mut V,
    target: Target,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    let Some(text) = view.rope(cx) else {
        return;
    };
    let selection = view.selection();
    let range = if selection.range.is_empty() {
        let (cursor, target) = (selection.cursor(), target(&text, selection.cursor()));
        cursor.min(target)..cursor.max(target)
    } else {
        selection.range.clone()
    };
    if !range.is_empty() {
        view.replace(Some(range), "", window, cx);
    }
}

fn selected_text<V: TextEditing>(view: &V, cx: &App) -> Option<String> {
    let range = view.selection().range.clone();
    let text = view.rope(cx)?;
    (!range.is_empty()).then(|| text.byte_slice(range).to_string())
}

fn copy<V: TextEditing>(view: &mut V, _: &Copy, _: &mut Window, cx: &mut Context<V>) {
    if let Some(text) = selected_text(view, cx) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

fn cut<V: TextEditing>(view: &mut V, _: &Cut, window: &mut Window, cx: &mut Context<V>) {
    if let Some(text) = selected_text(view, cx) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        view.replace(None, "", window, cx);
    }
}

fn paste<V: TextEditing>(view: &mut V, _: &Paste, window: &mut Window, cx: &mut Context<V>) {
    let Some(mut text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
        return;
    };
    if V::SINGLE_LINE {
        text = text.replace('\n', " ");
    }
    view.replace(None, &text, window, cx);
}

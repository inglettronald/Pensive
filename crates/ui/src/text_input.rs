//! A single-line text field. Adapted from GPUI's `examples/input.rs` (Apache-2.0, in this crate's
//! `LICENSE-APACHE`) at the Zed tag pinned in the root `Cargo.toml`: it handles IME composition
//! and the mouse, edits with the actions in `text_editing`, and is styled from the theme.

use std::ops::Range;

use gpui::{
    App, Bounds, Context, CursorStyle, ElementId, ElementInputHandler, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, GlobalElementId, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine, SharedString, Style,
    TextRun, UTF16Selection, UnderlineStyle, Window, div, fill, point, prelude::*, relative, size,
};
use model::Rope;
use theme::{ActiveTheme, CARET, HAIRLINE};

use crate::text_editing::{self, Selection, TextEditing, utf16};

const CONTEXT: &str = "TextInput";

/// Sent whenever the text changes.
pub(crate) struct Changed;

pub(crate) struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selection: Selection,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl EventEmitter<Changed> for TextInput {}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: SharedString::default(),
            selection: Selection::default(),
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub fn text(&self) -> &SharedString {
        &self.content
    }

    pub fn cursor_at_end(&self) -> bool {
        self.selection.range == (self.content.len()..self.content.len())
    }

    /// Replaces the text and puts the cursor at its end.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selection = Selection::at(self.content.len());
        cx.emit(Changed);
        cx.notify();
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position);
        self.move_cursor(offset, event.modifiers.shift, window, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            let offset = self.index_for_mouse_position(event.position);
            self.move_cursor(offset, true, window, cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    /// The text as a rope, which the shared movement and UTF-16 functions take.
    fn text_rope(&self) -> Rope {
        Rope::from_str(&self.content)
    }

    fn splice(&mut self, range: Range<usize>, new_text: &str) {
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
    }

    /// `range`, counted in UTF-16, as bytes.
    fn bytes(&self, range: &Range<usize>) -> Range<usize> {
        utf16::range_from_utf16(&self.text_rope(), range)
    }

    fn selection_changed(&self, window: &Window, cx: &mut Context<Self>) {
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    /// Tells the platform's input method and the view's listeners that the text changed.
    fn text_changed(&self, window: &Window, cx: &mut Context<Self>) {
        window.notify_text_input_changed(&self.focus_handle);
        cx.emit(Changed);
        cx.notify();
    }
}

impl TextEditing for TextInput {
    const SINGLE_LINE: bool = true;

    fn rope(&self, _: &App) -> Option<Rope> {
        Some(self.text_rope())
    }

    fn selection(&self) -> &Selection {
        &self.selection
    }

    fn move_cursor(
        &mut self,
        offset: usize,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection.move_cursor(offset, select);
        self.selection_changed(window, cx);
    }

    fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.select(0..self.content.len());
        self.selection_changed(window, cx);
    }

    fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.selection.replaced(range);
        self.splice(range.clone(), text);
        self.selection = Selection::at(range.start + text.len());
        self.text_changed(window, cx);
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.text_rope();
        let range = utf16::range_from_utf16(&text, &range_utf16);
        actual_range.replace(utf16::range_to_utf16(&text, &range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(self.selection.to_utf16(&self.text_rope()))
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.bytes(&range_utf16);
        self.selection.select(range);
        self.selection_changed(window, cx);
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let marked = self.selection.marked.as_ref()?;
        Some(utf16::range_to_utf16(&self.text_rope(), marked))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.marked = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16.map(|range| self.bytes(&range));
        self.replace(range, new_text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16.map(|range| self.bytes(&range));
        let range = self.selection.replaced(range);
        self.splice(range.clone(), new_text);
        let text = self.text_rope();
        self.selection
            .mark(&text, range.start, new_text, new_selected_range_utf16);
        self.text_changed(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        if last_layout.text != self.content {
            return None;
        }
        let range = self.bytes(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn selection_bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(range_utf16, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        if last_layout.text != self.content {
            return None;
        }
        let offset = last_layout.closest_index_for_x(line_point.x);
        Some(utf16::to_utf16(&self.text_rope(), offset))
    }
}

/// Lays out and paints the text, the selection and the cursor.
struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let colors = &cx.theme().colors;
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selection.range.clone();
        let cursor = input.selection.cursor();
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), colors.text_faint)
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match input.selection.marked.as_ref() {
            Some(marked_range) => [
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: HAIRLINE,
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            None => vec![run],
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let (selection, cursor) = if selected_range.is_empty() {
            let x = line.x_for_index(cursor);
            let cursor = fill(
                Bounds::new(
                    point(bounds.left() + x, bounds.top()),
                    size(CARET, bounds.bottom() - bounds.top()),
                ),
                colors.text_strong,
            );
            (None, Some(cursor))
        } else {
            let selection = fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(selected_range.start),
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(selected_range.end),
                        bounds.bottom(),
                    ),
                ),
                colors.accent_solid.opacity(0.4),
            );
            (Some(selection), None)
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let Some(line) = prepaint.line.take() else {
            return;
        };
        line.paint(
            bounds.origin,
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .ok();

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input = div()
            .flex()
            .w_full()
            .key_context(text_editing::key_context(CONTEXT))
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam);
        text_editing::on_actions(input, cx)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use gpui::{TestAppContext, VisualTestContext};

    use gpui::ClipboardItem;

    use super::*;
    use crate::test_support::secondary;

    fn input_window(cx: &mut TestAppContext) -> (Entity<TextInput>, &mut VisualTestContext) {
        crate::test_support::init(cx);
        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new(cx));
        input.update_in(cx, |input, window, cx| {
            window.focus(&input.focus_handle(cx), cx)
        });
        (input, cx)
    }

    fn text(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> String {
        input.read_with(cx, |input, _| input.text().to_string())
    }

    #[gpui::test]
    fn select_all_cut_and_paste_go_through_the_clipboard(cx: &mut TestAppContext) {
        let (input, cx) = input_window(cx);
        cx.simulate_input("pensive");
        cx.simulate_keystrokes(&format!("{} {}", secondary("a"), secondary("x")));
        assert_eq!(text(&input, cx), "");
        let copied = cx.read_from_clipboard().and_then(|item| item.text());
        assert_eq!(copied.as_deref(), Some("pensive"));

        cx.simulate_keystrokes(&format!("{} {}", secondary("v"), secondary("v")));
        assert_eq!(text(&input, cx), "pensivepensive");
    }

    #[gpui::test]
    fn arrows_backspace_and_delete_edit_around_the_cursor(cx: &mut TestAppContext) {
        let (input, cx) = input_window(cx);
        cx.simulate_input("abcd");
        cx.simulate_keystrokes("left left backspace");
        assert_eq!(text(&input, cx), "acd");
        cx.simulate_keystrokes("delete");
        assert_eq!(text(&input, cx), "ad");
        cx.simulate_keystrokes("home");
        cx.simulate_input("x");
        cx.simulate_keystrokes("end");
        cx.simulate_input("y");
        assert_eq!(text(&input, cx), "xady");
    }

    #[gpui::test]
    fn the_word_keys_move_and_delete_by_word(cx: &mut TestAppContext) {
        let word = if cfg!(target_os = "macos") {
            "alt"
        } else {
            "ctrl"
        };
        let (input, cx) = input_window(cx);
        cx.simulate_input("open the file");
        cx.simulate_keystrokes(&format!("{word}-left {word}-left"));
        cx.simulate_input("x");
        assert_eq!(text(&input, cx), "open xthe file");
        cx.simulate_keystrokes(&format!("{word}-backspace"));
        assert_eq!(text(&input, cx), "open the file");
    }

    #[gpui::test]
    fn a_pasted_line_break_becomes_a_space(cx: &mut TestAppContext) {
        let (input, cx) = input_window(cx);
        cx.write_to_clipboard(ClipboardItem::new_string("one\ntwo".into()));
        cx.simulate_keystrokes(&secondary("v"));
        assert_eq!(text(&input, cx), "one two");
    }
}

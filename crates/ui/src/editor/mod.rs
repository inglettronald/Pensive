//! A code editor over a document: it shows the document's diff in one of three modes, edits it
//! from the keyboard, mouse and input method, and saves as the save setting says.

mod input;
mod keys;
mod line;
mod render;
mod rows;
mod saving;

use std::ops::Range;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, ListAlignment, ListState, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollEvent, Subscription, Window, px,
};
use model::{Document, Rope};
use theme::ActiveTheme;

pub(crate) use keys::bind_keys;
use keys::*;
use line::Layouts;
pub use rows::Mode;
use rows::{Row, rows};
use saving::AUTO_SAVE_DELAY;
pub(crate) use saving::{CATEGORY, SAVE, SaveMode, declare_settings};

use crate::text_editing::{Selection, TextEditing, movement};

/// What Tab inserts, unless the line is indented with tabs.
const INDENT: &str = "    ";

/// How far past the viewport the list lays out rows, so scrolling doesn't show gaps.
#[expect(
    clippy::disallowed_methods,
    reason = "measured against the viewport in pixels"
)]
const OVERDRAW: Pixels = px(200.);

pub struct Editor {
    document: Option<Entity<Document>>,
    mode: Mode,
    /// Unchanged lines opened from folds in Changes mode, by line.
    revealed: Vec<Range<usize>>,
    rows: Vec<Row>,
    list: ListState,
    selection: Selection,
    /// The column Up and Down aim for, kept across short lines.
    goal_column: Option<usize>,
    selecting: bool,
    /// How far the text is scrolled sideways.
    scroll_x: Pixels,
    /// Whether the next frame scrolls to show the cursor.
    autoscroll: bool,
    layouts: Layouts,
    focus_handle: FocusHandle,
    _document: Option<Subscription>,
    _blur: [Subscription; 2],
}

impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let blur = cx.on_blur(&focus_handle, window, |this, _, cx| this.auto_save_now(cx));
        let deactivate = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.auto_save_now(cx);
            }
        });
        Self {
            document: None,
            mode: Mode::default(),
            revealed: Vec::new(),
            rows: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, OVERDRAW),
            selection: Selection::default(),
            goal_column: None,
            selecting: false,
            scroll_x: Pixels::ZERO,
            autoscroll: false,
            layouts: Layouts::default(),
            focus_handle,
            _document: None,
            _blur: [blur, deactivate],
        }
    }

    pub fn document(&self) -> Option<&Entity<Document>> {
        self.document.as_ref()
    }

    /// Shows `document` with the cursor on its first change. With auto-save, the document it
    /// replaces saves first.
    pub fn set_document(&mut self, document: Option<Entity<Document>>, cx: &mut Context<Self>) {
        if self.document == document {
            return;
        }
        self.auto_save_now(cx);
        self._document = document.as_ref().map(|document| {
            cx.observe(document, |this, _, cx| {
                this.clamp_selection(cx);
                this.update_rows(cx);
            })
        });
        let first_change = document.as_ref().map_or(0, |document| {
            let document = document.read(cx);
            let text = document.text();
            let line = document
                .diff()
                .changes
                .first()
                .map_or(0, |change| change.new.start);
            text.line_to_byte(line.min(text.len_lines() - 1))
        });
        self.document = document;
        self.selection = Selection::at(first_change);
        self.goal_column = None;
        self.revealed.clear();
        self.scroll_x = Pixels::ZERO;
        self.rows.clear();
        self.list.reset(0);
        self.reveal_cursor(cx);
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.update_rows(cx);
        self.reveal_cursor(cx);
    }

    /// Types `text` at `offset`, as clicking there and typing would.
    #[cfg(feature = "mocks")]
    pub fn type_at(&mut self, offset: usize, text: &str, cx: &mut Context<Self>) {
        self.insert(Some(offset..offset), text, cx);
    }

    fn text<'a>(&self, cx: &'a App) -> Option<&'a Rope> {
        Some(self.document.as_ref()?.read(cx).text())
    }

    fn editable(&self, cx: &App) -> bool {
        self.document
            .as_ref()
            .is_some_and(|document| document.read(cx).is_editable())
    }

    /// Lines in the text. A deleted file has none, though its rope holds one empty line.
    fn line_count(&self, cx: &App) -> usize {
        match &self.document {
            Some(document) => {
                let document = document.read(cx);
                let text = document.text();
                if !document.is_editable() && text.len_bytes() == 0 {
                    0
                } else {
                    text.len_lines()
                }
            }
            None => 0,
        }
    }

    /// Keeps the selection inside the text and between characters, for when the text changed
    /// without the editor, as reloading does.
    fn clamp_selection(&mut self, cx: &App) {
        if let Some(text) = self.text(cx) {
            self.selection.clamp(text);
        }
    }

    /// Rebuilds the rows from the document and tells the list which ones changed height.
    fn update_rows(&mut self, cx: &mut Context<Self>) {
        let rows = match &self.document {
            Some(document) => {
                let document = document.read(cx);
                let cursor = document.text().byte_to_line(self.selection.cursor());
                rows(
                    document.diff(),
                    self.line_count(cx),
                    self.mode,
                    &self.revealed,
                    Some(cursor),
                )
            }
            None => Vec::new(),
        };
        let same = |old: &Row, new: &Row| old.is_fold() == new.is_fold();
        let prefix = self
            .rows
            .iter()
            .zip(&rows)
            .take_while(|(old, new)| same(old, new))
            .count();
        let longest = self.rows.len().min(rows.len()) - prefix;
        let suffix = self
            .rows
            .iter()
            .rev()
            .zip(rows.iter().rev())
            .take(longest)
            .take_while(|(old, new)| same(old, new))
            .count();
        if prefix + suffix < self.rows.len().max(rows.len()) {
            self.list.splice(
                prefix..self.rows.len() - suffix,
                rows.len() - prefix - suffix,
            );
        }
        self.rows = rows;
        cx.notify();
    }

    fn cursor_moved(&mut self, cx: &mut Context<Self>) {
        if let Some(document) = &self.document {
            document.update(cx, |document, _| document.break_undo_group());
        }
        self.reveal_cursor(cx);
    }

    /// Shows the cursor's line, which a fold never hides, then scrolls to it.
    fn reveal_cursor(&mut self, cx: &mut Context<Self>) {
        let Some(line) = self
            .text(cx)
            .map(|text| text.byte_to_line(self.selection.cursor()))
        else {
            return;
        };
        self.update_rows(cx);
        if let Some(row) = self.row_of(line) {
            self.list.scroll_to_reveal_item(row);
        }
        self.autoscroll = true;
        cx.notify();
    }

    fn row_of(&self, line: usize) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| row.text_line() == Some(line))
    }

    pub(super) fn set_scroll_x(&mut self, scroll_x: Pixels, cx: &mut Context<Self>) {
        self.autoscroll = false;
        if self.scroll_x != scroll_x {
            self.scroll_x = scroll_x;
            cx.notify();
        }
    }

    /// The offset `rows` text rows away from the cursor, aiming for the goal column.
    fn vertical(&mut self, rows: isize, cx: &App) -> Option<usize> {
        let text = self.text(cx)?;
        let cursor = self.selection.cursor();
        let line = text.byte_to_line(cursor);
        let column = *self
            .goal_column
            .get_or_insert_with(|| movement::column(text, cursor));
        let lines: Vec<usize> = self.rows.iter().filter_map(Row::text_line).collect();
        let here = lines.iter().position(|&shown| shown == line)?;
        let target = here as isize + rows;
        if target < 0 {
            return Some(0);
        }
        match lines.get(target as usize) {
            Some(&line) => Some(movement::at_column(text, line, column)),
            None => Some(text.len_bytes()),
        }
    }

    fn move_vertically(
        &mut self,
        rows: isize,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let goal = self.goal_column;
        let Some(offset) = self.vertical(rows, cx) else {
            return;
        };
        let goal_column = self.goal_column.or(goal);
        self.move_cursor(offset, select, window, cx);
        self.goal_column = goal_column;
    }

    /// Text rows that fit in the viewport, for Page Up and Page Down.
    fn page(&self, window: &Window, cx: &App) -> isize {
        let height = self.list.viewport_bounds().size.height;
        let line_height = cx.density().code.line_height.to_pixels(window.rem_size());
        ((height / line_height) as isize - 1).max(1)
    }

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, false, window, cx);
    }

    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, false, window, cx);
    }

    fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, true, window, cx);
    }

    fn select_down(&mut self, _: &SelectDown, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, true, window, cx);
    }

    fn top(&mut self, _: &Top, window: &mut Window, cx: &mut Context<Self>) {
        self.move_cursor(0, false, window, cx);
    }

    fn bottom(&mut self, _: &Bottom, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(len) = self.text(cx).map(Rope::len_bytes) {
            self.move_cursor(len, false, window, cx);
        }
    }

    fn page_up(&mut self, _: &PageUp, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-self.page(window, cx), false, window, cx);
    }

    fn page_down(&mut self, _: &PageDown, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(self.page(window, cx), false, window, cx);
    }

    /// Replaces the selection, or the bytes in `range`, with `text`, and puts the cursor after
    /// it. Does nothing in a read-only document.
    fn insert(&mut self, range: Option<Range<usize>>, text: &str, cx: &mut Context<Self>) {
        let Some(document) = self.document.clone() else {
            return;
        };
        if !self.editable(cx) {
            return;
        }
        let range = self.selection.replaced(range);
        let lines = document.read(cx).text();
        let first = lines.byte_to_line(range.start);
        let removed = lines.byte_to_line(range.end) - first;
        self.shift_revealed(first, removed, text.matches('\n').count());
        document.update(cx, |document, cx| {
            document.edit(range.clone(), text, cx);
            if SAVE.get(cx) == SaveMode::Auto {
                document.save_after(AUTO_SAVE_DELAY, cx);
            }
        });
        self.selection = Selection::at(range.start + text.len());
        self.goal_column = None;
        self.reveal_cursor(cx);
    }

    /// Moves the revealed lines below line `first` along, after an edit there that replaced
    /// `removed` line breaks with `added`, so they stay on the same text.
    fn shift_revealed(&mut self, first: usize, removed: usize, added: usize) {
        let shift = |line: usize| {
            if line <= first {
                line
            } else {
                (line + added).saturating_sub(removed).max(first + 1)
            }
        };
        for range in &mut self.revealed {
            *range = shift(range.start)..shift(range.end);
        }
        self.revealed.retain(|range| !range.is_empty());
    }

    /// Starts a new line with the current line's indentation and line ending.
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.text(cx) else {
            return;
        };
        let index = text.byte_to_line(self.selection.range.start);
        let (_, line) = movement::line(text, index);
        let indent = &line[..line.len() - line.trim_start().len()];
        let ending = if text.line(index).to_string().ends_with("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        self.insert(None, &format!("{ending}{indent}"), cx);
    }

    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.text(cx) else {
            return;
        };
        let (_, line) = movement::line(text, text.byte_to_line(self.selection.range.start));
        let indent = if line.starts_with('\t') { "\t" } else { INDENT };
        self.insert(None, indent, cx);
    }

    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        self.history(Document::undo, window, cx);
    }

    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        self.history(Document::redo, window, cx);
    }

    fn history(
        &mut self,
        step: fn(&mut Document, &mut Context<Document>) -> Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(document) = self.document.clone() else {
            return;
        };
        let cursor = document.update(cx, |document, cx| {
            let cursor = step(document, cx);
            if cursor.is_some() && SAVE.get(cx) == SaveMode::Auto {
                document.save_after(AUTO_SAVE_DELAY, cx);
            }
            cursor
        });
        if let Some(cursor) = cursor {
            self.move_cursor(cursor, false, window, cx);
        }
    }

    /// Shows lines hidden in a fold.
    fn reveal(&mut self, lines: Range<usize>, cx: &mut Context<Self>) {
        self.revealed.push(lines);
        self.update_rows(cx);
    }

    /// The text offset under `position`, from where lines were painted last frame.
    fn offset_at(&self, position: Point<Pixels>) -> Option<usize> {
        let layouts = self.layouts.borrow();
        let layout = layouts
            .iter()
            .find(|layout| position.y < layout.bounds.bottom())
            .or(layouts.last())?;
        Some(layout.offset_at(position.x, self.scroll_x))
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let Some(offset) = self.offset_at(event.position) else {
            return;
        };
        self.selecting = true;
        if event.click_count == 2
            && !event.modifiers.shift
            && let Some(text) = self.text(cx)
        {
            let start = movement::previous_word(text, movement::next(text, offset));
            let end = movement::next_word(text, start);
            self.selection.select(start..end);
            self.goal_column = None;
            window.notify_text_selection_changed(&self.focus_handle);
            cx.notify();
        } else {
            self.move_cursor(offset, event.modifiers.shift, window, cx);
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selecting
            && let Some(offset) = self.offset_at(event.position)
        {
            self.move_cursor(offset, true, window, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    /// Scrolls the text sideways. The list scrolls it up and down.
    fn on_scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        let line_height = cx.density().code.line_height.to_pixels(window.rem_size());
        let delta = event.delta.pixel_delta(line_height);
        let sideways = if event.modifiers.shift && delta.x == Pixels::ZERO {
            delta.y
        } else {
            delta.x
        };
        if sideways != Pixels::ZERO {
            self.set_scroll_x((self.scroll_x - sideways).max(Pixels::ZERO), cx);
        }
    }
}

impl TextEditing for Editor {
    fn rope(&self, cx: &App) -> Option<Rope> {
        self.text(cx).cloned()
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
        self.goal_column = None;
        window.notify_text_selection_changed(&self.focus_handle);
        self.cursor_moved(cx);
    }

    fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(len) = self.text(cx).map(Rope::len_bytes) else {
            return;
        };
        self.selection.select(0..len);
        self.goal_column = None;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert(range, text, cx);
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc};

    use gpui::{
        AppContext as _, EntityInputHandler, Modifiers, MouseButton, TestAppContext,
        VisualTestContext, point,
    };
    use model::{
        FileKey, GitBackend, Projects, Source, Version,
        fixtures::{FakeGit, PENSIVE_ROOT},
    };

    use super::*;
    use crate::test_support::secondary;

    const PATH: &str = "crates/theme/src/colors.rs";

    /// An editor in a window, focused, on the sample file at `path` from `source`, with the
    /// cursor at its start.
    async fn editor_on<'a>(
        source: Source,
        path: &str,
        cx: &'a mut TestAppContext,
    ) -> (Entity<Editor>, Arc<FakeGit>, &'a mut VisualTestContext) {
        crate::test_support::init(cx);
        let git = Arc::new(FakeGit::default());
        let projects = cx.new(|_| Projects::new(git.clone()));
        let key = FileKey {
            source,
            path: path.into(),
        };
        let open = projects.update(cx, |projects, cx| {
            projects.open_document(PENSIVE_ROOT.into(), key.clone(), cx)
        });
        open.await.unwrap();
        let document = projects.read_with(cx, |projects: &Projects, _| {
            projects.document(Path::new(PENSIVE_ROOT), &key).cloned()
        });
        let (editor, cx) = cx.add_window_view(Editor::new);
        editor.update_in(cx, |editor, window, cx| {
            editor.set_document(document, cx);
            editor.move_cursor(0, false, window, cx);
            window.focus(&editor.focus_handle, cx);
        });
        cx.run_until_parked();
        (editor, git, cx)
    }

    /// [`editor_on`] the uncommitted colors file.
    async fn editor(
        cx: &mut TestAppContext,
    ) -> (Entity<Editor>, Arc<FakeGit>, &mut VisualTestContext) {
        editor_on(Source::Uncommitted, PATH, cx).await
    }

    fn text(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> String {
        editor.read_with(cx, |editor, cx| editor.text(cx).unwrap().to_string())
    }

    fn on_disk(git: &FakeGit) -> String {
        match git
            .read_file(Path::new(PENSIVE_ROOT), Path::new(PATH))
            .unwrap()
        {
            Version::Text(text) => text,
            version => panic!("expected text, found {version:?}"),
        }
    }

    #[gpui::test]
    async fn typing_saves_a_second_after_it_stops(cx: &mut TestAppContext) {
        let (editor, git, cx) = editor(cx).await;
        cx.simulate_input("abc");
        assert!(text(&editor, cx).starts_with("abc"));
        cx.executor().advance_clock(AUTO_SAVE_DELAY / 2);
        cx.run_until_parked();
        assert!(!on_disk(&git).starts_with("abc"));
        cx.executor().advance_clock(AUTO_SAVE_DELAY);
        cx.run_until_parked();
        assert!(on_disk(&git).starts_with("abc"));
    }

    #[gpui::test]
    async fn manual_save_waits_for_the_shortcut(cx: &mut TestAppContext) {
        let (editor, git, cx) = editor(cx).await;
        cx.update(|_, cx| SAVE.set(SaveMode::Manual, cx));
        cx.simulate_input("x");
        cx.executor().advance_clock(AUTO_SAVE_DELAY * 3);
        cx.run_until_parked();
        assert!(!on_disk(&git).starts_with('x'));
        editor.update_in(cx, |editor, window, cx| editor.save(&Save, window, cx));
        cx.run_until_parked();
        assert!(on_disk(&git).starts_with('x'));
    }

    #[gpui::test]
    async fn a_new_line_keeps_the_indentation(cx: &mut TestAppContext) {
        let (editor, _, cx) = editor(cx).await;
        cx.simulate_keystrokes("end enter");
        cx.simulate_input("next");
        let text = text(&editor, cx);
        assert_eq!(text.lines().nth(1), Some("    next"));
    }

    #[gpui::test]
    async fn a_new_line_in_a_crlf_file_ends_with_crlf(cx: &mut TestAppContext) {
        let (editor, _, cx) = editor(cx).await;
        cx.simulate_input("first\r\n");
        cx.simulate_keystrokes("up end enter");
        assert!(text(&editor, cx).starts_with("first\r\n\r\n"));
    }

    #[gpui::test]
    async fn undoing_a_change_leaves_the_cursor_unfolded(cx: &mut TestAppContext) {
        let (editor, _, cx) = editor(cx).await;
        cx.simulate_keystrokes("ctrl-end");
        cx.simulate_input("x");
        cx.run_until_parked();
        editor.update_in(cx, |editor, window, cx| editor.undo(&Undo, window, cx));
        cx.run_until_parked();
        editor.read_with(cx, |editor, cx| {
            let line = editor
                .text(cx)
                .unwrap()
                .byte_to_line(editor.selection.cursor());
            assert!(editor.row_of(line).is_some());
        });
    }

    #[gpui::test]
    async fn undo_takes_back_the_typing_since_the_cursor_last_moved(cx: &mut TestAppContext) {
        let (editor, _, cx) = editor(cx).await;
        let original = text(&editor, cx);
        cx.simulate_input("one");
        cx.simulate_keystrokes("right");
        cx.simulate_input("two");
        editor.update_in(cx, |editor, window, cx| editor.undo(&Undo, window, cx));
        assert!(text(&editor, cx).starts_with("one"));
        editor.update_in(cx, |editor, window, cx| editor.undo(&Undo, window, cx));
        assert_eq!(text(&editor, cx), original);
    }

    fn goal_column(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Option<usize> {
        editor.read_with(cx, |editor, _| editor.goal_column)
    }

    #[gpui::test]
    async fn left_or_right_on_a_selection_forgets_the_goal_column(cx: &mut TestAppContext) {
        let (editor, _, cx) = editor(cx).await;
        for key in ["left", "right"] {
            cx.simulate_keystrokes("shift-down");
            assert!(goal_column(&editor, cx).is_some());
            cx.simulate_keystrokes(key);
            assert_eq!(goal_column(&editor, cx), None, "after {key}");
        }
    }

    /// Sets a goal column with Down, then checks that `step` forgets it.
    fn forgets_the_goal_column(
        editor: &Entity<Editor>,
        cx: &mut VisualTestContext,
        name: &str,
        step: impl FnOnce(&mut VisualTestContext),
    ) {
        cx.simulate_keystrokes("down");
        assert!(goal_column(editor, cx).is_some());
        step(cx);
        assert_eq!(goal_column(editor, cx), None, "after {name}");
    }

    #[gpui::test]
    #[expect(
        clippy::disallowed_methods,
        reason = "the test clicks at window pixels"
    )]
    async fn clicks_undo_and_select_all_forget_the_goal_column(cx: &mut TestAppContext) {
        let (editor, _, cx) = editor(cx).await;
        let inside = point(px(300.), px(20.));
        cx.simulate_input("x");
        forgets_the_goal_column(&editor, cx, "click", |cx| {
            crate::test_support::click(cx, inside)
        });
        forgets_the_goal_column(&editor, cx, "double click", |cx| {
            cx.simulate_event(MouseDownEvent {
                button: MouseButton::Left,
                position: inside,
                modifiers: Modifiers::none(),
                click_count: 2,
                first_mouse: false,
            });
            cx.simulate_mouse_up(inside, MouseButton::Left, Modifiers::none());
        });
        forgets_the_goal_column(&editor, cx, "undo", |cx| {
            cx.simulate_keystrokes(&secondary("z"))
        });
        forgets_the_goal_column(&editor, cx, "select all", |cx| {
            cx.simulate_keystrokes(&secondary("a"))
        });
        forgets_the_goal_column(&editor, cx, "an input method's selection", |cx| {
            editor.update_in(cx, |editor, window, cx| {
                editor.set_selected_text_range(0..1, window, cx)
            })
        });
    }

    #[gpui::test]
    async fn a_commits_file_ignores_typing(cx: &mut TestAppContext) {
        let source = Source::Commit("5e35ea9".into());
        let (editor, _, cx) = editor_on(source, "crates/ui/src/lib.rs", cx).await;
        let original = text(&editor, cx);
        cx.simulate_input("x");
        cx.simulate_keystrokes("enter backspace");
        assert_eq!(text(&editor, cx), original);
    }
}

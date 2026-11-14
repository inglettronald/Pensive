//! A code editor over a document: it shows the document's diff in one of three modes, edits it
//! from the keyboard, mouse and input method, and saves as the save setting says.

mod input;
mod keys;
mod line;
mod movement;
mod render;
mod rows;
mod saving;

use std::ops::Range;

use gpui::{
    App, ClipboardItem, Context, Entity, FocusHandle, Focusable, ListAlignment, ListState,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollEvent, Subscription, Window,
    px,
};
use model::{Document, Rope};
use theme::ActiveTheme;

pub(crate) use keys::bind_keys;
use keys::*;
use line::Layouts;
pub use rows::Mode;
use rows::{Row, rows};
use saving::AUTO_SAVE_DELAY;
pub(crate) use saving::{SAVE, SaveMode, declare_settings};

/// What Tab inserts, unless the line is indented with tabs.
const INDENT: &str = "    ";

pub struct Editor {
    document: Option<Entity<Document>>,
    mode: Mode,
    /// Unchanged lines opened from folds in Changes mode, by line.
    revealed: Vec<Range<usize>>,
    rows: Vec<Row>,
    list: ListState,
    /// In bytes. The cursor is its end, or its start when `reversed`.
    selection: Range<usize>,
    reversed: bool,
    /// Text the input method is composing.
    marked: Option<Range<usize>>,
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
            list: ListState::new(0, ListAlignment::Top, px(200.)),
            selection: 0..0,
            reversed: false,
            marked: None,
            goal_column: None,
            selecting: false,
            scroll_x: px(0.),
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
        self.selection = first_change..first_change;
        self.reversed = false;
        self.marked = None;
        self.goal_column = None;
        self.revealed.clear();
        self.scroll_x = px(0.);
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
        self.move_to(offset, cx);
        self.insert(None, text, cx);
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
        let Some(text) = self.text(cx) else {
            return;
        };
        let clamp =
            |offset: usize| text.char_to_byte(text.byte_to_char(offset.min(text.len_bytes())));
        self.selection = clamp(self.selection.start)..clamp(self.selection.end);
        self.marked = self
            .marked
            .take()
            .map(|marked| clamp(marked.start)..clamp(marked.end))
            .filter(|marked| !marked.is_empty());
    }

    /// Rebuilds the rows from the document and tells the list which ones changed height.
    fn update_rows(&mut self, cx: &mut Context<Self>) {
        let rows = match &self.document {
            Some(document) => {
                let document = document.read(cx);
                let cursor = document.text().byte_to_line(self.cursor());
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

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selection = offset..offset;
        self.reversed = false;
        self.cursor_moved(cx);
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.reversed = !self.reversed;
            self.selection = self.selection.end..self.selection.start;
        }
        self.cursor_moved(cx);
    }

    fn cursor_moved(&mut self, cx: &mut Context<Self>) {
        if let Some(document) = &self.document {
            document.update(cx, |document, _| document.break_undo_group());
        }
        self.reveal_cursor(cx);
    }

    /// Shows the cursor's line, which a fold never hides, then scrolls to it.
    fn reveal_cursor(&mut self, cx: &mut Context<Self>) {
        let Some(line) = self.text(cx).map(|text| text.byte_to_line(self.cursor())) else {
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

    /// Moves or extends the selection to `target`, given the text and the cursor.
    fn motion(
        &mut self,
        select: bool,
        target: impl FnOnce(&Rope, usize) -> usize,
        cx: &mut Context<Self>,
    ) {
        let Some(text) = self.text(cx) else {
            return;
        };
        let offset = target(text, self.cursor());
        self.goal_column = None;
        if select {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    /// The offset `rows` text rows away from the cursor, aiming for the goal column.
    fn vertical(&mut self, rows: isize, cx: &App) -> Option<usize> {
        let text = self.text(cx)?;
        let cursor = self.cursor();
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

    fn move_vertically(&mut self, rows: isize, select: bool, cx: &mut Context<Self>) {
        let goal = self.goal_column;
        let Some(offset) = self.vertical(rows, cx) else {
            return;
        };
        let goal_column = self.goal_column.or(goal);
        if select {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        self.goal_column = goal_column;
    }

    /// Text rows that fit in the viewport, for Page Up and Page Down.
    fn page(&self, window: &Window, cx: &App) -> isize {
        let height = self.list.viewport_bounds().size.height;
        let line_height = cx.density().code.line_height.to_pixels(window.rem_size());
        ((height / line_height) as isize - 1).max(1)
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            self.motion(false, movement::previous, cx);
        } else {
            self.move_to(self.selection.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            self.motion(false, movement::next, cx);
        } else {
            self.move_to(self.selection.end, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, false, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(true, movement::previous, cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(true, movement::next, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, true, cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(false, movement::previous_word, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(false, movement::next_word, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(true, movement::previous_word, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(true, movement::next_word, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(false, movement::home, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(false, movement::end, cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(true, movement::home, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(true, movement::end, cx);
    }

    fn top(&mut self, _: &Top, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(false, |_, _| 0, cx);
    }

    fn bottom(&mut self, _: &Bottom, _: &mut Window, cx: &mut Context<Self>) {
        self.motion(false, |text, _| text.len_bytes(), cx);
    }

    fn page_up(&mut self, _: &PageUp, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-self.page(window, cx), false, cx);
    }

    fn page_down(&mut self, _: &PageDown, window: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(self.page(window, cx), false, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        let Some(len) = self.text(cx).map(Rope::len_bytes) else {
            return;
        };
        self.selection = 0..len;
        self.reversed = false;
        cx.notify();
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
        let range = range
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
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
        let cursor = range.start + text.len();
        self.selection = cursor..cursor;
        self.reversed = false;
        self.marked = None;
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

    /// Deletes the selection, or from the cursor to `target` when nothing is selected.
    fn delete_to(&mut self, target: fn(&Rope, usize) -> usize, cx: &mut Context<Self>) {
        let Some(text) = self.text(cx) else {
            return;
        };
        let range = if self.selection.is_empty() {
            let (cursor, target) = (self.cursor(), target(text, self.cursor()));
            cursor.min(target)..cursor.max(target)
        } else {
            self.selection.clone()
        };
        if !range.is_empty() {
            self.insert(Some(range), "", cx);
        }
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(movement::previous, cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(movement::next, cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(movement::previous_word, cx);
    }

    /// Starts a new line with the current line's indentation and line ending.
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.text(cx) else {
            return;
        };
        let index = text.byte_to_line(self.selection.start);
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
        let (_, line) = movement::line(text, text.byte_to_line(self.selection.start));
        let indent = if line.starts_with('\t') { "\t" } else { INDENT };
        self.insert(None, indent, cx);
    }

    fn selected_text(&self, cx: &App) -> Option<String> {
        let text = self.text(cx)?;
        (!self.selection.is_empty()).then(|| text.byte_slice(self.selection.clone()).to_string())
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text(cx) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text(cx) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.insert(None, "", cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.insert(None, &text, cx);
        }
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.history(Document::undo, cx);
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.history(Document::redo, cx);
    }

    fn history(
        &mut self,
        step: fn(&mut Document, &mut Context<Document>) -> Option<usize>,
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
            self.move_to(cursor, cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
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
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else if event.click_count == 2
            && let Some(text) = self.text(cx)
        {
            let start = movement::previous_word(text, movement::next(text, offset));
            let end = movement::next_word(text, start);
            self.selection = start..end;
            self.reversed = false;
            cx.notify();
        } else {
            self.move_to(offset, cx);
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting
            && let Some(offset) = self.offset_at(event.position)
        {
            self.select_to(offset, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    /// Scrolls the text sideways. The list scrolls it up and down.
    fn on_scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        let line_height = cx.density().code.line_height.to_pixels(window.rem_size());
        let delta = event.delta.pixel_delta(line_height);
        let sideways = if event.modifiers.shift && delta.x == px(0.) {
            delta.y
        } else {
            delta.x
        };
        if sideways != px(0.) {
            self.set_scroll_x((self.scroll_x - sideways).max(px(0.)), cx);
        }
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

    use gpui::{AppContext as _, TestAppContext, VisualTestContext};
    use model::{
        FileKey, GitBackend, Projects, Source, Version,
        fixtures::{FakeGit, PENSIVE_ROOT},
    };

    use super::*;

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
            editor.move_to(0, cx);
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
            let line = editor.text(cx).unwrap().byte_to_line(editor.cursor());
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

//! Draws the editor: its rows, the folds between them, and the text it scrolls.

use std::ops::Range;

use components::{Button, DiffRow, IconName, LineKind};
use gpui::{
    AnyElement, App, Context, CursorStyle, ElementId, ElementInputHandler, HighlightStyle,
    MouseButton, StyledText, WeakEntity, Window, canvas, div, list, prelude::*, px,
};
use model::changed_words;
use theme::{ActiveTheme, Typeset};

use super::{
    Editor,
    keys::CONTEXT,
    line::LineElement,
    movement,
    rows::{Mode, Row, STEP},
};

impl Editor {
    fn render_row(
        &self,
        index: usize,
        this: &WeakEntity<Self>,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        let (Some(row), Some(document)) = (self.rows.get(index), &self.document) else {
            return div().into_any_element();
        };
        let document = document.read(cx);
        let colors = &cx.theme().colors;
        let marks = self.mode != Mode::Plain;
        let word = |kind: LineKind| kind.hue(colors).map(|hue| hue.opacity(0.28));
        match row.clone() {
            Row::Line { line, added, pair } => {
                let kind = if added && marks {
                    LineKind::Added
                } else {
                    LineKind::Context
                };
                let (start, text) = movement::line(document.text(), line);
                let end = start + text.len();
                let highlights = match (pair, word(kind)) {
                    (Some(pair), Some(color)) if marks => {
                        changed_words(document.base_line(pair), &text)
                            .1
                            .into_iter()
                            .map(|range| (range, color))
                            .collect()
                    }
                    _ => Vec::new(),
                };
                let selection = (self.selection.start <= end
                    && self.selection.end >= start
                    && !self.selection.is_empty())
                .then(|| {
                    let from = self.selection.start.max(start) - start;
                    let to = self.selection.end.min(end) - start;
                    (from..to, self.selection.end > end)
                });
                let cursor = self.cursor();
                let focused = self.focus_handle.is_focused(window);
                let element = LineElement {
                    editor: this.clone(),
                    line,
                    start,
                    text: text.into(),
                    selection,
                    cursor: (focused && (start..=end).contains(&cursor)).then(|| cursor - start),
                    highlights,
                    marked: self
                        .marked
                        .as_ref()
                        .filter(|marked| start <= marked.start && marked.end <= end)
                        .map(|marked| marked.start - start..marked.end - start),
                    scroll_x: self.scroll_x,
                    autoscroll: self.autoscroll,
                    layouts: self.layouts.clone(),
                };
                DiffRow::new(kind, Some(line + 1), element).into_any_element()
            }
            Row::Removed { line, pair } => {
                let text = document.base_line(line).to_string();
                let words = pair
                    .filter(|_| marks)
                    .map(|pair| changed_words(&text, &movement::line(document.text(), pair).1).0)
                    .unwrap_or_default();
                let style = HighlightStyle {
                    background_color: word(LineKind::Removed),
                    ..Default::default()
                };
                let text = StyledText::new(text)
                    .with_highlights(words.into_iter().map(|range| (range, style)));
                DiffRow::new(
                    LineKind::Removed,
                    None,
                    div().ml(-self.scroll_x).child(text),
                )
                .into_any_element()
            }
            Row::Fold { lines, down, up } => self.render_fold(lines, down, up, this, cx),
        }
    }

    /// Buttons that show hidden lines from the top, from the bottom, or all of them, then how
    /// many are hidden.
    fn render_fold(
        &self,
        lines: Range<usize>,
        down: bool,
        up: bool,
        this: &WeakEntity<Self>,
        cx: &App,
    ) -> AnyElement {
        let theme = cx.theme();
        let density = cx.density();
        let reveal = |range: Range<usize>| {
            let this = this.clone();
            move |_: &mut Window, cx: &mut App| {
                this.update(cx, |this, cx| this.reveal(range.clone(), cx))
                    .ok();
            }
        };
        let step = STEP.min(lines.len());
        let id = |name: &str| ElementId::Name(format!("fold-{name}-{}", lines.start).into());
        div()
            // A click on a fold is for its buttons, not for placing the cursor.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .flex()
            .w_full()
            .items_center()
            .gap_1()
            .h(density.row_height + px(4.))
            .my_0p5()
            .px_2()
            .border_y_1()
            .border_color(theme.colors.border_subtle)
            .bg(theme.colors.surface_panel)
            .font_family(theme.fonts.ui.clone())
            .typeset(density.small)
            .text_color(theme.colors.text_faint)
            .when(down, |fold| {
                fold.child(
                    Button::new(id("down"), step.to_string())
                        .icon(IconName::ArrowDown)
                        .small()
                        .on_click(reveal(lines.start..lines.start + step)),
                )
            })
            .when(up, |fold| {
                fold.child(
                    Button::new(id("up"), step.to_string())
                        .icon(IconName::ArrowUp)
                        .small()
                        .on_click(reveal(lines.end - step..lines.end)),
                )
            })
            .child(
                Button::new(id("all"), "All")
                    .icon(IconName::ChevronsUpDown)
                    .small()
                    .on_click(reveal(lines.clone())),
            )
            .child(div().pl_2().child(match lines.len() {
                1 => "1 unchanged line".to_string(),
                count => format!("{count} unchanged lines"),
            }))
            .into_any_element()
    }
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Lines record where they're painted this frame.
        self.layouts.borrow_mut().clear();
        let theme = cx.theme();
        let code = cx.density().code;
        let rows = cx.entity().downgrade();
        let entity = cx.entity();
        let focus_handle = self.focus_handle.clone();
        div()
            .id("editor")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::top))
            .on_action(cx.listener(Self::bottom))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::show_character_palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll(cx.listener(Self::on_scroll))
            .relative()
            .size_full()
            .py_1()
            .cursor(CursorStyle::IBeam)
            .font_family(theme.fonts.code.clone())
            .typeset(code)
            .text_color(theme.colors.text)
            .whitespace_nowrap()
            .child(
                list(self.list.clone(), move |index, window, cx| {
                    let Some(editor) = rows.upgrade() else {
                        return div().into_any_element();
                    };
                    editor.read(cx).render_row(index, &rows, window, cx)
                })
                .size_full(),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        window.handle_input(
                            &focus_handle,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}

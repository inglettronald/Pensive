//! Draws the editor: its rows, the folds between them, and the text it scrolls.

use std::ops::Range;

use components::{Button, DiffRow, IconName, LineKind};
use gpui::{
    AnyElement, App, Context, CursorStyle, ElementId, ElementInputHandler, MouseButton, WeakEntity,
    Window, canvas, div, list, prelude::*,
};
use model::{DocumentSide, changed_words};
use theme::{ActiveTheme, Typeset, dp};

use super::{
    Editor,
    display_line::{DisplayLine, local_range, runs},
    keys::CONTEXT,
    line::{LineElement, LineInteraction},
    rows::{Mode, Row, STEP},
    syntax::foregrounds,
};
use crate::text_editing::{self, movement};

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
        let display_line =
            |side, start, text: String, highlights: &[_], marked: Option<&Range<usize>>| {
                let foregrounds = foregrounds(
                    document.syntax_spans(&self.syntax_viewport, side),
                    start,
                    &text,
                    colors,
                );
                let styles = runs(
                    &text,
                    window.text_style().to_run(text.len()),
                    &foregrounds,
                    highlights,
                    marked,
                );
                DisplayLine::compose(start, text, styles, &[]).into()
            };
        match row.clone() {
            Row::Line { line, added, pair } => {
                let kind = if added && marks {
                    LineKind::Added
                } else {
                    LineKind::Context
                };
                let (start, text) = movement::line(document.text(), line);
                let end = start + text.len();
                let highlights: Vec<_> = match (pair, word(kind)) {
                    (Some(pair), Some(color)) if marks => {
                        changed_words(document.base_line(pair), &text)
                            .1
                            .into_iter()
                            .map(|range| (range, color))
                            .collect()
                    }
                    _ => Vec::new(),
                };
                let selected = &self.selection.range;
                let selection = (selected.start <= end
                    && selected.end >= start
                    && !selected.is_empty())
                .then(|| {
                    let from = selected.start.max(start);
                    let to = selected.end.min(end);
                    (from..to, selected.end > end)
                });
                let cursor = self.selection.cursor();
                let focused = self.focus_handle.is_focused(window);
                let marked = self
                    .selection
                    .marked
                    .as_ref()
                    .and_then(|range| local_range(&text, start, range));
                let element = LineElement {
                    display: display_line(
                        DocumentSide::New,
                        start,
                        text,
                        &highlights,
                        marked.as_ref(),
                    ),
                    interaction: Some(LineInteraction {
                        line,
                        selection,
                        cursor: (focused && (start..=end).contains(&cursor)).then_some(cursor),
                        autoscroll: self.autoscroll.then(|| this.clone()),
                        layouts: self.layouts.clone(),
                    }),
                    scroll_x: self.scroll_x,
                };
                DiffRow::new(kind, Some(line + 1), element).into_any_element()
            }
            Row::Removed { line, pair } => {
                let text = document.base_line(line);
                let highlights: Vec<_> = pair
                    .zip(word(LineKind::Removed))
                    .filter(|(pair, _)| marks && *pair < document.text().len_lines())
                    .map(|(pair, color)| {
                        changed_words(text, &movement::line(document.text(), pair).1)
                            .0
                            .into_iter()
                            .map(|range| (range, color))
                            .collect()
                    })
                    .unwrap_or_default();
                let element = LineElement {
                    display: display_line(
                        DocumentSide::Base,
                        document
                            .base_line_range(line)
                            .map_or(0, |range| range.start),
                        text.to_owned(),
                        &highlights,
                        None,
                    ),
                    scroll_x: self.scroll_x,
                    interaction: None,
                };
                div()
                    .cursor(CursorStyle::Arrow)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(DiffRow::new(LineKind::Removed, None, element))
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
            .h(density.row_height + dp(4.))
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
        let syntax = rows.clone();
        let entity = cx.entity();
        let focus_handle = self.focus_handle.clone();
        let editor = div().id("editor").track_focus(&self.focus_handle);
        text_editing::attach(editor, CONTEXT, cx)
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::top))
            .on_action(cx.listener(Self::bottom))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save))
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
                    move |_, _, cx| Self::schedule_syntax(&syntax, cx),
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

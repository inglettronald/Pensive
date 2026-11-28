//! Schedules syntax from the laid-out list and maps model spans to rendered lines.

use std::ops::Range;

use gpui::{App, Hsla, WeakEntity};
use model::{Document, DocumentSide, TokenRole, TokenSpan};
use theme::ThemeColors;

use super::{Editor, Row, display_line::local_range};

// Keep nearby rows colored while scrolling a few lines at a time.
const PREFETCH_ROWS: usize = 8;

impl Editor {
    /// Called in prepaint, after the list has resolved its actual scroll anchor
    /// and measured the viewport. Submit from a deferred app update.
    pub(super) fn schedule_syntax(editor: &WeakEntity<Self>, cx: &mut App) {
        let Some(view) = editor.upgrade() else { return };
        if view.read(cx).syntax_requests(cx).next().is_none() {
            return;
        }
        let editor = editor.clone();
        cx.defer(move |cx| {
            let Some(view) = editor.upgrade() else { return };
            let view = view.read(cx);
            let Some(document) = view.document.clone() else {
                return;
            };
            // Read the latest rows and source again in case a document update
            // was queued during the frame.
            let requests: Vec<_> = view.syntax_requests(cx).collect();
            if !requests.is_empty() {
                let viewport = view.syntax_viewport.clone();
                document.update(cx, |document, cx| {
                    for (side, ranges) in requests {
                        document
                            .request_viewport_syntax(&viewport, side, ranges, cx)
                            .expect("viewport ranges come from the current document");
                    }
                });
            }
        });
    }

    fn syntax_requests(&self, cx: &App) -> impl Iterator<Item = (DocumentSide, Vec<Range<usize>>)> {
        let start = self.list.logical_scroll_top().item_ix.min(self.rows.len());
        let bottom = self.list.viewport_bounds().bottom();
        let end = (start..self.rows.len())
            .take_while(|&index| {
                self.list
                    .bounds_for_item(index)
                    .is_some_and(|bounds| bounds.top() < bottom)
            })
            .last()
            .map_or(start, |index| index + 1);

        self.document.iter().flat_map(move |document| {
            let document = document.read(cx);
            [DocumentSide::Base, DocumentSide::New]
                .into_iter()
                .filter_map(move |side| {
                    if start == end {
                        return None;
                    }
                    let visible = source_ranges(document, side, &self.rows[start..end]);
                    if !document.syntax_viewport_needs(&self.syntax_viewport, side, &visible) {
                        return None;
                    }
                    if visible.is_empty() {
                        return Some((side, Vec::new()));
                    }
                    let before = start.saturating_sub(PREFETCH_ROWS);
                    let after = (end + PREFETCH_ROWS).min(self.rows.len());
                    Some((
                        side,
                        source_ranges(document, side, &self.rows[before..after]),
                    ))
                })
        })
    }
}

fn source_ranges(document: &Document, side: DocumentSide, rows: &[Row]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut previous = None;
    for row in rows {
        let (line, bytes) = match (side, row) {
            (DocumentSide::Base, Row::Removed { line, .. }) => {
                let Some(bytes) = document.base_line_range(*line) else {
                    continue;
                };
                (*line, bytes)
            }
            (DocumentSide::New, Row::Line { line, .. }) => {
                let text = document.text();
                if *line >= text.len_lines() {
                    continue;
                }
                (
                    *line,
                    text.line_to_byte(*line)..text.line_to_byte(*line + 1),
                )
            }
            _ => continue,
        };
        if previous.is_some_and(|last| line == last + 1)
            && let Some(last) = ranges.last_mut()
        {
            last.end = bytes.end;
        } else {
            ranges.push(bytes);
        }
        previous = Some(line);
    }
    ranges.retain(|range| !range.is_empty());
    ranges
}

pub(super) fn foregrounds(
    spans: &[TokenSpan],
    start: usize,
    text: &str,
    colors: &ThemeColors,
) -> Vec<(Range<usize>, Hsla)> {
    let first = spans.partition_point(|span| span.bytes().end <= start);
    spans[first..]
        .iter()
        .take_while(|span| span.bytes().start < start + text.len())
        .filter_map(|span| {
            Some((
                local_range(text, start, &span.bytes())?,
                match span.role() {
                    TokenRole::Keyword => colors.syntax_keyword,
                    TokenRole::String | TokenRole::TagValue => colors.syntax_string,
                    TokenRole::Number => colors.syntax_number,
                    TokenRole::Comment => colors.syntax_comment,
                    TokenRole::Type | TokenRole::TypeParameter => colors.syntax_type,
                    TokenRole::Function | TokenRole::Method => colors.syntax_function,
                    TokenRole::FunctionCall | TokenRole::MethodCall => colors.syntax_call,
                    TokenRole::Parameter | TokenRole::CapturedVariable => colors.syntax_parameter,
                    TokenRole::Property => colors.syntax_property,
                    TokenRole::Constant => colors.syntax_constant,
                    TokenRole::Annotation => colors.syntax_annotation,
                    TokenRole::Operator => colors.syntax_operator,
                    TokenRole::Tag => colors.syntax_tag,
                    TokenRole::TagAttribute => colors.syntax_tag_attribute,
                    TokenRole::Plain | TokenRole::Variable => return None,
                },
            ))
        })
        .collect()
}

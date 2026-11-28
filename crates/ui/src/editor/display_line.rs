//! Composes one displayed row and maps between its display bytes and document bytes.

use std::{iter::Peekable, ops::Range};

use gpui::{Hsla, SharedString, TextRun, UnderlineStyle};
use theme::HAIRLINE;

/// Inline text occupies space on screen at a single document offset.
pub(super) struct InlineText {
    pub anchor: usize,
    pub text: SharedString,
    pub style: TextRun,
}

enum Origin {
    SourceText(Range<usize>),
    AnchoredText(usize),
}

struct LinePart {
    origin: Origin,
    display_start: usize,
    style: TextRun,
}

impl LinePart {
    fn source(&self) -> Range<usize> {
        match &self.origin {
            Origin::SourceText(bytes) => bytes.clone(),
            Origin::AnchoredText(anchor) => *anchor..*anchor,
        }
    }
}

pub(super) struct DisplayLine {
    pub text: SharedString,
    parts: Vec<LinePart>,
    source: Range<usize>,
}

impl DisplayLine {
    /// Runs cover the source in order. Inserts use ordered absolute UTF-8
    /// boundaries on this line and are displayed in input order at a shared anchor.
    pub fn compose(start: usize, text: String, runs: Vec<TextRun>, inserts: &[InlineText]) -> Self {
        let source = start..start + text.len();
        let mut parts = Vec::with_capacity(runs.len() + inserts.len());
        let mut displayed = (!inserts.is_empty()).then(String::new);
        let mut display_offset = 0;
        let mut push = |origin, slice: &str, mut style: TextRun| {
            if slice.is_empty() {
                return;
            }
            style.len = slice.len();
            parts.push(LinePart {
                origin,
                display_start: display_offset,
                style,
            });
            if let Some(buffer) = &mut displayed {
                buffer.push_str(slice);
            }
            display_offset += slice.len();
        };

        let mut inserts = inserts.iter().peekable();
        let mut offset = 0;
        for run in runs {
            let end = offset + run.len;
            while let Some(insert) = inserts.next_if(|insert| insert.anchor < start + end) {
                let at = insert
                    .anchor
                    .checked_sub(start)
                    .expect("anchor precedes line");
                push(
                    Origin::SourceText(start + offset..insert.anchor),
                    &text[offset..at],
                    run.clone(),
                );
                push(
                    Origin::AnchoredText(insert.anchor),
                    &insert.text,
                    insert.style.clone(),
                );
                offset = at;
            }
            push(
                Origin::SourceText(start + offset..start + end),
                &text[offset..end],
                run,
            );
            offset = end;
        }
        assert_eq!(offset, text.len(), "styles must cover the source line");
        for insert in inserts {
            assert_eq!(insert.anchor, source.end, "anchor beyond line");
            push(
                Origin::AnchoredText(insert.anchor),
                &insert.text,
                insert.style.clone(),
            );
        }
        Self {
            text: displayed.unwrap_or(text).into(),
            parts,
            source,
        }
    }

    pub fn runs(&self) -> Vec<TextRun> {
        self.parts.iter().map(|part| part.style.clone()).collect()
    }

    pub fn source_offset(&self, display: usize) -> usize {
        let index = self
            .parts
            .partition_point(|part| part.display_start + part.style.len <= display);
        self.parts.get(index).map_or(self.source.end, |part| {
            let bytes = part.source();
            bytes.start + (display - part.display_start).min(bytes.len())
        })
    }

    /// At an insert's anchor the caret sits before the inserted text.
    pub fn display_offset(&self, source: usize) -> usize {
        let source = source.clamp(self.source.start, self.source.end);
        let index = self
            .parts
            .partition_point(|part| part.source().end < source);
        self.parts.get(index).map_or(self.text.len(), |part| {
            part.display_start + source.saturating_sub(part.source().start)
        })
    }
}

/// Clip absolute bytes to the displayed line, preserving scalar boundaries.
pub(super) fn local_range(text: &str, start: usize, bytes: &Range<usize>) -> Option<Range<usize>> {
    let from = bytes.start.max(start) - start;
    let to = bytes.end.min(start + text.len()).saturating_sub(start);
    (from < to && text.is_char_boundary(from) && text.is_char_boundary(to)).then_some(from..to)
}

/// Merge ordered, disjoint ranges within each layer. Every run uses the same
/// font, even where foreground, background and the IME underline overlap.
pub(super) fn runs(
    text: &str,
    base: TextRun,
    foregrounds: &[(Range<usize>, Hsla)],
    highlights: &[(Range<usize>, Hsla)],
    marked: Option<&Range<usize>>,
) -> Vec<TextRun> {
    let mut foregrounds = foregrounds.iter().cloned().peekable();
    let mut highlights = highlights.iter().cloned().peekable();
    let mut marked = marked
        .cloned()
        .map(|range| (range, ()))
        .into_iter()
        .peekable();
    let mut offset = 0;
    let mut result = Vec::new();
    while offset < text.len() {
        let mut end = text.len();
        let color = layer(&mut foregrounds, text, offset, &mut end).unwrap_or(base.color);
        let background = layer(&mut highlights, text, offset, &mut end);
        let underline = layer(&mut marked, text, offset, &mut end).map(|()| UnderlineStyle {
            color: Some(base.color),
            thickness: HAIRLINE,
            wavy: false,
        });
        result.push(TextRun {
            len: end - offset,
            color,
            background_color: background,
            underline,
            ..base.clone()
        });
        offset = end;
    }
    result
}

fn layer<T: Copy>(
    ranges: &mut Peekable<impl Iterator<Item = (Range<usize>, T)>>,
    text: &str,
    offset: usize,
    end: &mut usize,
) -> Option<T> {
    while let Some((range, value)) = ranges.peek() {
        if range.end <= offset
            || range.is_empty()
            || !text.is_char_boundary(range.start)
            || !text.is_char_boundary(range.end)
        {
            ranges.next();
        } else if range.start > offset {
            *end = (*end).min(range.start);
            return None;
        } else {
            *end = (*end).min(range.end);
            return Some(*value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use gpui::{Hsla, font};

    use super::*;

    fn base() -> TextRun {
        TextRun {
            len: 0,
            font: font("Mono"),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn anchored_text_keeps_its_style_and_maps_to_one_document_offset() {
        let source = "aé😀";
        let start = 40;
        let hint = |anchor, text: &'static str| InlineText {
            anchor,
            text: text.into(),
            style: TextRun {
                color: Hsla::red(),
                ..base()
            },
        };
        let inserts = [hint(40, ">"), hint(43, ": "), hint(43, "型"), hint(47, "!")];
        let line = DisplayLine::compose(
            start,
            source.into(),
            runs(source, base(), &[(1..3, Hsla::blue())], &[], None),
            &inserts,
        );
        assert_eq!(line.text.as_ref(), ">aé: 型😀!");
        let parts: Vec<_> = line
            .parts
            .iter()
            .map(|part| {
                let bytes = part.display_start..part.display_start + part.style.len;
                (&line.text[bytes], part.source(), part.style.color)
            })
            .collect();
        assert_eq!(
            parts,
            [
                (">", 40..40, Hsla::red()),
                ("a", 40..41, base().color),
                ("é", 41..43, Hsla::blue()),
                (": ", 43..43, Hsla::red()),
                ("型", 43..43, Hsla::red()),
                ("😀", 43..47, base().color),
                ("!", 47..47, Hsla::red())
            ]
        );
        for (document, display) in [(40, 0), (41, 2), (43, 4), (47, 13)] {
            assert_eq!(line.display_offset(document), display);
            assert_eq!(line.source_offset(display), document);
        }
        for display in [4, 5, 6, 9] {
            assert_eq!(line.source_offset(display), 43);
        }
        assert_eq!(line.source_offset(line.text.len()), 47);
    }

    #[test]
    fn an_insert_can_occupy_an_empty_line() {
        let insert = InlineText {
            anchor: 12,
            text: "hint".into(),
            style: base(),
        };
        let line = DisplayLine::compose(12, String::new(), vec![], &[insert]);
        assert_eq!(line.text.as_ref(), "hint");
        assert_eq!(line.display_offset(12), 0);
        for display in 0..=4 {
            assert_eq!(line.source_offset(display), 12);
        }
    }

    #[test]
    fn highlights_split_the_line_into_runs() {
        let red = Hsla::red();
        let runs = runs("0123456789", base(), &[], &[(2..5, red)], Some(&(4..6)));
        let lens: Vec<_> = runs.iter().map(|run| run.len).collect();
        assert_eq!(lens, [2, 2, 1, 1, 4]);
        assert_eq!(runs[1].background_color, Some(red));
        assert!(runs[2].underline.is_some() && runs[2].background_color.is_some());
        assert!(runs[3].underline.is_some() && runs[3].background_color.is_none());
    }

    #[test]
    fn an_empty_line_has_no_runs() {
        assert!(runs("", base(), &[], &[], None).is_empty());
    }

    #[test]
    fn unicode_foreground_word_background_and_multiline_ime_compose() {
        let source = "x\r\né😀z\r\na";
        let start = source.find('é').unwrap();
        let text = &source[start..source.find('z').unwrap() + 1];
        let syntax = local_range(text, start, &(0..start + 6)).unwrap();
        let marked = local_range(text, start, &(start + 2..source.len())).unwrap();
        let blue = Hsla::blue();
        let background = Hsla::red();
        let runs = runs(
            text,
            base(),
            &[(syntax, blue)],
            &[(2..6, background)],
            Some(&marked),
        );
        assert_eq!(
            runs.iter().map(|run| run.len).collect::<Vec<_>>(),
            [2, 4, 1]
        );
        assert_eq!(runs[1].color, blue);
        assert_eq!(runs[1].background_color, Some(background));
        assert!(runs[1].underline.is_some());
        assert_eq!(runs[2].color, base().color);
        assert!(runs[2].underline.is_some());
        assert_eq!(runs[2].background_color, None);
        assert_eq!(local_range(text, start, &(start + 1..start + 2)), None);
    }
}

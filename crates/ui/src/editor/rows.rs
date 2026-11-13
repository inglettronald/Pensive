//! The rows the editor shows in each mode: the text's lines, the removed lines between them, and
//! folds over unchanged lines.

use std::ops::Range;

use components::IconName;
use model::LineDiff;
use strum::VariantArray;

/// Unchanged lines kept around each change in Changes mode.
const CONTEXT: usize = 3;
/// Lines a fold's up or down button shows.
pub(crate) const STEP: usize = 20;

/// How much of a file the editor shows, and whether it marks the diff.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, VariantArray)]
pub enum Mode {
    /// The changes with a few lines around each, and the rest folded.
    #[default]
    Changes,
    /// The whole file with the diff marked.
    File,
    /// The whole file with no marks.
    Plain,
}

impl Mode {
    pub(crate) fn icon(self) -> IconName {
        match self {
            Mode::Changes => IconName::FoldVertical,
            Mode::File => IconName::FileDiff,
            Mode::Plain => IconName::FileText,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Mode::Changes => "Changes",
            Mode::File => "File",
            Mode::Plain => "Plain",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    /// A line of the text, by index. An added line can pair with the removed line it replaced,
    /// for marking the words that changed.
    Line {
        line: usize,
        added: bool,
        pair: Option<usize>,
    },
    /// A line of the committed text that's gone, by its index there.
    Removed { line: usize, pair: Option<usize> },
    /// Unchanged lines that are hidden. `down` shows them from the top, growing what's above, and
    /// `up` from the bottom, growing what's below.
    Fold {
        lines: Range<usize>,
        down: bool,
        up: bool,
    },
}

impl Row {
    /// The text line a cursor can sit on, if the row is one.
    pub(crate) fn text_line(&self) -> Option<usize> {
        match self {
            Row::Line { line, .. } => Some(*line),
            _ => None,
        }
    }

    /// Whether it has a different height from a line, which the list must measure again.
    pub(crate) fn is_fold(&self) -> bool {
        matches!(self, Row::Fold { .. })
    }
}

/// The rows for a text of `line_count` lines whose changes are `diff`. In Changes mode, lines in
/// `revealed` and the cursor's line stay unfolded too. The diff may lag the text, so its ranges
/// are clamped to it.
pub(crate) fn rows(
    diff: &LineDiff,
    line_count: usize,
    mode: Mode,
    revealed: &[Range<usize>],
    cursor: Option<usize>,
) -> Vec<Row> {
    let unchanged = |line| Row::Line {
        line,
        added: false,
        pair: None,
    };
    if mode == Mode::Plain {
        return (0..line_count).map(unchanged).collect();
    }
    let mut rows = Vec::new();
    let mut next = 0;
    let changes: Vec<_> = diff
        .changes
        .iter()
        .filter(|change| change.new.start <= line_count)
        .collect();
    for (index, change) in changes.iter().enumerate() {
        let new = change.new.start..change.new.end.min(line_count);
        let gap = next..new.start;
        let shown = Shown {
            first: index == 0,
            last: false,
            mode,
            revealed,
            cursor,
        };
        unchanged_run(&mut rows, gap, &shown);
        for (offset, line) in change.old.clone().enumerate() {
            rows.push(Row::Removed {
                line,
                pair: (offset < new.len()).then(|| new.start + offset),
            });
        }
        for (offset, line) in new.clone().enumerate() {
            rows.push(Row::Line {
                line,
                added: true,
                pair: (offset < change.old.len()).then(|| change.old.start + offset),
            });
        }
        next = new.end;
    }
    let shown = Shown {
        first: changes.is_empty(),
        last: true,
        mode,
        revealed,
        cursor,
    };
    unchanged_run(&mut rows, next..line_count, &shown);
    rows
}

/// Which unchanged lines in a run show.
struct Shown<'a> {
    /// The run starts the file, where there's no change above to keep context for.
    first: bool,
    /// The run ends the file.
    last: bool,
    mode: Mode,
    revealed: &'a [Range<usize>],
    cursor: Option<usize>,
}

/// Pushes the unchanged lines in `run`. In Changes mode, only the context next to a change,
/// revealed lines and the cursor's line show, and the rest fold.
fn unchanged_run(rows: &mut Vec<Row>, run: Range<usize>, shown: &Shown) {
    let Shown { first, last, .. } = *shown;
    let shown = |line: usize| {
        shown.mode == Mode::File
            || (!first && line < run.start + CONTEXT)
            || (!last && line + CONTEXT >= run.end)
            || shown.revealed.iter().any(|range| range.contains(&line))
            || shown.cursor == Some(line)
    };
    let mut line = run.start;
    while line < run.end {
        if shown(line) {
            rows.push(Row::Line {
                line,
                added: false,
                pair: None,
            });
            line += 1;
            continue;
        }
        let start = line;
        while line < run.end && !shown(line) {
            line += 1;
        }
        let long = line - start > STEP;
        rows.push(Row::Fold {
            lines: start..line,
            down: long && !(first && start == run.start),
            up: long && !(last && line == run.end),
        });
    }
}

#[cfg(test)]
mod tests {
    use model::ChangedLines;

    use super::*;

    /// Line 10 of 100 replaced by one line.
    fn one_change() -> LineDiff {
        LineDiff {
            changes: vec![ChangedLines {
                old: 9..10,
                new: 9..10,
            }],
        }
    }

    fn folds(rows: &[Row]) -> Vec<(Range<usize>, bool, bool)> {
        rows.iter()
            .filter_map(|row| match row {
                Row::Fold { lines, down, up } => Some((lines.clone(), *down, *up)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn plain_shows_every_line_without_marks() {
        let rows = rows(&one_change(), 100, Mode::Plain, &[], None);
        assert_eq!(rows.len(), 100);
        assert!(
            rows.iter()
                .all(|row| matches!(row, Row::Line { added: false, .. }))
        );
    }

    #[test]
    fn file_shows_every_line_and_the_removed_one() {
        let rows = rows(&one_change(), 100, Mode::File, &[], None);
        assert_eq!(rows.len(), 101);
        assert_eq!(
            rows[9],
            Row::Removed {
                line: 9,
                pair: Some(9)
            }
        );
        assert_eq!(
            rows[10],
            Row::Line {
                line: 9,
                added: true,
                pair: Some(9)
            }
        );
    }

    #[test]
    fn changes_folds_all_but_three_lines_around_each_change() {
        let rows = rows(&one_change(), 100, Mode::Changes, &[], None);
        // Above the first change nothing grows downward into the fold, and below the last
        // nothing grows up. The first fold is short anyway.
        assert_eq!(folds(&rows), [(0..6, false, false), (13..100, true, false)]);
    }

    #[test]
    fn revealed_lines_split_a_fold() {
        let revealed = std::slice::from_ref(&(13..33));
        let rows = rows(&one_change(), 100, Mode::Changes, revealed, None);
        assert_eq!(folds(&rows), [(0..6, false, false), (33..100, true, false)]);
    }

    #[test]
    fn the_cursors_line_never_folds() {
        let rows = rows(&LineDiff::default(), 100, Mode::Changes, &[], Some(50));
        assert_eq!(folds(&rows), [(0..50, false, true), (51..100, true, false)]);
    }

    #[test]
    fn a_fold_between_changes_grows_both_ways() {
        let diff = LineDiff {
            changes: vec![
                ChangedLines {
                    old: 0..1,
                    new: 0..1,
                },
                ChangedLines {
                    old: 60..61,
                    new: 60..61,
                },
            ],
        };
        let rows = rows(&diff, 100, Mode::Changes, &[], None);
        assert_eq!(folds(&rows)[0], (4..57, true, true));
    }

    #[test]
    fn a_short_fold_only_shows_all() {
        let diff = LineDiff {
            changes: vec![
                ChangedLines {
                    old: 0..1,
                    new: 0..1,
                },
                ChangedLines {
                    old: 20..21,
                    new: 20..21,
                },
            ],
        };
        let rows = rows(&diff, 30, Mode::Changes, &[], None);
        assert_eq!(folds(&rows)[0], (4..17, false, false));
    }

    #[test]
    fn a_diff_longer_than_the_text_is_clamped() {
        let diff = LineDiff {
            changes: vec![ChangedLines {
                old: 0..0,
                new: 0..50,
            }],
        };
        assert_eq!(rows(&diff, 10, Mode::File, &[], None).len(), 10);
    }
}

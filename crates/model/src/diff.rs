//! A changed file's two versions, the lines that changed between them, and the words that
//! changed on lines that pair up.

use std::ops::Range;

use imara_diff::{Algorithm, Diff, InternedInput, sources::lines};

/// How far into a file Git looks for a NUL byte before calling it binary.
const BINARY_PROBE: usize = 8000;

/// One side of a changed file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Version {
    /// The file doesn't exist on this side, such as before it was added.
    Missing,
    Binary,
    Text(String),
}

impl Version {
    /// Binary when a NUL byte is in the first 8,000 bytes, the test Git uses, or when it isn't
    /// UTF-8.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        if bytes.iter().take(BINARY_PROBE).any(|&byte| byte == 0) {
            return Self::Binary;
        }
        String::from_utf8(bytes).map_or(Self::Binary, Self::Text)
    }
}

/// A file before and after a change.
#[derive(Clone, Debug)]
pub struct FileVersions {
    pub old: Version,
    pub new: Version,
}

/// Which lines changed between two texts, as runs of old lines replaced by new ones, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineDiff {
    pub changes: Vec<ChangedLines>,
}

/// Old lines, by index, replaced by new ones. Either side can be empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedLines {
    pub old: Range<usize>,
    pub new: Range<usize>,
}

impl LineDiff {
    pub fn new(old: &str, new: &str) -> Self {
        let input = InternedInput::new(lines(old), lines(new));
        let mut diff = Diff::compute(Algorithm::Histogram, &input);
        diff.postprocess_lines(&input);
        Self {
            changes: diff
                .hunks()
                .map(|change| ChangedLines {
                    old: change.before.start as usize..change.before.end as usize,
                    new: change.after.start as usize..change.after.end as usize,
                })
                .collect(),
        }
    }
}

/// The byte range of each line of `text`, without its line ending.
pub(crate) fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut start = 0;
    lines(text)
        .map(|line| {
            let range = start..start + without_newline(line).len();
            start += line.len();
            range
        })
        .collect()
}

/// The byte ranges of the words that differ between two lines that pair up, on each side. When
/// nothing is shared, neither side gets any, since the line's own fill already says it all.
pub fn changed_words(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let old_words = words(old);
    let new_words = words(new);
    let mut input = InternedInput::default();
    input.update_before(old_words.iter().map(|word| &old[word.clone()]));
    input.update_after(new_words.iter().map(|word| &new[word.clone()]));
    let diff = Diff::compute(Algorithm::Myers, &input);
    let mut old_changed = Vec::new();
    let mut new_changed = Vec::new();
    for change in diff.hunks() {
        if let Some(bytes) = span(&old_words, &change.before) {
            old_changed.push(bytes);
        }
        if let Some(bytes) = span(&new_words, &change.after) {
            new_changed.push(bytes);
        }
    }
    let old_changed = joined(old_changed, old);
    let new_changed = joined(new_changed, new);
    let whole =
        |ranges: &[Range<usize>], line: &str| matches!(ranges, [only] if *only == (0..line.len()));
    if whole(&old_changed, old) && whole(&new_changed, new) {
        return (Vec::new(), Vec::new());
    }
    (old_changed, new_changed)
}

/// Joins ranges with only punctuation or spaces between them, so `px(28.)` reads as one change
/// rather than three.
fn joined(ranges: Vec<Range<usize>>, line: &str) -> Vec<Range<usize>> {
    let mut joined: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        match joined.last_mut() {
            Some(last)
                if !line[last.end..range.start]
                    .chars()
                    .any(|char| char.is_alphanumeric() || char == '_') =>
            {
                last.end = range.end
            }
            _ => joined.push(range),
        }
    }
    joined
}

/// Words, runs of spaces, and single punctuation marks, as byte ranges that cover the line.
fn words(line: &str) -> Vec<Range<usize>> {
    #[derive(PartialEq)]
    enum Class {
        Word,
        Space,
        Mark,
    }
    let class = |char: char| {
        if char.is_alphanumeric() || char == '_' {
            Class::Word
        } else if char.is_whitespace() {
            Class::Space
        } else {
            Class::Mark
        }
    };
    let mut words: Vec<Range<usize>> = Vec::new();
    let mut previous = None;
    for (start, char) in line.char_indices() {
        let end = start + char.len_utf8();
        let current = class(char);
        match words.last_mut() {
            Some(word) if current != Class::Mark && previous.as_ref() == Some(&current) => {
                word.end = end
            }
            _ => words.push(start..end),
        }
        previous = Some(current);
    }
    words
}

/// The bytes `tokens` cover in a line split by [`words`].
fn span(words: &[Range<usize>], tokens: &Range<u32>) -> Option<Range<usize>> {
    (!tokens.is_empty())
        .then(|| words[tokens.start as usize].start..words[tokens.end as usize - 1].end)
}

fn without_newline(line: &str) -> &str {
    line.strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(old: &str, new: &str) -> Vec<(Range<usize>, Range<usize>)> {
        LineDiff::new(old, new)
            .changes
            .into_iter()
            .map(|change| (change.old, change.new))
            .collect()
    }

    fn numbered(count: usize) -> String {
        (1..=count).map(|line| format!("line {line}\n")).collect()
    }

    #[test]
    fn a_replaced_line_is_one_change() {
        let old = numbered(20);
        let new = old.replace("line 10\n", "line ten\n");
        assert_eq!(changes(&old, &new), [(9..10, 9..10)]);
    }

    #[test]
    fn a_new_file_is_all_added_and_identical_text_has_no_changes() {
        assert_eq!(changes("", "a\nb\n"), [(0..0, 0..2)]);
        assert!(changes("same\n", "same\n").is_empty());
    }

    #[test]
    fn paired_lines_mark_only_the_words_that_changed() {
        let (old, new) = ("    .h(px(28.))", "    .h(density.row_height)");
        let (old_words, new_words) = changed_words(old, new);
        assert_eq!(&old[old_words[0].clone()], "px(28.)");
        assert_eq!(&new[new_words[0].clone()], "density.row_height");
    }

    #[test]
    fn a_line_with_nothing_in_common_marks_no_words() {
        assert_eq!(changed_words("alpha", "beta"), (Vec::new(), Vec::new()));
    }

    #[test]
    fn line_ranges_leave_out_line_endings() {
        let text = "a\r\nbc\n";
        let lines: Vec<_> = line_ranges(text)
            .into_iter()
            .map(|range| &text[range])
            .collect();
        assert_eq!(lines, ["a", "bc"]);
    }

    #[test]
    fn a_nul_byte_makes_a_version_binary() {
        assert_eq!(Version::from_bytes(b"a\0b".to_vec()), Version::Binary);
        assert_eq!(Version::from_bytes(vec![0xff]), Version::Binary);
        assert_eq!(
            Version::from_bytes(b"text".to_vec()),
            Version::Text("text".into())
        );
    }
}

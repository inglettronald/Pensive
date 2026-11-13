//! A changed file's two versions, and the hunks between them with the words that changed.

use std::ops::Range;

use imara_diff::{Algorithm, Diff, InternedInput, sources::lines};

use crate::{Hunk, HunkLine, LineChange};

/// Unchanged lines shown around each change, as many as `git diff` shows.
const CONTEXT: u32 = 3;
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

    /// The text, empty when the file is missing, or `None` when it's binary.
    fn text(&self) -> Option<&str> {
        match self {
            Self::Missing => Some(""),
            Self::Binary => None,
            Self::Text(text) => Some(text),
        }
    }
}

/// A file before and after a change.
#[derive(Clone, Debug)]
pub struct FileVersions {
    pub old: Version,
    pub new: Version,
}

/// What the main view shows for a file.
#[derive(Clone, Debug)]
pub enum FileDiff {
    Text(Vec<Hunk>),
    /// Either side is binary, so there are no lines to compare.
    Binary,
}

impl FileDiff {
    pub fn new(versions: &FileVersions) -> Self {
        match (versions.old.text(), versions.new.text()) {
            (Some(old), Some(new)) => Self::Text(hunks(old, new)),
            _ => Self::Binary,
        }
    }
}

/// The changed lines between `old` and `new`, each run with up to three unchanged lines around
/// it. Runs whose context would meet share one hunk, as in `git diff`.
fn hunks(old: &str, new: &str) -> Vec<Hunk> {
    let input = InternedInput::new(lines(old), lines(new));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let old: Vec<&str> = lines(old).map(without_newline).collect();
    let new: Vec<&str> = lines(new).map(without_newline).collect();

    let mut groups: Vec<Vec<imara_diff::Hunk>> = Vec::new();
    for change in diff.hunks() {
        match groups.last_mut() {
            Some(group)
                if change.before.start - group.last().expect("groups aren't empty").before.end
                    <= 2 * CONTEXT =>
            {
                group.push(change)
            }
            _ => groups.push(vec![change]),
        }
    }
    groups.iter().map(|group| hunk(group, &old, &new)).collect()
}

/// One hunk from changes that lie within each other's context.
fn hunk(group: &[imara_diff::Hunk], old: &[&str], new: &[&str]) -> Hunk {
    let (first, last) = (&group[0], &group[group.len() - 1]);
    let lead = CONTEXT.min(first.before.start).min(first.after.start);
    let trail = CONTEXT
        .min(old.len() as u32 - last.before.end)
        .min(new.len() as u32 - last.after.end);
    let old_range = first.before.start - lead..last.before.end + trail;
    let new_range = first.after.start - lead..last.after.end + trail;

    let mut lines = Vec::new();
    let context = |lines: &mut Vec<HunkLine>, from: u32, to: u32| {
        lines.extend((from..to).map(|index| HunkLine {
            change: LineChange::Context(index as usize + 1),
            text: new[index as usize].to_string().into(),
            changed: Vec::new(),
        }));
    };
    let mut position = new_range.start;
    for change in group {
        context(&mut lines, position, change.after.start);
        let removed = &old[range(&change.before)];
        let added = &new[range(&change.after)];
        let mut words: Vec<_> = removed
            .iter()
            .zip(added)
            .map(|(removed, added)| changed_words(removed, added))
            .collect();
        lines.extend(removed.iter().enumerate().map(|(index, text)| {
            HunkLine {
                change: LineChange::Removed,
                text: text.to_string().into(),
                changed: words
                    .get(index)
                    .map(|(old, _)| old.clone())
                    .unwrap_or_default(),
            }
        }));
        lines.extend(added.iter().enumerate().map(|(index, text)| {
            HunkLine {
                change: LineChange::Added(change.after.start as usize + index + 1),
                text: text.to_string().into(),
                changed: words
                    .get_mut(index)
                    .map(|(_, new)| std::mem::take(new))
                    .unwrap_or_default(),
            }
        }));
        position = change.after.end;
    }
    context(&mut lines, position, new_range.end);

    Hunk {
        header: format!(
            "@@ -{} +{} @@",
            header_range(&old_range),
            header_range(&new_range)
        )
        .into(),
        lines,
    }
}

/// The byte ranges of the words that differ between two lines that pair up, on each side. When
/// nothing is shared, neither side gets any, since the line's own fill already says it all.
fn changed_words(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
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

fn range(tokens: &Range<u32>) -> Range<usize> {
    tokens.start as usize..tokens.end as usize
}

fn without_newline(line: &str) -> &str {
    line.strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .unwrap_or(line)
}

/// A side of a hunk header the way Git writes it: an empty range names the line before it.
fn header_range(lines: &Range<u32>) -> String {
    if lines.is_empty() {
        format!("{},0", lines.start)
    } else {
        format!("{},{}", lines.start + 1, lines.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff(old: &str, new: &str) -> Vec<Hunk> {
        hunks(old, new)
    }

    fn marks(hunk: &Hunk) -> String {
        hunk.lines
            .iter()
            .map(|line| match line.change {
                LineChange::Context(_) => ' ',
                LineChange::Added(_) => '+',
                LineChange::Removed => '-',
            })
            .collect()
    }

    fn numbered(count: usize) -> String {
        (1..=count).map(|line| format!("line {line}\n")).collect()
    }

    #[test]
    fn a_change_gets_three_lines_of_context_each_side() {
        let old = numbered(20);
        let new = old.replace("line 10\n", "line ten\n");
        let hunks = diff(&old, &new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].header, "@@ -7,7 +7,7 @@");
        assert_eq!(marks(&hunks[0]), "   -+   ");
        assert_eq!(hunks[0].lines[0].change, LineChange::Context(7));
        assert_eq!(hunks[0].lines[4].change, LineChange::Added(10));
    }

    #[test]
    fn nearby_changes_share_a_hunk_and_distant_ones_dont() {
        let old = numbered(40);
        let near = old.replace("line 10\n", "x\n").replace("line 17\n", "y\n");
        assert_eq!(diff(&old, &near).len(), 1);
        let far = old.replace("line 10\n", "x\n").replace("line 18\n", "y\n");
        assert_eq!(diff(&old, &far).len(), 2);
    }

    #[test]
    fn a_new_file_is_one_hunk_of_added_lines() {
        let hunks = diff("", "a\nb\n");
        assert_eq!(hunks[0].header, "@@ -0,0 +1,2 @@");
        assert_eq!(marks(&hunks[0]), "++");
    }

    #[test]
    fn identical_text_has_no_hunks() {
        assert!(diff("same\n", "same\n").is_empty());
    }

    #[test]
    fn paired_lines_mark_only_the_words_that_changed() {
        let hunks = diff("    .h(px(28.))\n", "    .h(density.row_height)\n");
        let lines = &hunks[0].lines;
        assert_eq!(&lines[0].text[lines[0].changed[0].clone()], "px(28.)");
        assert_eq!(
            &lines[1].text[lines[1].changed[0].clone()],
            "density.row_height"
        );
    }

    #[test]
    fn a_line_with_nothing_in_common_marks_no_words() {
        let hunks = diff("alpha\n", "beta\n");
        assert!(hunks[0].lines.iter().all(|line| line.changed.is_empty()));
    }

    #[test]
    fn crlf_endings_stay_out_of_the_text() {
        let hunks = diff("a\r\n", "b\r\n");
        assert_eq!(hunks[0].lines[0].text, "a");
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

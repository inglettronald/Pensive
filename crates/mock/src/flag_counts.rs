//! How many layout flags each mock has per problem and matrix cell, as text and compared with
//! another count. `cargo mocks --matrix --check` keeps the flags people accepted this way.

use std::collections::BTreeMap;

use crate::flags::Problem;

const HEADER: &str = "\
# The layout flags `cargo mocks --matrix --check` accepts: problem, mock, cell, count.
# Update it with `cargo mocks --matrix --check --accept`.
";

/// Sorts worst problem first, then by mock, so a mock's cells stay together.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FlagKey {
    pub problem: Problem,
    pub mock: String,
    pub cell: String,
}

impl FlagKey {
    /// `<cell>/<mock>`, which is what the matrix's filters match.
    pub fn id(&self) -> String {
        format!("{}/{}", self.cell, self.mock)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlagCounts(BTreeMap<FlagKey, usize>);

/// A key with more flags than were accepted for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Excess {
    pub key: FlagKey,
    pub found: usize,
    pub accepted: usize,
}

#[derive(Debug, Default, PartialEq)]
pub struct Comparison {
    /// Worst problem first.
    pub new: Vec<Excess>,
    /// How many accepted flags no longer appear.
    pub gone: usize,
}

impl FlagCounts {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut counts = Self::default();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, count) =
                parse_line(line).ok_or_else(|| format!("line {}: `{line}`", index + 1))?;
            counts.0.insert(key, count);
        }
        Ok(counts)
    }

    pub fn to_text(&self) -> String {
        let mut text = HEADER.to_string();
        for (key, count) in &self.0 {
            let problem = key.problem.id();
            text += &format!("{problem} {} {} {count}\n", key.mock, key.cell);
        }
        text
    }

    pub fn add(&mut self, key: FlagKey) {
        *self.0.entry(key).or_default() += 1;
    }

    pub fn total(&self) -> usize {
        self.0.values().sum()
    }

    fn count(&self, key: &FlagKey) -> usize {
        self.0.get(key).copied().unwrap_or(0)
    }

    /// How `found` differs from these accepted counts, among the keys `matches` picks.
    pub fn compare(&self, found: &Self, matches: impl Fn(&FlagKey) -> bool) -> Comparison {
        let new = found
            .0
            .iter()
            .filter(|(key, _)| matches(key))
            .filter_map(|(key, &count)| {
                let accepted = self.count(key);
                (count > accepted).then(|| Excess {
                    key: key.clone(),
                    found: count,
                    accepted,
                })
            })
            .collect();
        let gone = self
            .0
            .iter()
            .filter(|(key, _)| matches(key))
            .map(|(key, &count)| count.saturating_sub(found.count(key)))
            .sum();
        Comparison { new, gone }
    }

    /// Takes `found`'s counts for the keys `matches` picks, and keeps the rest.
    pub fn accept(&mut self, found: &Self, matches: impl Fn(&FlagKey) -> bool) {
        self.0.retain(|key, _| !matches(key));
        let accepted = found.0.iter().filter(|(key, _)| matches(key));
        self.0
            .extend(accepted.map(|(key, &count)| (key.clone(), count)));
    }
}

fn parse_line(line: &str) -> Option<(FlagKey, usize)> {
    let mut words = line.split_whitespace();
    let problem = Problem::from_id(words.next()?)?;
    let mock = words.next()?.to_string();
    let cell = words.next()?.to_string();
    let count = words.next()?.parse().ok().filter(|&count| count > 0)?;
    words.next().is_none().then_some((
        FlagKey {
            problem,
            mock,
            cell,
        },
        count,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(problem: Problem, mock: &str, cell: &str) -> FlagKey {
        FlagKey {
            problem,
            mock: mock.to_string(),
            cell: cell.to_string(),
        }
    }

    fn counts(keys: &[(FlagKey, usize)]) -> FlagCounts {
        let mut counts = FlagCounts::default();
        for (key, count) in keys {
            for _ in 0..*count {
                counts.add(key.clone());
            }
        }
        counts
    }

    #[test]
    fn counts_survive_being_written_and_read() {
        let counts = counts(&[
            (
                key(Problem::CutOff, "root/project", "720x480/zoom80-calm"),
                2,
            ),
            (
                key(Problem::Overlap, "notices/folded", "960x540/zoom200-dense"),
                1,
            ),
        ]);
        assert_eq!(FlagCounts::parse(&counts.to_text()), Ok(counts));
    }

    #[test]
    fn the_worst_problem_is_written_first() {
        let text = counts(&[
            (key(Problem::NeedlessFade, "a/b", "c"), 1),
            (key(Problem::LettersStacked, "z/z", "z"), 1),
        ])
        .to_text();
        let first = text.lines().find(|line| !line.starts_with('#'));
        assert_eq!(first, Some("letters-stacked z/z z 1"));
    }

    #[test]
    fn a_line_that_cannot_be_read_is_an_error_naming_it() {
        let error = FlagCounts::parse("# note\n\nno-such-problem a/b c 1\n").unwrap_err();
        assert!(error.starts_with("line 3:"), "{error}");
        assert!(FlagCounts::parse("cut-off a/b c 0").is_err());
        assert!(FlagCounts::parse("cut-off a/b c 1 extra").is_err());
    }

    #[test]
    fn only_flags_past_the_accepted_count_are_new() {
        let same = key(Problem::CutOff, "a/same", "c");
        let more = key(Problem::CutOff, "a/more", "c");
        let unseen = key(Problem::Overlap, "a/unseen", "c");
        let accepted = counts(&[(same.clone(), 2), (more.clone(), 1)]);
        let found = counts(&[(same, 2), (more.clone(), 3), (unseen.clone(), 1)]);
        let comparison = accepted.compare(&found, |_| true);
        assert_eq!(
            comparison.new,
            [
                Excess {
                    key: unseen,
                    found: 1,
                    accepted: 0,
                },
                Excess {
                    key: more,
                    found: 3,
                    accepted: 1,
                },
            ]
        );
        assert_eq!(comparison.gone, 0);
    }

    #[test]
    fn fixed_flags_are_counted_as_gone() {
        let fixed = key(Problem::CutOff, "a/fixed", "c");
        let fewer = key(Problem::CutOff, "a/fewer", "c");
        let accepted = counts(&[(fixed, 1), (fewer.clone(), 3)]);
        let comparison = accepted.compare(&counts(&[(fewer, 1)]), |_| true);
        assert_eq!(
            comparison,
            Comparison {
                new: Vec::new(),
                gone: 3,
            }
        );
    }

    #[test]
    fn a_filter_limits_what_is_compared_and_accepted() {
        let picked = key(Problem::CutOff, "a/picked", "c");
        let other = key(Problem::CutOff, "a/other", "c");
        let is_picked = |key: &FlagKey| key.id().contains("picked");

        let mut accepted = counts(&[(other.clone(), 1)]);
        let found = counts(&[(picked.clone(), 2)]);
        let comparison = accepted.compare(&found, is_picked);
        assert_eq!(comparison.new.len(), 1);
        assert_eq!(comparison.gone, 0);

        accepted.accept(&found, is_picked);
        assert_eq!(accepted, counts(&[(other, 1), (picked, 2)]));
    }
}

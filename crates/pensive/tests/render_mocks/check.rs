//! `cargo mocks --matrix --check`: compares what the matrix flagged with the accepted list, and
//! prints only what's new, worst first.

use std::{cmp::Reverse, io::ErrorKind};

use anyhow::anyhow;
use mock::flag_counts::{Excess, FlagCounts, FlagKey};

use crate::{
    Args,
    report::{Report, rect_name},
};

const ACCEPTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/render_mocks/accepted_flags.txt"
);
/// How much of each new problem to print, since one break can reach every window size.
const CELLS_SHOWN: usize = 3;
const PLACES_SHOWN: usize = 3;

/// Returns whether the check passed.
pub(crate) fn run(report: &Report, args: &Args, any_failed: bool) -> gpui::Result<bool> {
    let mut accepted = load()?;
    let found = report.counts();
    let matches = |key: &FlagKey| args.matches_filters(&key.id());
    if args.accept {
        if any_failed {
            eprintln!("nothing accepted, because some mocks failed to render");
            return Ok(false);
        }
        accepted.accept(&found, matches);
        std::fs::write(ACCEPTED, accepted.to_text())?;
        println!("{} flags accepted in {ACCEPTED}", accepted.total());
        return Ok(true);
    }

    let comparison = accepted.compare(&found, matches);
    print_new(report, &comparison.new);
    let new: usize = comparison
        .new
        .iter()
        .map(|excess| excess.found - excess.accepted)
        .sum();
    println!("{} renders, {new} new flags", report.rendered());
    if comparison.gone > 0 {
        println!(
            "{} accepted flags are gone, so `--accept` can drop them",
            comparison.gone
        );
    }
    if new > 0 {
        let filters: String = args.filters.iter().map(|f| format!(" {f}")).collect();
        println!(
            "if these are expected, accept them with `cargo mocks --matrix --check --accept{filters}`"
        );
    }
    Ok(new == 0)
}

fn load() -> gpui::Result<FlagCounts> {
    let text = match std::fs::read_to_string(ACCEPTED) {
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        text => text?,
    };
    FlagCounts::parse(&text).map_err(|error| anyhow!("{ACCEPTED}, {error}"))
}

/// One block per problem in a mock, with the problems that reach the most windows first among
/// equally bad ones.
fn print_new(report: &Report, new: &[Excess]) {
    let mut groups: Vec<&[Excess]> = new
        .chunk_by(|a, b| a.key.problem == b.key.problem && a.key.mock == b.key.mock)
        .collect();
    groups.sort_by_key(|group| (group[0].key.problem, Reverse(group.len())));
    for group in groups {
        let FlagKey { problem, mock, .. } = &group[0].key;
        println!(
            "{} in {mock}, at {} combinations",
            problem.id(),
            group.len()
        );
        for excess in group.iter().take(CELLS_SHOWN) {
            println!("  {}", describe(report, excess));
        }
        if group.len() > CELLS_SHOWN {
            println!("  and {} more", group.len() - CELLS_SHOWN);
        }
        println!(
            "  to see it: cargo mocks --matrix {} {mock}",
            group[0].key.cell
        );
    }
}

fn describe(report: &Report, excess: &Excess) -> String {
    let flags: Vec<_> = report.flags_of(&excess.key).collect();
    let places: Vec<String> = flags
        .iter()
        .take(PLACES_SHOWN)
        .map(|flag| rect_name(&flag.at))
        .collect();
    let more = match flags.len().saturating_sub(PLACES_SHOWN) {
        0 => String::new(),
        more => format!(" and {more} more"),
    };
    format!(
        "{}: {} at {}{more}, {} found and {} accepted",
        excess.key.cell,
        flags[0].detail,
        places.join(", "),
        excess.found,
        excess.accepted
    )
}

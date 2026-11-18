//! What a matrix run found, as a table for the terminal and a full list in Markdown.

use std::{collections::BTreeMap, fmt::Write};

use mock::{
    flags::{Flag, Kind},
    matrix::{Cell, SIZES, WindowSize, cells_of},
    painted::Rect,
};

/// What the checks can't see, so a clean report isn't read as a clean layout.
const LOOK_AT_THE_PNGS: &str = "\
The checks read what each frame painted, so they catch geometry. Look at the PNGs for the rest:

- Whether a layout looks right: spacing, balance, and wide windows with lots of empty space.
- Content that scrolls out of reach, because a scrolled list is cut at its edge by design.
- Text cut between two letters. GPUI never paints the hidden letter, so nothing crosses the edge.
- A control covering text, since popups cover text on purpose. Only text over text is flagged.
- Groups that should be centered other than the palette and the landing screen.
- Whether a fade loses something the reader needs. Fades are only counted and measured.
";

#[derive(Default)]
pub(crate) struct Report {
    rendered: Vec<Rendered>,
}

struct Rendered {
    cell: Cell,
    mock: String,
    flags: Vec<Flag>,
}

impl Report {
    pub(crate) fn add(&mut self, cell: Cell, mock: String, flags: Vec<Flag>) {
        self.rendered.push(Rendered { cell, mock, flags });
    }

    pub(crate) fn rendered(&self) -> usize {
        self.rendered.len()
    }

    fn in_cell(&self, cell: &Cell) -> impl Iterator<Item = &Rendered> {
        let cell = *cell;
        self.rendered.iter().filter(move |item| item.cell == cell)
    }

    /// One line per window size, for the terminal.
    pub(crate) fn summary(&self) -> String {
        let flagged = self
            .rendered
            .iter()
            .filter(|item| !item.flags.is_empty())
            .count();
        let mut summary = format!("{} renders, {flagged} flagged", self.rendered.len());
        for size in SIZES {
            let (mut renders, mut flagged) = (0, 0);
            for cell in cells_of(size) {
                for item in self.in_cell(&cell) {
                    renders += 1;
                    flagged += usize::from(!item.flags.is_empty());
                }
            }
            if renders > 0 {
                let _ = write!(
                    summary,
                    "\n  {}: {flagged} of {renders} flagged",
                    size_name(&size)
                );
            }
        }
        summary
    }

    pub(crate) fn markdown(&self) -> String {
        let mut out = String::from("# Mock matrix\n\n");
        let _ = writeln!(out, "{}\n", self.summary().replace("\n  ", "\n- "));
        out.push_str("Coordinates are window pixels. The PNGs are at twice that.\n\n");
        out.push_str("## What this doesn't check\n\n");
        out.push_str(LOOK_AT_THE_PNGS);
        out.push_str("\n## Flagged mocks per cell\n\n");
        self.write_table(&mut out);
        out.push_str("\n## Flags\n");
        for size in SIZES {
            for cell in cells_of(size) {
                self.write_cell(&cell, &mut out);
            }
        }
        out
    }

    fn write_table(&self, out: &mut String) {
        let header = cells_of(SIZES[0])
            .map(|cell| {
                cell.id()
                    .split_once('/')
                    .map_or(String::new(), |id| id.1.to_string())
            })
            .collect::<Vec<_>>();
        let _ = writeln!(out, "| Size | Setup | {} |", header.join(" | "));
        let _ = writeln!(out, "|---|---|{}", "---|".repeat(header.len()));
        for size in SIZES {
            let cells: Vec<String> = cells_of(size).map(|cell| self.cell_counts(&cell)).collect();
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                size_name(&size),
                size.setup,
                cells.join(" | ")
            );
        }
    }

    /// `3 of 39: 2 clipped, 1 overlap`, counting the mocks with each kind of flag.
    fn cell_counts(&self, cell: &Cell) -> String {
        let items: Vec<&Rendered> = self.in_cell(cell).collect();
        if items.is_empty() {
            return "not rendered".to_string();
        }
        let flagged = items.iter().filter(|item| !item.flags.is_empty()).count();
        let kinds: Vec<String> = Kind::ALL
            .into_iter()
            .filter_map(|kind| {
                let count = items
                    .iter()
                    .filter(|item| item.flags.iter().any(|flag| flag.kind == kind))
                    .count();
                (count > 0).then(|| format!("{count} {}", kind.name()))
            })
            .collect();
        match kinds.is_empty() {
            true => format!("0 of {}", items.len()),
            false => format!("{flagged} of {}: {}", items.len(), kinds.join(", ")),
        }
    }

    /// Lists a cell's flags, with the mocks that share each one on one line.
    fn write_cell(&self, cell: &Cell, out: &mut String) {
        let mut mocks_by_flag: BTreeMap<(Kind, String), Vec<&str>> = BTreeMap::new();
        for item in self.in_cell(cell) {
            for flag in &item.flags {
                let place = format!("{} at {}", flag.detail, rect_name(&flag.at));
                let mocks = mocks_by_flag.entry((flag.kind, place)).or_default();
                if mocks.last() != Some(&item.mock.as_str()) {
                    mocks.push(&item.mock);
                }
            }
        }
        if mocks_by_flag.is_empty() {
            return;
        }
        let _ = writeln!(out, "\n### {}\n", cell.id());
        for ((kind, place), mocks) in mocks_by_flag {
            let _ = writeln!(out, "- {}: {place}. {}", kind.name(), mocks.join(", "));
        }
    }
}

fn size_name(size: &WindowSize) -> String {
    format!("{}x{}", size.width, size.height)
}

fn rect_name(rect: &Rect) -> String {
    format!(
        "({:.0}, {:.0}) {:.0}x{:.0}",
        rect.left,
        rect.top,
        rect.width(),
        rect.height()
    )
}

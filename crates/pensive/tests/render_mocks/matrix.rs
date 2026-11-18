//! `cargo mocks --matrix [FILTER...]` renders each window mock at every window size, zoom and
//! density to `target/mocks-matrix/<size>/<zoom>-<density>/<subject>/<mock>.png`, with an atlas
//! of each subject beside them. It checks each frame for layout problems, outlines what it finds
//! in a copy under `flagged`, and lists it all in `report.md`. Every filter must appear in
//! `<size>/<zoom>-<density>/<subject>/<mock>`.
//!
//! A flag is reported, never a failure, as a changed mock is. Only a render that fails fails the
//! run.
//!
//! `--check` is the same run for a validation loop. It writes no files, and fails on any flag
//! that isn't in the accepted list. See [`check`](crate::check).

use std::{collections::HashMap, path::Path, process::ExitCode};

use image::RgbaImage;
use mock::{
    Atlas, Capture, Mock, Output,
    flags::{self, Expected},
    matrix::{Cell, SIZES, cells_of},
};

use crate::{Args, catching_panics, check, remove, report::Report, save, write_atlas};

pub(crate) fn run(args: &Args, folder: &Path) -> gpui::Result<ExitCode> {
    let mocks: Vec<Mock> = ui::every_mock()
        .into_iter()
        .filter(|mock| mock.in_matrix)
        .collect();
    let mut run = Run {
        folder,
        checking: args.check,
        reference_fades: HashMap::new(),
        report: Report::default(),
        failed: Vec::new(),
    };
    for cell in SIZES.into_iter().flat_map(cells_of) {
        let before = run.report.rendered();
        for subject_mocks in mocks.chunk_by(|a, b| a.subject == b.subject) {
            let matching: Vec<&Mock> = subject_mocks
                .iter()
                .filter(|mock| matches(args, &cell, mock))
                .collect();
            if !matching.is_empty() && !run.render_subject(&cell, subject_mocks, &matching)? {
                println!("skipped: this platform has no headless renderer");
                return Ok(ExitCode::SUCCESS);
            }
        }
        if !args.check && run.report.rendered() > before {
            println!("{}", folder.join(cell.id()).display());
        }
    }
    run.finish(args)
}

fn matches(args: &Args, cell: &Cell, mock: &Mock) -> bool {
    args.matches_filters(&format!("{}/{}", cell.id(), mock.id()))
}

struct Run<'a> {
    folder: &'a Path,
    /// Whether only the flags matter, so nothing is drawn or written.
    checking: bool,
    /// The widths of the boxes that fade at the reference size, by zoom, density and mock.
    reference_fades: HashMap<String, Vec<f32>>,
    report: Report,
    failed: Vec<String>,
}

impl Run<'_> {
    /// Renders `matching`, which are some or all of one subject's mocks, in `cell`. Returns false
    /// when this platform can't render.
    fn render_subject(
        &mut self,
        cell: &Cell,
        subject_mocks: &[Mock],
        matching: &[&Mock],
    ) -> gpui::Result<bool> {
        let folder = self.folder.join(cell.id());
        let subject = matching[0].subject;
        let whole_subject = subject_mocks.len() > 1 && matching.len() == subject_mocks.len();
        let mut atlas = (whole_subject && !self.checking).then(|| Atlas::new(subject));
        for mock in matching {
            let image = match self.render(cell, mock) {
                Ok(Some(image)) => image,
                Ok(None) => return Ok(false),
                Err(error) => {
                    let id = format!("{}/{}", cell.id(), mock.id());
                    eprintln!("FAILED {id}: {error}");
                    self.failed.push(id);
                    atlas = None;
                    continue;
                }
            };
            if let Some(atlas) = &mut atlas {
                let sized = (*mock)
                    .clone()
                    .size(cell.size.width as f32, cell.size.height as f32);
                atlas.push(&sized, image);
            }
        }
        if !self.checking {
            write_atlas(atlas, &folder, subject)?;
        }
        Ok(true)
    }

    /// Renders `mock` in `cell`, notes what's flagged in it and returns its image. Unless this run
    /// is only checking, it also saves the image and a copy with the flags outlined.
    fn render(&mut self, cell: &Cell, mock: &Mock) -> gpui::Result<Option<RgbaImage>> {
        let output = match self.checking {
            true => Output::PaintedOnly,
            false => Output::Image,
        };
        let Some(Capture { image, painted }) = capture(cell, mock, output)? else {
            return Ok(None);
        };
        let Some(reference_fades) = self.reference_fades(cell, mock)? else {
            return Ok(None);
        };
        let found = flags::check(
            &painted,
            Expected {
                zoom: cell.zoom,
                centered: mock.centered,
                reference_fades: &reference_fades,
            },
        );
        if !self.checking {
            let file = format!("{}/{}.png", cell.id(), mock.id());
            save(&image, &self.folder.join(&file))?;
            let flagged = self.folder.join("flagged").join(&file);
            match found.is_empty() {
                true => remove(&flagged)?,
                false => save(&flags::marked(&image, &found), &flagged)?,
            }
        }
        self.report.add(*cell, mock.id(), found);
        Ok(Some(image))
    }

    fn reference_fades(&mut self, cell: &Cell, mock: &Mock) -> gpui::Result<Option<Vec<f32>>> {
        let reference = cell.reference();
        let key = format!("{}/{}", reference.id(), mock.id());
        if let Some(fades) = self.reference_fades.get(&key) {
            return Ok(Some(fades.clone()));
        }
        let Some(capture) = capture(&reference, mock, Output::PaintedOnly)? else {
            return Ok(None);
        };
        let fades = flags::fade_widths(&capture.painted, cell.zoom);
        self.reference_fades.insert(key, fades.clone());
        Ok(Some(fades))
    }

    fn finish(self, args: &Args) -> gpui::Result<ExitCode> {
        if self.report.rendered() == 0 && self.failed.is_empty() {
            eprintln!("no mock matches `{}`", args.filters.join(" "));
            return Ok(if args.strict {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            });
        }
        let passed = match args.check {
            true => check::run(&self.report, args, !self.failed.is_empty())?,
            false => {
                let path = self.folder.join("report.md");
                std::fs::write(&path, self.report.markdown())?;
                println!("{}", self.report.summary());
                println!("{}", path.display());
                true
            }
        };
        if !self.failed.is_empty() {
            eprintln!("{} failed: {}", self.failed.len(), self.failed.join(", "));
            return Ok(ExitCode::FAILURE);
        }
        Ok(match passed {
            true => ExitCode::SUCCESS,
            false => ExitCode::FAILURE,
        })
    }
}

fn capture(cell: &Cell, mock: &Mock, output: Output) -> gpui::Result<Option<Capture>> {
    catching_panics(|| mock.capture_in(cell, output))
}

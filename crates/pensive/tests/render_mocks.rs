//! Renders every mock to `target/mocks/<subject>/<mock>.png`, and each subject with more than one
//! mock to an atlas at `target/mocks/<subject>.png`. A filter renders only the mocks whose
//! `<subject>/<mock>` contains it. The atlas is only written when the filter matches every mock of
//! its subject, and an old one is deleted when it matches only some.
//!
//! `cargo mocks <filter>` passes the filter with `--only`, and fails when nothing matches it.
//! `cargo test <filter>` passes it bare to every test binary, so here, as in any other test
//! binary, a filter that matches nothing renders nothing and passes.
//!
//! A mock that fails or panics is reported by id, and the rest still render.

use std::{
    any::Any,
    io::ErrorKind,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    process::ExitCode,
};

use anyhow::anyhow;
use mock::{Atlas, Mock};

/// libtest's options that take a value, so a value such as the `1` in `--test-threads 1` isn't
/// read as a filter.
const VALUE_OPTIONS: &[&str] = &[
    "--test-threads",
    "--skip",
    "--format",
    "--color",
    "--logfile",
    "--shuffle-seed",
    "-Z",
];

struct Filter {
    text: String,
    /// Whether matching nothing is a failure, as it is for `cargo mocks`.
    strict: bool,
}

impl Filter {
    fn from_args(mut args: impl Iterator<Item = String>) -> Self {
        let mut bare = None;
        while let Some(arg) = args.next() {
            if arg == "--only" {
                return Self {
                    text: args.next().unwrap_or_default(),
                    strict: true,
                };
            }
            if VALUE_OPTIONS.contains(&arg.as_str()) {
                args.next();
            } else if !arg.starts_with('-') && bare.is_none() {
                bare = Some(arg);
            }
        }
        Self {
            text: bare.unwrap_or_default(),
            strict: false,
        }
    }
}

fn main() -> gpui::Result<ExitCode> {
    let filter = Filter::from_args(std::env::args().skip(1));
    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("the target tmp dir is inside the target dir")
        .join("mocks");

    let mocks = ui::every_mock();
    let mut rendered = 0;
    let mut failed = Vec::new();
    for subject_mocks in mocks.chunk_by(|a, b| a.subject == b.subject) {
        let matching: Vec<&Mock> = subject_mocks
            .iter()
            .filter(|mock| mock.id().contains(&filter.text))
            .collect();
        let Some(first) = matching.first() else {
            continue;
        };
        let subject = first.subject;
        let mut atlas = (subject_mocks.len() > 1 && matching.len() == subject_mocks.len())
            .then(|| Atlas::new(subject));

        for mock in matching {
            let image = match catching_panics(|| mock.render()) {
                Ok(Some(image)) => image,
                Ok(None) => {
                    println!("skipped: this platform has no headless renderer");
                    return Ok(ExitCode::SUCCESS);
                }
                Err(error) => {
                    eprintln!("FAILED {}: {error}", mock.id());
                    failed.push(mock.id());
                    atlas = None;
                    continue;
                }
            };
            let path = out_dir.join(format!("{}.png", mock.id()));
            std::fs::create_dir_all(path.parent().expect("the path has a view directory"))?;
            image.save(&path)?;
            println!("{}", path.display());
            rendered += 1;
            if let Some(atlas) = &mut atlas {
                atlas.push(mock, image);
            }
        }

        let atlas_path = out_dir.join(format!("{subject}.png"));
        match atlas {
            Some(atlas) => {
                let image = atlas.render()?.expect("the mocks above rendered headless");
                image.save(&atlas_path)?;
                println!("{}", atlas_path.display());
            }
            None => match std::fs::remove_file(&atlas_path) {
                Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.into()),
                _ => {}
            },
        }
    }

    if rendered == 0 && failed.is_empty() {
        eprintln!("no mock matches `{}`", filter.text);
        return Ok(if filter.strict {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        });
    }
    if failed.is_empty() {
        println!("{rendered} mocks rendered");
        return Ok(ExitCode::SUCCESS);
    }
    eprintln!(
        "{rendered} mocks rendered, {} failed: {}",
        failed.len(),
        failed.join(", ")
    );
    Ok(ExitCode::FAILURE)
}

/// Runs `f`, turning a panic into an error so the mocks after it still render.
fn catching_panics<T>(f: impl FnOnce() -> gpui::Result<T>) -> gpui::Result<T> {
    catch_unwind(AssertUnwindSafe(f))
        .unwrap_or_else(|panic| Err(anyhow!("panicked: {}", panic_message(&*panic))))
}

fn panic_message(panic: &(dyn Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("no message")
}

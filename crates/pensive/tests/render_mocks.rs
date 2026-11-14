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
//!
//! `--save-baseline` also copies each render to `target/mocks-baseline`. While that folder
//! exists, every later render is compared with its copy there: each one that changed is listed,
//! with a picture of what changed in `target/mocks-diff`. A change is reported, never a failure,
//! since changing how a view looks is the point of most UI work.

use std::{
    any::Any,
    io::ErrorKind,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::anyhow;
use image::RgbaImage;
use mock::{Atlas, Mock, diff::diff};

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

struct Args {
    filter: String,
    /// Whether matching nothing is a failure, as it is for `cargo mocks`.
    strict: bool,
    save_baseline: bool,
}

impl Args {
    fn parse(mut args: impl Iterator<Item = String>) -> Self {
        let mut parsed = Self {
            filter: String::new(),
            strict: false,
            save_baseline: false,
        };
        let mut filter = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--only" => parsed.strict = true,
                "--save-baseline" => parsed.save_baseline = true,
                option if VALUE_OPTIONS.contains(&option) => {
                    args.next();
                }
                _ if !arg.starts_with('-') && filter.is_none() => filter = Some(arg),
                _ => {}
            }
        }
        parsed.filter = filter.unwrap_or_default();
        parsed
    }
}

/// Where renders go, and the copies they're compared with.
struct Folders {
    mocks: PathBuf,
    baseline: PathBuf,
    diff: PathBuf,
}

impl Folders {
    fn new() -> Self {
        let target = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .parent()
            .expect("the target tmp dir is inside the target dir");
        Self {
            mocks: target.join("mocks"),
            baseline: target.join("mocks-baseline"),
            diff: target.join("mocks-diff"),
        }
    }
}

/// How one render compares with its baseline.
enum Comparison {
    Same,
    Changed(String),
    New,
}

fn main() -> gpui::Result<ExitCode> {
    let args = Args::parse(std::env::args().skip(1));
    let folders = Folders::new();
    let comparing = !args.save_baseline && folders.baseline.is_dir();

    let mocks = ui::every_mock();
    let mut rendered = 0;
    let mut failed = Vec::new();
    let mut changed = 0;
    for subject_mocks in mocks.chunk_by(|a, b| a.subject == b.subject) {
        let matching: Vec<&Mock> = subject_mocks
            .iter()
            .filter(|mock| mock.id().contains(&args.filter))
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
            let file = format!("{}.png", mock.id());
            let path = folders.mocks.join(&file);
            save(&image, &path)?;
            println!("{}", path.display());
            rendered += 1;
            if args.save_baseline {
                save(&image, &folders.baseline.join(&file))?;
            } else if comparing {
                match compare(&image, &file, &folders)? {
                    Comparison::Same => {}
                    Comparison::Changed(summary) => {
                        changed += 1;
                        println!("changed {}: {summary}", mock.id());
                    }
                    Comparison::New => {
                        changed += 1;
                        println!("new {}: no baseline to compare with", mock.id());
                    }
                }
            }
            if let Some(atlas) = &mut atlas {
                atlas.push(mock, image);
            }
        }

        let atlas_path = folders.mocks.join(format!("{subject}.png"));
        match atlas {
            Some(atlas) => {
                let image = atlas.render()?.expect("the mocks above rendered headless");
                save(&image, &atlas_path)?;
                println!("{}", atlas_path.display());
            }
            None => remove(&atlas_path)?,
        }
    }

    if rendered == 0 && failed.is_empty() {
        eprintln!("no mock matches `{}`", args.filter);
        return Ok(if args.strict {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        });
    }
    let mut summary = format!("{rendered} mocks rendered");
    if args.save_baseline {
        summary += &format!(", saved as the baseline in {}", folders.baseline.display());
    } else if comparing {
        summary += &format!(", {changed} changed since the baseline");
    }
    if failed.is_empty() {
        println!("{summary}");
        return Ok(ExitCode::SUCCESS);
    }
    eprintln!("{summary}, {} failed: {}", failed.len(), failed.join(", "));
    Ok(ExitCode::FAILURE)
}

/// Compares the render saved as `file` with its baseline, and writes a picture of what changed to
/// the diff folder, or removes an old one.
fn compare(image: &RgbaImage, file: &str, folders: &Folders) -> gpui::Result<Comparison> {
    let diff_path = folders.diff.join(file);
    let baseline = match image::open(folders.baseline.join(file)) {
        Ok(baseline) => baseline.into_rgba8(),
        Err(image::ImageError::IoError(error)) if error.kind() == ErrorKind::NotFound => {
            remove(&diff_path)?;
            return Ok(Comparison::New);
        }
        Err(error) => return Err(error.into()),
    };
    let Some(diff) = diff(&baseline, image) else {
        remove(&diff_path)?;
        return Ok(Comparison::Same);
    };
    save(&diff.image, &diff_path)?;
    let (x, y, width, height) = diff.bounds;
    Ok(Comparison::Changed(format!(
        "{} px in {width}×{height} at ({x}, {y}), see {}",
        diff.changed,
        diff_path.display()
    )))
}

fn save(image: &RgbaImage, path: &Path) -> gpui::Result<()> {
    std::fs::create_dir_all(path.parent().expect("the path has a folder"))?;
    image.save(path)?;
    Ok(())
}

fn remove(path: &Path) -> gpui::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
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

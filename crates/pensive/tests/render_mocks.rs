//! Renders every mock to `target/mocks/<subject>/<mock>.png`, and each subject with more than one
//! mock to an atlas at `target/mocks/<subject>.png`. Pass a filter to render only the mocks whose
//! `<subject>/<mock>` contains it. The atlas is only written when the filter matches every mock of
//! its subject, and an old one is deleted when it matches only some.

use std::{io::ErrorKind, path::Path, process::ExitCode};

use mock::{Atlas, Mock};

fn main() -> gpui::Result<ExitCode> {
    let filter = std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .unwrap_or_default();
    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("the target tmp dir is inside the target dir")
        .join("mocks");

    let mocks = ui::every_mock();
    let mut matched = false;
    for subject_mocks in mocks.chunk_by(|a, b| a.subject == b.subject) {
        let matching: Vec<&Mock> = subject_mocks
            .iter()
            .filter(|mock| mock.id().contains(&filter))
            .collect();
        let Some(first) = matching.first() else {
            continue;
        };
        matched = true;
        let subject = first.subject;
        let mut atlas = (subject_mocks.len() > 1 && matching.len() == subject_mocks.len())
            .then(|| Atlas::new(subject));

        for mock in matching {
            let Some(image) = mock.render()? else {
                println!("skipped: this platform has no headless renderer");
                return Ok(ExitCode::SUCCESS);
            };
            let path = out_dir.join(format!("{}.png", mock.id()));
            std::fs::create_dir_all(path.parent().expect("the path has a view directory"))?;
            image.save(&path)?;
            println!("{}", path.display());
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

    if !matched {
        eprintln!("no mock matches `{filter}`");
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

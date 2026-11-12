//! Renders every mock to `target/mocks/<subject>/<mock>.png`, and each subject with more than one
//! mock to an atlas at `target/mocks/<subject>.png`. Pass a filter to render only the mocks whose
//! `<subject>/<mock>` contains it.

use std::{path::Path, process::ExitCode};

use mock::Atlas;

fn main() -> gpui::Result<ExitCode> {
    let filter = std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .unwrap_or_default();
    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("the target tmp dir is inside the target dir")
        .join("mocks");

    let mut atlases: Vec<Atlas> = Vec::new();
    for mock in components::mocks().into_iter().chain(ui::mocks()) {
        let id = format!("{}/{}", mock.subject, mock.name);
        if !id.contains(&filter) {
            continue;
        }
        let Some(image) = mock.render()? else {
            println!("skipped: this platform has no headless renderer");
            return Ok(ExitCode::SUCCESS);
        };
        let path = out_dir.join(format!("{id}.png"));
        std::fs::create_dir_all(path.parent().expect("the path has a view directory"))?;
        image.save(&path)?;
        println!("{}", path.display());

        let atlas = match atlases
            .iter_mut()
            .find(|atlas| atlas.subject() == mock.subject)
        {
            Some(atlas) => atlas,
            None => atlases.push_mut(Atlas::new(mock.subject)),
        };
        atlas.push(&mock, &image);
    }

    if atlases.is_empty() {
        eprintln!("no mock matches `{filter}`");
        return Ok(ExitCode::FAILURE);
    }
    for atlas in atlases.into_iter().filter(|atlas| atlas.len() > 1) {
        let path = out_dir.join(format!("{}.png", atlas.subject()));
        let image = atlas.render()?.expect("the mocks above rendered headless");
        image.save(&path)?;
        println!("{}", path.display());
    }
    Ok(ExitCode::SUCCESS)
}

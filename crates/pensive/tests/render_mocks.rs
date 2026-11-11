//! Renders every mock to `target/mocks/<subject>/<mock>.png`. Pass a filter to render only the
//! mocks whose `<subject>/<mock>` contains it.

use std::{path::Path, process::ExitCode};

fn main() -> gpui::Result<ExitCode> {
    let filter = std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .unwrap_or_default();
    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("the target tmp dir is inside the target dir")
        .join("mocks");

    let mut matched = false;
    for mock in components::mocks().into_iter().chain(ui::mocks()) {
        let id = format!("{}/{}", mock.subject, mock.name);
        if !id.contains(&filter) {
            continue;
        }
        matched = true;
        let Some(image) = mock.render()? else {
            println!("skipped: this platform has no headless renderer");
            return Ok(ExitCode::SUCCESS);
        };
        let path = out_dir.join(format!("{id}.png"));
        std::fs::create_dir_all(path.parent().expect("the path has a view directory"))?;
        image.save(&path)?;
        println!("{}", path.display());
    }

    if !matched {
        eprintln!("no mock matches `{filter}`");
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

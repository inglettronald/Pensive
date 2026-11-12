mod config;

use std::{path::PathBuf, sync::Arc};

use git::GitCli;
use gpui::{App, AppContext};
use model::Projects;

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let (projects, paths) = match args.as_slice() {
        // The sample projects, for clicking around views without real repositories.
        #[cfg(debug_assertions)]
        [arg] if arg.as_os_str() == "mocked" => (model::fixtures::projects(), Vec::new()),
        // Until Pensive remembers projects, they come from the command line.
        _ => (Projects::new(Arc::new(GitCli)), args),
    };

    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            theme::init(cx);
            ui::init(cx);
            config::init(cx);

            cx.on_window_closed(|cx, _window_id| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let projects = cx.new(|_| projects);
            for path in paths {
                let opened = projects.update(cx, |projects, cx| projects.open(path.clone(), cx));
                cx.spawn(async move |_| {
                    if let Err(error) = opened.await {
                        eprintln!("couldn't open {}: {error:#}", path.display());
                    }
                })
                .detach();
            }
            ui::open_window(projects, None, cx);
            cx.activate(true);
        });
}

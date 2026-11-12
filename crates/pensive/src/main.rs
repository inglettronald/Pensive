mod config;

use std::process::ExitCode;

use gpui::{App, AppContext};
use model::Projects;

fn main() -> ExitCode {
    let projects = match std::env::args().nth(1).as_deref() {
        // Empty until a backend fills it.
        None => Projects::default(),
        // The sample projects, for clicking around views before there's a backend.
        #[cfg(debug_assertions)]
        Some("mocked") => model::fixtures::projects(),
        Some(arg) => {
            eprintln!("unknown argument `{arg}`");
            return ExitCode::FAILURE;
        }
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
            ui::open_window(projects, None, cx);
            cx.activate(true);
        });
    ExitCode::SUCCESS
}

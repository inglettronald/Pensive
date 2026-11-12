use std::process::ExitCode;

use gpui::{App, AppContext};
use model::{Changes, Projects};

fn main() -> ExitCode {
    let (projects, changes) = match std::env::args().nth(1).as_deref() {
        // Empty until a backend fills them.
        None => (Projects::default(), Changes::default()),
        // The sample projects and changes, for clicking around views before there's a backend.
        #[cfg(debug_assertions)]
        Some("mocked") => (model::fixtures::projects(), model::fixtures::changes()),
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

            cx.on_window_closed(|cx, _window_id| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let projects = cx.new(|_| projects);
            let changes = cx.new(|_| changes);
            ui::open_window(projects, changes, None, cx);
            cx.activate(true);
        });
    ExitCode::SUCCESS
}

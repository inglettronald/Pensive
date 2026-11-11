use gpui::{App, AppContext};
use model::Projects;

fn main() {
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            theme::init(cx);
            ui::init(cx);

            cx.on_window_closed(|cx, _window_id| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // Empty until a backend fills it.
            let projects = cx.new(|_| Projects::default());
            ui::open_window(projects, None, cx);
            cx.activate(true);
        });
}

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use ui::Pensive;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(960.), px(640.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Pensive".into()),
                    ..Default::default()
                }),
                app_id: Some("pensive".into()),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Pensive),
        )
        .expect("failed to open the main window");
        cx.activate(true);
    });
}

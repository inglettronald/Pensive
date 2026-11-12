mod backends;
mod config_sync;

use gpui::{App, AppContext};

fn main() {
    #[cfg(target_family = "wasm")]
    {
        gpui_platform::web_init();
        // Debug builds log each shader wgpu compiles, which buries the warnings.
        log::set_max_level(log::LevelFilter::Info);
    }
    let backends::Startup {
        projects,
        paths,
        config,
    } = backends::startup();

    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            theme::init(cx);
            ui::init(cx);
            if let Some(config) = config {
                config_sync::init(config, cx);
            }

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

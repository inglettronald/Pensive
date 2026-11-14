mod backends;
mod config_sync;
#[cfg(feature = "demo")]
mod demo;
#[cfg(all(feature = "demo", not(target_family = "wasm")))]
mod mock_thumbnails;

use gpui::{App, AppContext};
use model::Notices;

#[cfg(all(target_family = "wasm", not(feature = "demo")))]
compile_error!("the browser build is a demo build: build it with `--features demo`");

fn main() {
    #[cfg(target_family = "wasm")]
    {
        gpui_platform::web_init();
        // Debug builds log each shader wgpu compiles, which buries the warnings.
        log::set_max_level(log::LevelFilter::Info);
    }
    #[cfg(feature = "demo")]
    let demo = demo::Options::read().unwrap_or_else(|problem| demo::stop(&problem));
    let backends::Startup {
        projects,
        paths,
        config,
        check,
    } = backends::startup();

    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            theme::init(cx);
            ui::init(cx);
            #[cfg(all(feature = "demo", not(target_family = "wasm")))]
            mock_thumbnails::start(cx);
            if let Some(config) = config {
                config_sync::init(config, cx);
            }
            let checking = cx.background_executor().spawn(async move { check() });
            cx.spawn(async move |cx| {
                for notice in checking.await {
                    cx.update(|cx| Notices::post(notice, cx));
                }
            })
            .detach();

            cx.on_window_closed(|cx, _window_id| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let projects = cx.new(|_| projects);
            for path in paths {
                // Projects reports a repository that won't open.
                projects
                    .update(cx, |projects, cx| projects.open(path, cx))
                    .detach();
            }
            #[cfg(feature = "demo")]
            ui::open_demo_window(projects, demo.mock.as_deref(), demo.size, cx)
                .expect("the demo's options name a mock that exists");
            #[cfg(not(feature = "demo"))]
            ui::open_window(projects, cx);
            cx.activate(true);
        });
}

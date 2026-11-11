//! Named mock states for views, rendered headlessly for review and regression tests.

use std::{cell::Cell, sync::Arc};

use gpui::{
    AnyView, App, AppContext, Context, HeadlessAppContext, IntoElement, Pixels,
    PlatformHeadlessRenderer, PlatformTextSystem, Render, Result, Size, Window, px, size,
};
use image::RgbaImage;

/// One view in one state, built from fixed data.
pub struct Mock {
    pub view: &'static str,
    pub name: &'static str,
    pub size: Size<Pixels>,
    pub build: fn(&mut Window, &mut App) -> AnyView,
}

impl Mock {
    pub fn new(
        view: &'static str,
        name: &'static str,
        build: fn(&mut Window, &mut App) -> AnyView,
    ) -> Self {
        Self {
            view,
            name,
            size: size(px(960.), px(640.)),
            build,
        }
    }

    pub fn size(self, size: Size<Pixels>) -> Self {
        Self { size, ..self }
    }

    /// Renders the mock at twice its size without opening a window. Returns `None` on platforms
    /// with no headless renderer.
    pub fn render(&self) -> Result<Option<RgbaImage>> {
        let Some(renderer) = headless_renderer()? else {
            return Ok(None);
        };
        let renderer = Cell::new(Some(renderer));
        let mut cx = HeadlessAppContext::with_platform(text_system(), Arc::new(()), move || {
            Ok(renderer.take())
        });
        let build = self.build;
        let window = cx.open_window(self.size, |window, cx| {
            let view = build(window, cx);
            cx.new(|_| Frame(view))
        })?;
        cx.run_until_parked();
        cx.capture_screenshot(window.into()).map(Some)
    }
}

/// Holds a mock's view so the window has a concrete root type.
struct Frame(AnyView);

impl Render for Frame {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.0.clone()
    }
}

/// Built once per thread, because each platform built just for its text system leaks threads.
fn text_system() -> Arc<dyn PlatformTextSystem> {
    thread_local! {
        static TEXT_SYSTEM: Arc<dyn PlatformTextSystem> =
            gpui_platform::current_platform(true).text_system();
    }
    TEXT_SYSTEM.with(Arc::clone)
}

#[cfg(not(target_os = "windows"))]
fn headless_renderer() -> Result<Option<Box<dyn PlatformHeadlessRenderer>>> {
    gpui_platform::current_headless_renderer()
}

/// gpui_platform has no Windows headless renderer, so Windows uses the wgpu one Linux uses.
#[cfg(target_os = "windows")]
fn headless_renderer() -> Result<Option<Box<dyn PlatformHeadlessRenderer>>> {
    Ok(Some(Box::new(gpui_wgpu::WgpuHeadlessRenderer::new()?)))
}

//! Draws a view on the headless renderer and captures it, without opening a window.

use std::{cell::Cell, sync::Arc};

use gpui::{
    AnyView, App, Context, Entity, HeadlessAppContext, IntoElement, Pixels,
    PlatformHeadlessRenderer, PlatformTextSystem, Render, Result, Size, Window,
};
use image::RgbaImage;

/// Opens a window of `size` on the headless renderer and captures it at twice its size.
pub(crate) fn render_headless<V: Render>(
    size: Size<Pixels>,
    build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> Result<Option<RgbaImage>> {
    let Some(renderer) = headless_renderer()? else {
        return Ok(None);
    };
    let renderer = Cell::new(Some(renderer));
    let mut cx =
        HeadlessAppContext::with_platform(text_system(), Arc::new(assets::Assets), move || {
            Ok(renderer.take())
        });
    cx.update(theme::init);
    let window = cx.open_window(size, build_root)?;
    cx.run_until_parked();
    // The app draws again on the next frame, and some views finish setting up there once their
    // first layout is measured, such as scrolling to a heading. The mock draws that frame too.
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
    cx.run_until_parked();
    cx.capture_screenshot(window.into()).map(Some)
}

/// Holds a mock's view so the window has a concrete root type.
pub(crate) struct Frame(pub AnyView);

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

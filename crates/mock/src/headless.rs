//! Draws a view on the headless renderer and captures it, without opening a window.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use gpui::{
    AnyView, App, Context, DevicePixels, Entity, HeadlessAppContext, IntoElement, Pixels,
    PlatformAtlas, PlatformHeadlessRenderer, PlatformTextSystem, Render, Result, Scene, Size,
    Window,
};
use image::RgbaImage;

use crate::painted::Painted;

/// GPUI's headless windows draw at two pixels per point.
pub(crate) const SCALE: f32 = 2.;

/// What a capture produces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Output {
    Image,
    /// Only what was painted, which leaves the image empty and the GPU out of it.
    PaintedOnly,
}

/// A window's image, with what was painted to make it.
pub struct Capture {
    pub image: RgbaImage,
    pub painted: Painted,
}

/// Opens a window of `size` on the headless renderer and captures it at twice its size.
pub(crate) fn capture<V: Render>(
    size: Size<Pixels>,
    output: Output,
    build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> Result<Option<Capture>> {
    let Some(renderer) = headless_renderer()? else {
        return Ok(None);
    };
    let painted = Rc::new(RefCell::new(Painted::default()));
    let renderer: Box<dyn PlatformHeadlessRenderer> = Box::new(Recording {
        renderer,
        painted: painted.clone(),
        size,
        output,
    });
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
    let image = cx.capture_screenshot(window.into())?;
    let painted = painted.take();
    Ok(Some(Capture { image, painted }))
}

/// Notes what each captured frame paints, then has `renderer` draw it if `output` needs that.
struct Recording {
    renderer: Box<dyn PlatformHeadlessRenderer>,
    painted: Rc<RefCell<Painted>>,
    size: Size<Pixels>,
    output: Output,
}

impl PlatformHeadlessRenderer for Recording {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> Result<RgbaImage> {
        *self.painted.borrow_mut() = Painted::from_scene(
            scene,
            self.size.width.as_f32(),
            self.size.height.as_f32(),
            SCALE,
        );
        match self.output {
            Output::Image => self.renderer.render_scene_to_image(scene, size),
            Output::PaintedOnly => Ok(RgbaImage::new(0, 0)),
        }
    }

    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> Result<()> {
        match self.output {
            Output::Image => self.renderer.render_scene(scene, size),
            Output::PaintedOnly => Ok(()),
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.renderer.sprite_atlas()
    }
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

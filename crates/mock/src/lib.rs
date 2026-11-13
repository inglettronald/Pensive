//! Named mock states for views, rendered headlessly for review and regression tests.

pub mod atlas;

use std::{cell::Cell, sync::Arc};

use anyhow::anyhow;
pub use atlas::Atlas;
use gpui::{
    AnyElement, AnyView, App, AppContext, Context, Entity, HeadlessAppContext, IntoElement,
    ParentElement, Pixels, PlatformHeadlessRenderer, PlatformTextSystem, Render, Result, Size,
    Styled, Window, div, px, size,
};
use image::RgbaImage;
use model::{Notices, Origin};
use theme::ActiveTheme;

/// One view or component in one state, built from fixed data.
#[derive(Clone)]
pub struct Mock {
    /// The view or component this shows. Also its output folder.
    pub subject: &'static str,
    pub name: &'static str,
    pub size: Size<Pixels>,
    content: Content,
    /// Whether it shows fixture gaps on purpose, such as a sample notice about one.
    expects_gaps: bool,
}

#[derive(Clone)]
enum Content {
    View(fn(&mut Window, &mut App) -> AnyView),
    Element(fn() -> AnyElement),
}

impl Mock {
    /// A mock of a view, or of anything that needs `App` to set up.
    pub fn new(
        subject: &'static str,
        name: &'static str,
        build: fn(&mut Window, &mut App) -> AnyView,
    ) -> Self {
        Self::with_content(subject, name, Content::View(build))
    }

    /// A mock of a stateless component, rebuilt from its props every frame.
    pub fn component(
        subject: &'static str,
        name: &'static str,
        render: fn() -> AnyElement,
    ) -> Self {
        Self::with_content(subject, name, Content::Element(render))
    }

    fn with_content(subject: &'static str, name: &'static str, content: Content) -> Self {
        Self {
            subject,
            name,
            size: size(px(960.), px(640.)),
            content,
            expects_gaps: false,
        }
    }

    pub fn size(self, size: Size<Pixels>) -> Self {
        Self { size, ..self }
    }

    /// Lets the mock render although the fixtures had nothing for something it asked for.
    pub fn expects_fixture_gaps(self) -> Self {
        Self {
            expects_gaps: true,
            ..self
        }
    }

    /// `<subject>/<name>`, which is also its PNG's path under `target/mocks`.
    pub fn id(&self) -> String {
        format!("{}/{}", self.subject, self.name)
    }

    pub fn build(&self, window: &mut Window, cx: &mut App) -> AnyView {
        match self.content {
            Content::View(build) => build(window, cx),
            Content::Element(render) => element_view(cx, render),
        }
    }

    /// Renders the mock at twice its size without opening a window, with the default theme and
    /// the bundled fonts and assets. Returns `None` on platforms with no headless renderer.
    ///
    /// Fails if the fixtures had nothing for something the mock asked for, unless it
    /// [expects that](Self::expects_fixture_gaps).
    pub fn render(&self) -> Result<Option<RgbaImage>> {
        render_headless(
            self.size,
            |window, cx| {
                let view = self.build(window, cx);
                cx.new(|_| Frame(view))
            },
            |cx| match fixture_gaps(cx) {
                gaps if gaps.is_empty() || self.expects_gaps => Ok(()),
                gaps => Err(anyhow!(
                    "{} asked for what the fixtures don't have:\n{}\nAdd it to \
                     model::fixtures, or mark a mock that shows it on purpose with \
                     expects_fixture_gaps.",
                    self.id(),
                    gaps.join("\n"),
                )),
            },
        )
    }
}

/// Each notice blamed on the fixtures, as its title and detail.
fn fixture_gaps(cx: &mut App) -> Vec<String> {
    Notices::global(cx)
        .read(cx)
        .held()
        .into_iter()
        .filter(|held| held.notice.origin == Origin::Fixture)
        .map(|held| {
            let notice = &held.notice;
            match &notice.detail {
                Some(detail) => format!("  {}: {detail}", notice.title),
                None => format!("  {}", notice.title),
            }
        })
        .collect()
}

/// Opens a window of `size` on the headless renderer, waits for its work to finish, runs `check`,
/// and captures the window at twice its size.
fn render_headless<V: Render>(
    size: Size<Pixels>,
    build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    check: impl FnOnce(&mut App) -> Result<()>,
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
    cx.update(check)?;
    cx.capture_screenshot(window.into()).map(Some)
}

/// Wraps a component's render function in a view on the app background, for mocks that need
/// `App` to set up before showing a component.
pub fn element_view(cx: &mut App, render: fn() -> AnyElement) -> AnyView {
    cx.new(|_| ElementView(render)).into()
}

struct ElementView(fn() -> AnyElement);

impl Render for ElementView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().colors.surface_app)
            .child((self.0)())
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

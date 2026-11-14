//! Named mock states for views, rendered headlessly for review and regression tests.

pub mod atlas;
pub mod diff;
#[cfg(not(target_family = "wasm"))]
mod headless;

pub use atlas::Atlas;
use gpui::{
    AnyElement, AnyView, App, AppContext, Context, IntoElement, ParentElement, Pixels, Render,
    Size, Styled, Window, div, px, size,
};
use theme::ActiveTheme;

/// One view or component in one state, built from fixed data.
#[derive(Clone)]
pub struct Mock {
    /// The view or component this shows. Also its output folder.
    pub subject: &'static str,
    pub name: &'static str,
    pub size: Size<Pixels>,
    content: Content,
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

    #[expect(clippy::disallowed_methods, reason = "a mock's window is a fixed size")]
    fn with_content(subject: &'static str, name: &'static str, content: Content) -> Self {
        Self {
            subject,
            name,
            size: size(px(960.), px(640.)),
            content,
        }
    }

    /// The window the mock renders in, in pixels.
    #[expect(clippy::disallowed_methods, reason = "a mock's window is a fixed size")]
    pub fn size(self, width: f32, height: f32) -> Self {
        Self {
            size: size(px(width), px(height)),
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
}

/// The browser has no headless renderer, and GPUI's test support doesn't build for it.
#[cfg(not(target_family = "wasm"))]
impl Mock {
    /// Renders the mock at twice its size without opening a window, with the default theme and
    /// the bundled fonts and assets. Returns `None` on platforms with no headless renderer.
    pub fn render(&self) -> gpui::Result<Option<image::RgbaImage>> {
        headless::render_headless(self.size, |window, cx| {
            let view = self.build(window, cx);
            cx.new(|_| headless::Frame(view))
        })
    }
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

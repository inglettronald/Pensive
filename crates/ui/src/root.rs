use gpui::{Context, Window, div, prelude::*, rgb};

/// The view that fills the main window.
pub struct Pensive;

impl Render for Pensive {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xe0e0e0))
            .text_xl()
            .child("Pensive")
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![mock::Mock::new("root", "default", |_, cx| {
        cx.new(|_| Pensive).into()
    })]
}

use gpui::{Context, Window, div, prelude::*};
use theme::{ActiveTheme, Typeset};

/// The view that fills the main window.
pub struct Pensive;

impl Render for Pensive {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let heading = cx.density().heading;
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .bg(theme.colors.surface_app)
            .text_color(theme.colors.text)
            .font_family(theme.fonts.ui.clone())
            .typeset(heading)
            .child("Pensive")
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![mock::Mock::new("root", "default", |_, cx| {
        cx.new(|_| Pensive).into()
    })]
}

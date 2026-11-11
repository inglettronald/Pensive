//! A key or key combination, drawn as a keycap.

use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div};
use theme::{ActiveTheme, Typeset};

#[derive(IntoElement)]
pub struct Kbd {
    keys: SharedString,
}

impl Kbd {
    /// Keys as the user reads them, such as `Ctrl K` or `↑↓`.
    pub fn new(keys: impl Into<SharedString>) -> Self {
        Self { keys: keys.into() }
    }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        div()
            .flex_none()
            .px_1p5()
            .rounded(density.radius_sm)
            .border_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_raised)
            .text_color(colors.text_faint)
            .typeset(density.label)
            .whitespace_nowrap()
            .child(self.keys)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::{px, size};

    vec![
        mock::Mock::component("kbd", "gallery", || {
            crate::Gallery::new("Kbd")
                .section(
                    "Keys",
                    [
                        "Tab",
                        "Enter",
                        "Ctrl K",
                        "Ctrl Shift O",
                        "↑↓",
                        "←→",
                        "Ctrl ,",
                    ]
                    .map(|keys| (keys, Kbd::new(keys).into_any_element())),
                )
                .into_any_element()
        })
        .size(size(px(640.), px(200.))),
    ]
}

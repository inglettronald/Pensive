//! A small uppercase label, such as the origin of a notice.

use gpui::{
    App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
};
use theme::{ActiveTheme, Typeset, dp};

#[derive(IntoElement)]
pub struct Tag {
    label: SharedString,
    accent: bool,
}

impl Tag {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            accent: false,
        }
    }

    /// Draws it in the accent blue, for an interface signal such as a gap in the sample data.
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let (text, fill) = if self.accent {
            (colors.accent_fg, colors.accent_subtle)
        } else {
            (colors.text_muted, colors.surface_selected)
        };
        div()
            .flex_none()
            .px(dp(5.))
            .rounded(density.radius_sm)
            .bg(fill)
            .text_color(text)
            .typeset(density.label)
            .text_size(density.label.size - dp(1.))
            .font_weight(FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .child(self.label.to_uppercase())
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::{px, size};

    vec![
        mock::Mock::component("tag", "gallery", || {
            crate::Gallery::new("Tag")
                .section(
                    "Kinds",
                    [
                        ("Plain", Tag::new("Git").into_any_element()),
                        ("Accent", Tag::new("Fixture").accent().into_any_element()),
                    ],
                )
                .into_any_element()
        })
        .size(size(px(640.), px(200.))),
    ]
}

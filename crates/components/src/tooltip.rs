//! A small raised box for what doesn't fit where it's anchored, with an optional shortcut.

use gpui::{AnyElement, App, BoxShadow, SharedString, Window, div, hsla, prelude::*, px};
use theme::{ActiveTheme, Typeset};

use crate::Kbd;

#[derive(IntoElement)]
pub struct Tooltip {
    content: AnyElement,
    shortcut: Option<(SharedString, SharedString)>,
}

impl Tooltip {
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            content: content.into_any_element(),
            shortcut: None,
        }
    }

    /// An action the tooltip offers while it's open, such as `Copy` on `Ctrl Shift C`.
    pub fn shortcut(
        mut self,
        label: impl Into<SharedString>,
        keys: impl Into<SharedString>,
    ) -> Self {
        self.shortcut = Some((label.into(), keys.into()));
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        div()
            .flex()
            .items_center()
            .gap_2p5()
            .py_1()
            .pl_2()
            .pr_1p5()
            .rounded(density.radius_md)
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface_raised)
            .shadow(vec![
                BoxShadow::new(px(0.), px(6.), hsla(0., 0., 0., 0.5)).blur_radius(px(16.)),
            ])
            .typeset(density.small)
            .text_color(colors.text)
            .whitespace_nowrap()
            .child(self.content)
            .when_some(self.shortcut, |tooltip, (label, keys)| {
                tooltip
                    .child(div().w(px(1.)).self_stretch().bg(colors.border_subtle))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_color(colors.text_faint)
                            .child(label)
                            .child(Kbd::new(keys)),
                    )
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("tooltip", "gallery", || {
            crate::Gallery::new("Tooltip")
                .section(
                    "Kinds",
                    [
                        ("Text", Tooltip::new("Fetch from origin").into_any_element()),
                        (
                            "With a shortcut",
                            Tooltip::new("docs/design.md")
                                .shortcut("Copy", "Ctrl Shift C")
                                .into_any_element(),
                        ),
                    ],
                )
                .into_any_element()
        })
        .size(size(px(640.), px(200.))),
    ]
}

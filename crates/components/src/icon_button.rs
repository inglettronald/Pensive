//! A square button that shows only an icon.

use gpui::{App, ElementId, Hsla, Pixels, Window, div, prelude::*, px};
use theme::{ActiveTheme, ThemeColors};

use crate::{Callback, Icon, IconName, callback};

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    size: Pixels,
    color: fn(&ThemeColors) -> Hsla,
    on_click: Option<Callback<()>>,
}

impl IconButton {
    /// A 24 px button with a 14 px icon in `text_muted`.
    pub fn new(id: impl Into<ElementId>, icon: IconName) -> Self {
        Self {
            id: id.into(),
            icon,
            size: px(24.),
            color: |colors| colors.text_muted,
            on_click: None,
        }
    }

    /// The button's side. The icon stays 14 px.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Picks the icon's color role.
    pub fn color(mut self, color: fn(&ThemeColors) -> Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(callback(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let hover = colors.surface_raised;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.size)
            .rounded(cx.density().radius_md)
            .hover(move |button| button.bg(hover))
            .child(
                Icon::new(self.icon)
                    .size(px(14.))
                    .color((self.color)(colors)),
            )
            .when_some(self.on_click, |button, handler| {
                button.on_click(move |_, window, cx| handler((), window, cx))
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("icon_button", "gallery", || {
            crate::Gallery::new("IconButton")
                .section(
                    "Icons",
                    [
                        ("Fetch", IconName::RefreshCw),
                        ("Settings", IconName::Settings),
                        ("Minimize", IconName::Minus),
                        ("Maximize", IconName::Square),
                        ("Close", IconName::X),
                    ]
                    .map(|(label, icon)| (label, IconButton::new(label, icon).into_any_element())),
                )
                .into_any_element()
        })
        .size(size(px(640.), px(200.))),
    ]
}

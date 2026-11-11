//! An on/off switch.

use std::rc::Rc;

use gpui::{App, ClickEvent, ElementId, Window, div, prelude::*, px};
use theme::ActiveTheme;

use crate::Callback;

#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    on: bool,
    on_change: Option<Callback<bool>>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
            on_change: None,
        }
    }

    /// Called with the new value when the switch is clicked.
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let (track, knob) = if self.on {
            (colors.accent_solid, colors.text_strong)
        } else {
            (colors.border, colors.text_muted)
        };
        let on = self.on;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .when_else(
                on,
                |track| track.justify_end(),
                |track| track.justify_start(),
            )
            .w(px(28.))
            .h(px(16.))
            .p(px(2.))
            .rounded_full()
            .bg(track)
            .child(div().size(px(12.)).rounded_full().bg(knob))
            .when_some(self.on_change, |track, handler| {
                track.on_click(move |_: &ClickEvent, window, cx| handler(!on, window, cx))
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("toggle", "gallery", || {
            crate::Gallery::new("Toggle")
                .section(
                    "States",
                    [("On", true), ("Off", false)]
                        .map(|(label, on)| (label, Toggle::new(label, on).into_any_element())),
                )
                .into_any_element()
        })
        .size(size(px(480.), px(200.))),
    ]
}

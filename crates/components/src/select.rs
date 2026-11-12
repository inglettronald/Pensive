//! The closed state of a dropdown: the current value and a chevron.

use std::rc::Rc;

use gpui::{App, ElementId, SharedString, Window, div, prelude::*, px};
use theme::{ActiveTheme, Typeset};

use crate::{Callback, Icon, IconName};

#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    value: SharedString,
    on_click: Option<Callback<()>>,
}

impl Select {
    pub fn new(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            on_click: None,
        }
    }

    /// Called when the select is clicked, to open its options.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(move |(), window, cx| handler(window, cx)));
        self
    }
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap_1p5()
            .h(px(24.))
            .pl_2()
            .pr_1p5()
            .rounded(density.radius_sm)
            .border_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_raised)
            .text_color(colors.text)
            .typeset(density.small)
            .whitespace_nowrap()
            .child(self.value)
            .child(
                Icon::new(IconName::ChevronDown)
                    .size(px(12.))
                    .color(colors.text_faint),
            )
            .when_some(self.on_click, |select, handler| {
                select.on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    handler((), window, cx)
                })
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("select", "gallery", || {
            crate::Gallery::new("Select")
                .section(
                    "Values",
                    ["Edge bar", "Calm", "Tinted lines"]
                        .map(|value| (value, Select::new(value, value).into_any_element())),
                )
                .into_any_element()
        })
        .size(size(px(480.), px(200.))),
    ]
}

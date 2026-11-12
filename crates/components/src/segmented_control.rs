//! A row of mutually exclusive options in a track, with at most one selected.

use std::rc::Rc;

use gpui::{App, ElementId, SharedString, Window, div, prelude::*, px};
use theme::{ActiveTheme, Typeset};

use crate::Callback;

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    options: Vec<SharedString>,
    selected: Option<usize>,
    on_select: Option<Callback<usize>>,
}

impl SegmentedControl {
    pub fn new(
        id: impl Into<ElementId>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: impl Into<Option<usize>>,
    ) -> Self {
        Self {
            id: id.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected: selected.into(),
            on_select: None,
        }
    }

    /// Called with the index of the option that was clicked.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let on_select = self.on_select;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .gap_0p5()
            .p_0p5()
            .rounded(density.radius_md)
            .border_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_app)
            .typeset(density.small)
            .children(self.options.into_iter().enumerate().map(|(index, option)| {
                let selected = self.selected == Some(index);
                div()
                    .id(index)
                    .px_2()
                    .line_height(px(20.))
                    .rounded(density.radius_sm)
                    .whitespace_nowrap()
                    .when_else(
                        selected,
                        |option| option.bg(colors.border).text_color(colors.text_strong),
                        |option| option.text_color(colors.text_muted),
                    )
                    .child(option)
                    .when_some(on_select.clone(), |option, handler| {
                        option.on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            handler(index, window, cx)
                        })
                    })
            }))
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("segmented_control", "gallery", || {
            const OPTIONS: [&str; 3] = ["Off", "New file", "Both"];
            crate::Gallery::new("SegmentedControl")
                .section(
                    "Selected",
                    OPTIONS.iter().enumerate().map(|(index, &label)| {
                        (
                            label,
                            SegmentedControl::new(label, OPTIONS, index).into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .size(size(px(640.), px(200.))),
    ]
}

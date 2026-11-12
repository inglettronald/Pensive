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
                track.on_click(move |_: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    handler(!on, window, cx)
                })
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

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{Context, Modifiers, TestAppContext, point};

    use super::*;

    /// A clickable row with a toggle in its top-left corner, like a setting in the palette.
    struct Row {
        row_clicks: Rc<Cell<usize>>,
        toggled: Rc<Cell<Option<bool>>>,
    }

    impl Render for Row {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let row_clicks = self.row_clicks.clone();
            let toggled = self.toggled.clone();
            div()
                .id("row")
                .size_full()
                .on_click(move |_, _, _| row_clicks.set(row_clicks.get() + 1))
                .child(
                    Toggle::new("toggle", false).on_change(move |on, _, _| toggled.set(Some(on))),
                )
        }
    }

    #[gpui::test]
    fn clicking_the_toggle_doesnt_also_click_its_row(cx: &mut TestAppContext) {
        cx.update(theme::init);
        let row_clicks = Rc::new(Cell::new(0));
        let toggled = Rc::new(Cell::new(None));
        let (_, cx) = cx.add_window_view(|_, _| Row {
            row_clicks: row_clicks.clone(),
            toggled: toggled.clone(),
        });
        cx.simulate_click(point(px(6.), px(6.)), Modifiers::none());
        assert_eq!(toggled.get(), Some(true));
        assert_eq!(row_clicks.get(), 0);
    }
}

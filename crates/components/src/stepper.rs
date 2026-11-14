//! A value between a minus and a plus that step it down and up, for ordered choices such as a
//! zoom level. The end it can't go past dims. In settings, `ScrollToChange` lets the wheel step it
//! too.

use std::rc::Rc;

use gpui::{App, ElementId, SharedString, Window, div, prelude::*};
use theme::{ActiveTheme, Typeset, dp};

use crate::{Callback, Icon, IconName};

#[derive(IntoElement)]
pub struct Stepper {
    id: ElementId,
    value: SharedString,
    can_decrease: bool,
    can_increase: bool,
    on_step: Option<Callback<isize>>,
}

impl Stepper {
    pub fn new(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            can_decrease: true,
            can_increase: true,
            on_step: None,
        }
    }

    /// Which buttons work. A button that doesn't dims, at either end of the range.
    pub fn limits(mut self, can_decrease: bool, can_increase: bool) -> Self {
        self.can_decrease = can_decrease;
        self.can_increase = can_increase;
        self
    }

    /// Called with -1 for minus and 1 for plus.
    pub fn on_step(mut self, handler: impl Fn(isize, &mut Window, &mut App) + 'static) -> Self {
        self.on_step = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Stepper {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let button = |icon: IconName, step: isize, stopped: bool| {
            div()
                .id(if step < 0 { "minus" } else { "plus" })
                .flex()
                .items_center()
                .justify_center()
                .w(density.row_height)
                .child(Icon::new(icon).size(dp(14.)).color(if stopped {
                    colors.text_disabled
                } else {
                    colors.text_muted
                }))
                .when(!stopped, |button| {
                    button
                        .hover(|button| button.bg(colors.surface_selected))
                        .when_some(self.on_step.clone(), |button, handler| {
                            button.on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                handler(step, window, cx)
                            })
                        })
                })
        };
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .h(density.row_height)
            .overflow_hidden()
            .rounded(density.radius_md)
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface_raised)
            .child(button(IconName::Minus, -1, !self.can_decrease))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .min_w(dp(56.))
                    .px_2p5()
                    .border_x_1()
                    .border_color(colors.border_subtle)
                    .bg(colors.surface_app)
                    .font_family(theme.fonts.code.clone())
                    .typeset(density.code)
                    .whitespace_nowrap()
                    .text_color(colors.text_strong)
                    .child(self.value),
            )
            .child(button(IconName::Plus, 1, !self.can_increase))
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("stepper", "gallery", || {
            crate::Gallery::new("Stepper")
                .section(
                    "Range",
                    [
                        ("Within", "100%", true, true),
                        ("At the start", "80%", false, true),
                        ("At the end", "200%", true, false),
                    ]
                    .map(|(label, value, can_decrease, can_increase)| {
                        (
                            label,
                            Stepper::new(label, value)
                                .limits(can_decrease, can_increase)
                                .into_any_element(),
                        )
                    }),
                )
                .section(
                    "Values",
                    [
                        ("Zoom", "110%"),
                        ("Code size", "13 px"),
                        ("Toast", "3 seconds"),
                    ]
                    .map(|(label, value)| (label, Stepper::new(label, value).into_any_element())),
                )
                .into_any_element()
        })
        .size(640., 280.),
    ]
}

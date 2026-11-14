//! A value between − and + buttons, for ordered choices such as a zoom level. The wheel steps it
//! too, once the pointer has rested on it.

use std::{rc::Rc, time::Duration};

use gpui::{
    App, ElementId, Entity, Pixels, ScrollEvent, SharedString, Task, Window, div, prelude::*,
};
use theme::{ActiveTheme, Typeset, dp};

use crate::{Callback, Icon, IconName};

/// How long the pointer rests on the stepper, with no wheel events, before the wheel steps it.
/// Scrolling a page past the stepper never changes it.
const SETTLE: Duration = Duration::from_secs(1);

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

    /// Called with how many steps to move: negative to decrease, positive to increase.
    pub fn on_step(mut self, handler: impl Fn(isize, &mut Window, &mut App) + 'static) -> Self {
        self.on_step = Some(Rc::new(handler));
        self
    }
}

/// Whether the wheel steps the value yet, and the wheel distance not yet turned into a step.
#[derive(Default)]
struct Wheel {
    armed: bool,
    pending: Pixels,
    settling: Option<Task<()>>,
}

impl Wheel {
    /// Disarms, and arms again after `SETTLE` unless this is called again first.
    fn settle(&mut self, cx: &mut Context<Self>) {
        self.armed = false;
        self.pending = Pixels::ZERO;
        self.settling = Some(cx.spawn(async move |wheel, cx| {
            cx.background_executor().timer(SETTLE).await;
            wheel.update(cx, |wheel, _| wheel.armed = true).ok();
        }));
    }

    fn leave(&mut self) {
        self.armed = false;
        self.settling = None;
    }

    /// Whole steps in `event`, keeping the remainder for the next one. Up is positive.
    fn steps(&mut self, event: &ScrollEvent, line_height: Pixels) -> isize {
        self.pending += event.delta.pixel_delta(line_height).y;
        let steps = (self.pending / line_height).trunc();
        self.pending -= line_height * steps;
        steps as isize
    }
}

impl RenderOnce for Stepper {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let wheel: Entity<Wheel> =
            window.use_keyed_state(self.id.clone(), cx, |_, _| Wheel::default());
        let colors = &cx.theme().colors;
        let density = cx.density();
        let button = |id: &'static str, icon, step: isize, enabled: bool| {
            let on_step = self.on_step.clone().filter(|_| enabled);
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .w(dp(24.))
                .h(dp(20.))
                .rounded(density.radius_sm)
                .child(Icon::new(icon).size(dp(14.)).color(if enabled {
                    colors.text_muted
                } else {
                    colors.text_disabled
                }))
                .when_some(on_step, |button, on_step| {
                    let hover = colors.surface_raised;
                    button
                        .hover(move |button| button.bg(hover))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            on_step(step, window, cx)
                        })
                })
        };
        div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .items_center()
            .gap_0p5()
            .p_0p5()
            .rounded(density.radius_md)
            .border_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_app)
            .typeset(density.small)
            .child(button("decrease", IconName::Minus, -1, self.can_decrease))
            .child(
                div()
                    .min_w(dp(44.))
                    .text_center()
                    .line_height(dp(20.))
                    .text_color(colors.text_strong)
                    .child(self.value),
            )
            .child(button("increase", IconName::Plus, 1, self.can_increase))
            .on_hover({
                let wheel = wheel.clone();
                move |hovered, _, cx| {
                    wheel.update(cx, |wheel, cx| {
                        if *hovered {
                            wheel.settle(cx)
                        } else {
                            wheel.leave()
                        }
                    })
                }
            })
            .when_some(self.on_step, |stepper, on_step| {
                stepper.on_scroll(move |event, window, cx| {
                    if !wheel.read(cx).armed {
                        wheel.update(cx, |wheel, cx| wheel.settle(cx));
                        return;
                    }
                    cx.stop_propagation();
                    let line_height = window.line_height();
                    let steps = wheel.update(cx, |wheel, _| wheel.steps(event, line_height));
                    if steps != 0 {
                        on_step(steps, window, cx);
                    }
                })
            })
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
                        let stepper = Stepper::new(label, value)
                            .limits(can_decrease, can_increase)
                            .on_step(|_, _, _| {});
                        (label, stepper.into_any_element())
                    }),
                )
                .section(
                    "Values",
                    [("Zoom", "110%"), ("Code size", "13 px")].map(|(label, value)| {
                        (
                            label,
                            Stepper::new(label, value)
                                .on_step(|_, _, _| {})
                                .into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .size(640., 280.),
    ]
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "tests click and scroll at window pixels"
)]
mod tests {
    use std::cell::Cell;

    use gpui::{
        Context, Modifiers, Render, ScrollDelta, TestAppContext, VisualTestContext, point, px,
    };

    use super::*;

    struct Page {
        steps: Rc<Cell<isize>>,
        page_scrolls: Rc<Cell<usize>>,
    }

    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let steps = self.steps.clone();
            let page_scrolls = self.page_scrolls.clone();
            div()
                .id("page")
                .size_full()
                .on_scroll(move |_, _, _| page_scrolls.set(page_scrolls.get() + 1))
                .child(
                    Stepper::new("stepper", "100%")
                        .on_step(move |step, _, _| steps.set(steps.get() + step)),
                )
        }
    }

    fn scroll_up(cx: &mut VisualTestContext) {
        cx.simulate_event(ScrollEvent {
            position: point(px(50.), px(10.)),
            delta: ScrollDelta::Lines(point(0., 1.)),
            ..Default::default()
        });
    }

    #[gpui::test]
    fn the_wheel_steps_only_after_the_pointer_settles(cx: &mut TestAppContext) {
        cx.update(theme::init);
        let steps = Rc::new(Cell::new(0));
        let page_scrolls = Rc::new(Cell::new(0));
        let (_, cx) = cx.add_window_view(|_, _| Page {
            steps: steps.clone(),
            page_scrolls: page_scrolls.clone(),
        });
        cx.simulate_mouse_move(point(px(50.), px(10.)), None, Modifiers::none());

        scroll_up(cx);
        assert_eq!((steps.get(), page_scrolls.get()), (0, 1));

        // A wheel event before the pointer settles starts the wait again.
        cx.executor().advance_clock(SETTLE / 2);
        scroll_up(cx);
        cx.executor().advance_clock(SETTLE / 2);
        scroll_up(cx);
        assert_eq!((steps.get(), page_scrolls.get()), (0, 3));

        cx.executor().advance_clock(SETTLE);
        scroll_up(cx);
        assert_eq!((steps.get(), page_scrolls.get()), (1, 3));
    }
}

//! Hands the mouse wheel to a control with ordered values once the pointer has rested on it, so
//! scrolling past a card never changes a value but resting on one lets the wheel step through it.

use std::{rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, ScrollEvent, Task, Window, deferred, div,
    prelude::*, relative,
};
use theme::{ActiveTheme, Dp, dp};

use crate::{Callback, Icon, IconName, Tooltip, child_id};

/// How long the pointer rests on a control before it takes the wheel.
const DWELL: Duration = Duration::from_millis(400);
/// How far a trackpad scrolls for one step. A wheel steps once per notch.
const TRACKPAD_STEP: Dp = dp(24.);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Idle,
    /// The pointer arrived, and the wheel still scrolls the page. Counts arrivals, to key the
    /// line that fills and to ignore a timer from an earlier one.
    Resting(usize),
    Armed,
}

struct WheelState {
    phase: Phase,
    arrivals: usize,
    /// Scroll not yet turned into a step, in steps.
    pending: f32,
    _arm: Option<Task<()>>,
}

#[derive(IntoElement)]
pub struct ScrollToChange {
    id: ElementId,
    control: AnyElement,
    armed: Option<bool>,
    on_step: Option<Callback<isize>>,
}

impl ScrollToChange {
    /// `control` has the medium corner radius, as the dropdown, stepper and segmented control do,
    /// which the armed outline follows.
    pub fn new(id: impl Into<ElementId>, control: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            control: control.into_any_element(),
            armed: None,
            on_step: None,
        }
    }

    /// Shows the control as armed, outline and hint, for a mock. Resting on it does the same.
    pub fn armed(mut self, armed: bool) -> Self {
        self.armed = Some(armed);
        self
    }

    /// Called with 1 for each step down the wheel, or right, and -1 for each step up.
    pub fn on_step(mut self, handler: impl Fn(isize, &mut Window, &mut App) + 'static) -> Self {
        self.on_step = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ScrollToChange {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(child_id(&self.id, "wheel"), cx, |_, _| WheelState {
            phase: Phase::Idle,
            arrivals: 0,
            pending: 0.,
            _arm: None,
        });
        let phase = match self.armed {
            Some(true) => Phase::Armed,
            Some(false) => Phase::Idle,
            None => state.read(cx).phase,
        };
        let colors = &cx.theme().colors;
        let radius = cx.density().radius_md;
        let filling = match phase {
            Phase::Resting(arrival) => Some(
                div()
                    .absolute()
                    .left_0()
                    .top_full()
                    .mt(dp(3.))
                    .h(dp(2.))
                    .rounded_full()
                    .bg(colors.text_faint)
                    .with_animation(
                        child_id(&self.id, format!("rest-{arrival}")),
                        Animation::new(DWELL),
                        |line, delta| line.w(relative(delta)),
                    ),
            ),
            _ => None,
        };
        let armed = (phase == Phase::Armed).then(|| {
            [
                div()
                    .absolute()
                    .inset_0()
                    .rounded(radius)
                    .border_1()
                    .border_color(colors.text_faint)
                    .into_any_element(),
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_full()
                    .mr_2()
                    .flex()
                    .items_center()
                    .child(
                        deferred(Tooltip::new(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .child(Icon::new(IconName::ChevronsUpDown).size(dp(12.)))
                                .child("Scroll to change"),
                        ))
                        .priority(1),
                    )
                    .into_any_element(),
            ]
        });
        let on_step = self.on_step;
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .child(self.control)
            .children(filling)
            .children(armed.into_iter().flatten())
            .on_hover({
                let state = state.clone();
                move |hovered, _, cx| {
                    let hovered = *hovered;
                    state.update(cx, |state, cx| {
                        state.pending = 0.;
                        state._arm = None;
                        if !hovered {
                            state.phase = Phase::Idle;
                            cx.notify();
                            return;
                        }
                        state.arrivals += 1;
                        let arrival = state.arrivals;
                        state.phase = Phase::Resting(arrival);
                        state._arm = Some(cx.spawn(async move |state, cx| {
                            cx.background_executor().timer(DWELL).await;
                            state
                                .update(cx, |state, cx| {
                                    if state.phase == Phase::Resting(arrival) {
                                        state.phase = Phase::Armed;
                                        cx.notify();
                                    }
                                })
                                .ok();
                        }));
                        cx.notify();
                    })
                }
            })
            .on_scroll(move |event: &ScrollEvent, window, cx| {
                if state.read(cx).phase != Phase::Armed {
                    return;
                }
                cx.stop_propagation();
                let per_step = TRACKPAD_STEP.to_pixels(window.rem_size());
                // A wheel reports lines, one per notch, and a trackpad reports pixels.
                let steps = match event.delta {
                    gpui::ScrollDelta::Lines(lines) => -lines.y,
                    gpui::ScrollDelta::Pixels(pixels) => -(pixels.y / per_step),
                };
                let whole = state.update(cx, |state, _| {
                    state.pending += steps;
                    let whole = state.pending.trunc();
                    state.pending -= whole;
                    whole as isize
                });
                if whole != 0
                    && let Some(handler) = &on_step
                {
                    handler(whole.signum(), window, cx)
                }
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("scroll_to_change", "gallery", || {
            let control =
                || crate::SegmentedControl::new("line-numbers", ["Off", "New file", "Both"], 1);
            crate::Gallery::new("ScrollToChange")
                .section(
                    "States",
                    [("Resting or away", false), ("Armed", true)].map(|(label, armed)| {
                        (
                            label,
                            div()
                                .pl(dp(150.))
                                .child(ScrollToChange::new(label, control()).armed(armed))
                                .into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .size(960., 200.),
    ]
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "tests point and scroll at window pixels"
)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{Context, Modifiers, ScrollDelta, TestAppContext, point, px, size};

    use super::*;

    /// A scrollable page with one control in its top-left corner, like a settings card.
    struct Page {
        steps: Rc<RefCell<Vec<isize>>>,
        page_scrolls: Rc<RefCell<usize>>,
    }

    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let steps = self.steps.clone();
            let page_scrolls = self.page_scrolls.clone();
            div()
                .id("page")
                .size_full()
                .on_scroll(move |_, _, _| *page_scrolls.borrow_mut() += 1)
                .child(
                    ScrollToChange::new("control", div().w(px(100.)).h(px(28.)))
                        .on_step(move |step, _, _| steps.borrow_mut().push(step)),
                )
        }
    }

    fn wheel(cx: &mut gpui::VisualTestContext, lines: f32) {
        cx.simulate_event(ScrollEvent {
            position: point(px(20.), px(10.)),
            delta: ScrollDelta::Lines(point(0., lines)),
            ..Default::default()
        });
    }

    #[gpui::test]
    fn the_wheel_scrolls_the_page_until_the_pointer_has_rested(cx: &mut TestAppContext) {
        cx.update(theme::init);
        let steps = Rc::new(RefCell::new(Vec::new()));
        let page_scrolls = Rc::new(RefCell::new(0));
        let (_, cx) = cx.add_window_view(|_, _| Page {
            steps: steps.clone(),
            page_scrolls: page_scrolls.clone(),
        });
        cx.simulate_resize(size(px(400.), px(200.)));
        cx.simulate_mouse_move(point(px(20.), px(10.)), None, Modifiers::none());
        wheel(cx, -1.);
        assert!(steps.borrow().is_empty());
        assert_eq!(*page_scrolls.borrow(), 1);

        cx.executor().advance_clock(DWELL);
        cx.run_until_parked();
        wheel(cx, -1.);
        wheel(cx, 1.);
        assert_eq!(*steps.borrow(), [1, -1]);
        assert_eq!(
            *page_scrolls.borrow(),
            1,
            "an armed control keeps the wheel"
        );

        cx.simulate_mouse_move(point(px(300.), px(150.)), None, Modifiers::none());
        wheel(cx, -1.);
        assert_eq!(steps.borrow().len(), 2, "leaving hands the wheel back");
    }
}

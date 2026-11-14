//! An on/off switch. The outline stays the same either way, and only the knob changes: it slides
//! across and lights up when on.

use std::{rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, App, ClickEvent, ElementId, Window, div, ease_out_quint, prelude::*,
};
use theme::{ActiveTheme, Dp, dp};

use crate::{
    Callback,
    transition::{mix, transition},
};

const WIDTH: Dp = dp(34.);
const HEIGHT: Dp = dp(20.);
const KNOB: Dp = dp(16.);
/// Between the knob and the outline, which takes the other pixel of the 2 px around the knob.
const INSET: Dp = dp(1.);
const SLIDE: Duration = Duration::from_millis(120);

#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    on: bool,
    hovered: bool,
    on_change: Option<Callback<bool>>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
            hovered: false,
            on_change: None,
        }
    }

    /// Draws the outline as hovered, for a mock. The pointer does the same.
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// Called with the new value when the switch is clicked.
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let change = transition(&self.id, self.on, window, cx);
        let colors = &cx.theme().colors;
        let (off_knob, on_knob) = (colors.text_disabled, colors.text_strong);
        let hover_outline = colors.text_disabled;
        let left = |on: bool| if on { WIDTH - KNOB - INSET * 3. } else { INSET };
        let knob = div().absolute().top(INSET).size(KNOB).rounded_full();
        let knob = if change.moved() {
            let from = change.from;
            knob.with_animation(
                change.animation_id(&self.id, "knob"),
                Animation::new(SLIDE).with_easing(ease_out_quint()),
                move |knob, delta| {
                    let toward = |a: Dp, b: Dp| a + (b - a) * delta;
                    let (start, end) = if from {
                        (on_knob, off_knob)
                    } else {
                        (off_knob, on_knob)
                    };
                    knob.left(toward(left(from), left(!from)))
                        .bg(mix(start, end, delta))
                },
            )
            .into_any_element()
        } else {
            knob.left(left(self.on))
                .bg(if self.on { on_knob } else { off_knob })
                .into_any_element()
        };
        let on = self.on;
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .w(WIDTH)
            .h(HEIGHT)
            .rounded_full()
            .border_1()
            .border_color(if self.hovered {
                hover_outline
            } else {
                colors.control_outline
            })
            .bg(colors.surface_app)
            .hover(|track| track.border_color(hover_outline))
            .child(knob)
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
    vec![
        mock::Mock::component("toggle", "gallery", || {
            crate::Gallery::new("Toggle")
                .section(
                    "States",
                    [
                        ("On", true, false),
                        ("Off", false, false),
                        ("On, hovered", true, true),
                        ("Off, hovered", false, true),
                    ]
                    .map(|(label, on, hovered)| {
                        (
                            label,
                            Toggle::new(label, on).hovered(hovered).into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .size(480., 200.),
    ]
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "tests click and scroll at window pixels"
)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{Context, Modifiers, TestAppContext, point, px};

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

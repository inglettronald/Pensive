//! Background work in the title bar: a short bar that fills as the work goes, or sweeps when its
//! size isn't known, then one icon for any kind of work.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, App, ElementId, Window, div, ease_in_out, prelude::*, relative,
};
use theme::{ActiveTheme, Dp, dp};

use crate::{Callback, Icon, IconName, OnClick as _, callback};

const TRACK: Dp = dp(56.);
/// How long the sweep takes to cross the track.
const SWEEP: Duration = Duration::from_millis(1400);
/// The sweep's length, as a share of the track.
const SWEEP_LENGTH: f32 = 0.35;

#[derive(IntoElement)]
pub struct Activity {
    id: ElementId,
    progress: Option<f32>,
    on_click: Option<Callback<()>>,
}

impl Activity {
    /// Work that knows how far along it is, from 0 to 1.
    pub fn progress(id: impl Into<ElementId>, progress: f32) -> Self {
        Self {
            id: id.into(),
            progress: Some(progress.clamp(0., 1.)),
            on_click: None,
        }
    }

    /// Work whose size isn't known, such as a fetch.
    pub fn unknown(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            progress: None,
            on_click: None,
        }
    }

    /// Called when clicked, to show the list of tasks.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(callback(handler));
        self
    }
}

impl RenderOnce for Activity {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let fill = div().h_full().rounded_full().bg(colors.text_muted);
        let fill = match self.progress {
            Some(progress) => fill.w(relative(progress)).into_any_element(),
            None => fill
                .absolute()
                .top_0()
                .w(relative(SWEEP_LENGTH))
                .with_animation(
                    "sweep",
                    Animation::new(SWEEP).repeat().with_easing(ease_in_out),
                    |sweep, delta| {
                        sweep.left(relative(-SWEEP_LENGTH + delta * (1. + SWEEP_LENGTH)))
                    },
                )
                .into_any_element(),
        };
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(dp(28.))
            .px_2()
            .child(
                div()
                    .relative()
                    .w(TRACK)
                    .h(dp(3.))
                    .overflow_hidden()
                    .rounded_full()
                    .bg(colors.surface_selected)
                    .child(fill),
            )
            .child(
                Icon::new(IconName::CircleHelpFilled)
                    .size(dp(14.))
                    .color(colors.text_faint),
            )
            .on_click_callback(self.on_click)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("activity", "gallery", || {
            crate::Gallery::new("Activity")
                .section(
                    "Progress",
                    [
                        ("Just started", Activity::progress("start", 0.08)),
                        ("Most of the way", Activity::progress("most", 0.62)),
                        ("Size unknown, a sweep", Activity::unknown("sweep")),
                    ]
                    .map(|(label, activity)| (label, activity.into_any_element())),
                )
                .into_any_element()
        })
        .size(640., 200.),
    ]
}

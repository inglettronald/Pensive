//! A notice in the window's corner, outlined in its tone, with a line along its bottom that
//! drains until it folds away.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, App, BoxShadow, ElementId, SharedString, Window, div, hsla,
    prelude::*, px, relative,
};
use theme::{ActiveTheme, Typeset};

use crate::{Callback, IconButton, IconName, Tag, Tone, callback};

/// Wide enough for a path in its detail before it wraps.
const WIDTH: f32 = 340.;

#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    tone: Tone,
    title: SharedString,
    tag: Option<Tag>,
    detail: Option<SharedString>,
    timer: Option<(f32, Option<Duration>)>,
    on_close: Option<Callback<()>>,
    on_hover: Option<Callback<bool>>,
}

impl Toast {
    pub fn new(id: impl Into<ElementId>, tone: Tone, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            tone,
            title: title.into(),
            tag: None,
            detail: None,
            timer: None,
            on_close: None,
            on_hover: None,
        }
    }

    /// Follows the title, such as where a failure came from.
    pub fn tag(mut self, tag: Tag) -> Self {
        self.tag = Some(tag);
        self
    }

    /// A second line in the code font, such as the error's message.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The line along the bottom: `left` of the toast's time, from 1 to 0, draining to nothing
    /// over `draining`, or holding while that's `None`.
    pub fn timer(mut self, left: f32, draining: Option<Duration>) -> Self {
        self.timer = Some((left, draining));
        self
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(callback(handler));
        self
    }

    pub fn on_hover(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(std::rc::Rc::new(handler));
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let hue = self.tone.mark(colors).map(|(_, hue)| hue);
        let line = self.timer.zip(hue).map(|((left, draining), hue)| {
            let line = div().h_full().bg(hue).w(relative(left));
            // Inset by the corner radius, so the line stays clear of the rounded corners.
            div()
                .absolute()
                .bottom_0()
                .left(density.radius_lg)
                .right(density.radius_lg)
                .h(px(1.))
                .child(match draining {
                    Some(duration) => line
                        .with_animation(
                            ("timer", (left * 10_000.) as usize),
                            Animation::new(duration),
                            move |line, delta| line.w(relative(left * (1. - delta))),
                        )
                        .into_any_element(),
                    None => line.into_any_element(),
                })
        });
        div()
            .id(self.id)
            .relative()
            .flex()
            .gap_2p5()
            .w(px(WIDTH))
            .py_2p5()
            .pl_3()
            .pr_2p5()
            .overflow_hidden()
            .rounded(density.radius_lg)
            .border_1()
            .border_color(self.tone.outline(colors))
            .bg(colors.surface_raised)
            .shadow(vec![
                BoxShadow::new(px(0.), px(8.), hsla(0., 0., 0., 0.55)).blur_radius(px(24.)),
            ])
            .children(
                self.tone
                    .icon(16., colors)
                    .map(|icon| div().mt_0p5().child(icon)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .typeset(density.body)
                            .text_color(colors.text_strong)
                            .child(self.title)
                            .children(self.tag),
                    )
                    .children(self.detail.map(|detail| {
                        div()
                            .font_family(theme.fonts.code.clone())
                            .typeset(density.small)
                            .text_color(colors.text_muted)
                            .child(detail)
                    })),
            )
            .child(
                IconButton::new("close", IconName::X)
                    .size(px(20.))
                    .color(|colors| colors.text_faint)
                    .when_some(self.on_close, |button, handler| {
                        button.on_click(move |window, cx| handler((), window, cx))
                    }),
            )
            .children(line)
            .when_some(self.on_hover, |toast, handler| {
                toast.on_hover(move |hovered, window, cx| handler(*hovered, window, cx))
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("toast", "gallery", || {
            crate::Gallery::new("Toast")
                .section(
                    "Tones",
                    [
                        (
                            "Error, half its time left",
                            Toast::new("error", Tone::Error, "Couldn't read changes")
                                .tag(Tag::new("Fixture").accent())
                                .detail(
                                    "~/code/harbor-wt/tooltip-clip is not in a sample repository",
                                )
                                .timer(0.55, None)
                                .into_any_element(),
                        ),
                        (
                            "Success",
                            Toast::new("success", Tone::Success, "Cloned harbor")
                                .tag(Tag::new("Git"))
                                .detail("~/code/harbor")
                                .into_any_element(),
                        ),
                    ],
                )
                .into_any_element()
        })
        .size(size(px(960.), px(320.))),
    ]
}

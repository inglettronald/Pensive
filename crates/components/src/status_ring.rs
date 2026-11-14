//! A ring in a status color that fills to show how much of something is staged.

use gpui::{App, ElementId, Hsla, Window, div, prelude::*};
use strum::{IntoStaticStr, VariantArray};
use theme::{ActiveTheme, Dp, ThemeColors, dp};

use crate::{Callback, OnClick as _, callback};

#[derive(Clone, Copy, Debug, PartialEq, Eq, VariantArray, IntoStaticStr)]
pub enum RingFill {
    Empty,
    /// Some of it, such as a few of the uncommitted files.
    Half,
    Full,
}

#[derive(IntoElement)]
pub struct StatusRing {
    id: ElementId,
    fill: RingFill,
    color: fn(&ThemeColors) -> Hsla,
    size: Dp,
    on_click: Option<Callback<()>>,
}

impl StatusRing {
    /// A 10 px ring with a 2 px stroke in `text_muted`.
    pub fn new(id: impl Into<ElementId>, fill: RingFill) -> Self {
        Self {
            id: id.into(),
            fill,
            color: |colors| colors.text_muted,
            size: dp(10.),
            on_click: None,
        }
    }

    /// Picks the color role, such as `|colors| colors.diff_added`.
    pub fn color(mut self, color: fn(&ThemeColors) -> Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn size(mut self, size: Dp) -> Self {
        self.size = size;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(callback(handler));
        self
    }
}

impl RenderOnce for StatusRing {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = (self.color)(&cx.theme().colors);
        let stroke = dp(2.);
        let inner = self.size - stroke * 2.;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .size(self.size)
            .rounded_full()
            .border(stroke)
            .border_color(color)
            .when(self.fill == RingFill::Full, |ring| ring.bg(color))
            .when(self.fill == RingFill::Half, |ring| {
                ring.child(div().w(inner / 2.).h(inner).rounded_l(inner / 2.).bg(color))
            })
            .when(self.on_click.is_some(), |ring| ring.cursor_pointer())
            .on_click_callback(self.on_click)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    #[derive(Clone, Copy, VariantArray, IntoStaticStr)]
    enum Status {
        Added,
        Modified,
        Removed,
    }

    impl Status {
        fn color(self) -> fn(&ThemeColors) -> Hsla {
            match self {
                Status::Added => |colors| colors.diff_added,
                Status::Modified => |colors| colors.diff_modified,
                Status::Removed => |colors| colors.diff_removed,
            }
        }
    }

    vec![
        mock::Mock::component("status_ring", "gallery", || {
            let ring = |status: Status, fill: RingFill, size: f32| {
                StatusRing::new("ring", fill)
                    .color(status.color())
                    .size(dp(size))
                    .into_any_element()
            };
            crate::Gallery::new("StatusRing")
                .matrix(
                    "File, 10 px",
                    RingFill::VARIANTS,
                    Status::VARIANTS,
                    |fill, status| ring(status, fill, 10.),
                )
                .matrix(
                    "Node, 14 px",
                    RingFill::VARIANTS,
                    Status::VARIANTS,
                    |fill, status| ring(status, fill, 14.),
                )
                .into_any_element()
        })
        .size(640., 420.),
    ]
}

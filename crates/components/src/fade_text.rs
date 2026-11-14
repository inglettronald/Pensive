//! One line of text that fades out at its end when it doesn't fit, instead of ending in an
//! ellipsis.

use gpui::{
    App, Bounds, Hsla, Pixels, SharedString, Window, canvas, div, fill, linear_color_stop,
    linear_gradient, point, prelude::*, size,
};
use theme::{ActiveTheme, Dp, ThemeColors, dp};

/// How far before the end the fade starts.
const FADE: Dp = dp(28.);

#[derive(IntoElement)]
pub struct FadeText {
    text: SharedString,
    background: fn(&ThemeColors) -> Hsla,
}

impl FadeText {
    /// Fades into `surface_panel` unless told otherwise.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            background: |colors| colors.surface_panel,
        }
    }

    /// The color behind the text, which the fade ends in.
    pub fn background(mut self, background: fn(&ThemeColors) -> Hsla) -> Self {
        self.background = background;
        self
    }
}

impl RenderOnce for FadeText {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let background = (self.background)(&cx.theme().colors);
        let text = self.text.clone();
        let len = text.len();
        div()
            .relative()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .child(self.text)
            .child(
                canvas(
                    move |bounds, window, _| {
                        // Measured with the inherited text style, so it matches what's drawn.
                        let style = window.text_style();
                        let font_size = style.font_size.to_pixels(window.rem_size());
                        let line = window.text_system().shape_line(
                            text,
                            font_size,
                            &[style.to_run(len)],
                            None,
                        );
                        line.width > bounds.size.width
                    },
                    move |bounds, overflows, window, _| {
                        if overflows {
                            paint_fade(bounds, Edge::End, background, window);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}

/// Which edge of a box text runs past.
pub(crate) enum Edge {
    Start,
    End,
}

/// Paints `background` over the last stretch of `bounds` toward `edge`, from clear to solid.
pub(crate) fn paint_fade(
    bounds: Bounds<Pixels>,
    edge: Edge,
    background: Hsla,
    window: &mut Window,
) {
    let width = FADE.to_pixels(window.rem_size()).min(bounds.size.width);
    let (left, angle) = match edge {
        Edge::Start => (bounds.left(), 270.),
        Edge::End => (bounds.right() - width, 90.),
    };
    window.paint_quad(fill(
        Bounds::new(point(left, bounds.top()), size(width, bounds.size.height)),
        linear_gradient(
            angle,
            linear_color_stop(background.opacity(0.), 0.),
            // Solid a few pixels early, so no sliver of a glyph shows at the edge.
            linear_color_stop(background, 0.85),
        ),
    ));
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::px;

    use crate::gallery::Panel;

    vec![
        mock::Mock::component("fade_text", "gallery", || {
            let line = |text: &'static str| {
                Panel::new(dp(200.), [div().px_2().child(FadeText::new(text))]).into_any_element()
            };
            crate::Gallery::new("FadeText")
                .section(
                    "Fits or not",
                    [
                        ("Fits, no fade", line("feat: add the shell")),
                        (
                            "Too long, fades out",
                            line("feat: open the command palette with Ctrl K"),
                        ),
                    ],
                )
                .into_any_element()
        })
        .size(gpui::size(px(640.), px(200.))),
    ]
}

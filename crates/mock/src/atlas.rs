//! One subject's mocks as a grid of numbered cards, drawn into a single image.
#![expect(
    clippy::disallowed_methods,
    reason = "the atlas is an image laid out in pixels"
)]

use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use gpui::{AppContext, Result};
use gpui::{
    Context, IntoElement, ParentElement, Pixels, Render, RenderImage, SharedString, Size, Styled,
    Window, div, img, px, size,
};
use image::{Frame, RgbaImage};
use theme::{ActiveTheme, Dp, Typeset};

use crate::Mock;

const PADDING: Pixels = px(32.);
const GAP: Pixels = px(32.);
const TITLE_HEIGHT: Pixels = px(48.);
/// Includes the divider under it, because GPUI boxes size their borders inward.
const HEADER_HEIGHT: Pixels = px(40.);
const BORDER: Pixels = px(1.);

/// Fills rows left to right in the order mocks are added, with about as many columns as rows.
pub struct Atlas {
    subject: &'static str,
    cards: Vec<Card>,
}

#[derive(Clone)]
struct Card {
    name: &'static str,
    size: Size<Pixels>,
    image: Arc<RenderImage>,
}

impl Atlas {
    pub fn new(subject: &'static str) -> Self {
        Self {
            subject,
            cards: Vec::new(),
        }
    }

    /// Adds a mock with the image its `render` returned.
    pub fn push(&mut self, mock: &Mock, mut image: RgbaImage) {
        for pixel in image.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        self.cards.push(Card {
            name: mock.name,
            size: mock.size,
            image: Arc::new(RenderImage::new(vec![Frame::new(image)])),
        });
    }

    fn columns(&self) -> usize {
        columns(self.cards.len())
    }

    fn largest_mock(&self) -> Size<Pixels> {
        largest(self.cards.iter().map(|card| card.size))
    }

    fn card_size(&self) -> Size<Pixels> {
        let largest = self.largest_mock();
        size(
            largest.width + BORDER * 2.,
            HEADER_HEIGHT + largest.height + BORDER * 2.,
        )
    }

    fn summary(&self) -> SharedString {
        summary(self.subject, self.cards.len(), self.largest_mock())
    }
}

#[cfg(not(target_family = "wasm"))]
impl Atlas {
    /// Renders like `Mock::render`, at twice its size so each mock keeps the pixels of its PNG.
    pub fn render(self) -> Result<Option<RgbaImage>> {
        let window_size = self.window_size();
        crate::headless::render_headless(window_size, |_, cx| cx.new(|_| self))
    }

    fn window_size(&self) -> Size<Pixels> {
        let columns = self.columns();
        let rows = self.cards.len().div_ceil(columns);
        let card_size = self.card_size();
        size(
            PADDING * 2. + card_size.width * columns as f32 + GAP * (columns - 1) as f32,
            PADDING * 2. + TITLE_HEIGHT + (GAP + card_size.height) * rows as f32,
        )
    }
}

/// About as many columns as rows, for a grid of `count` cards.
pub fn columns(count: usize) -> usize {
    (count as f64).sqrt().ceil().max(1.) as usize
}

/// The size every card leaves room for.
pub fn largest(sizes: impl IntoIterator<Item = Size<Pixels>>) -> Size<Pixels> {
    sizes
        .into_iter()
        .fold(Size::default(), |largest, size| largest.max(&size))
}

/// The line under a subject's name: how many mocks, the largest one's size and the command that
/// renders them.
pub fn summary(subject: &str, count: usize, largest: Size<Pixels>) -> SharedString {
    format!(
        "{count} mocks · {} × {} · cargo mocks {subject}/",
        largest.width.as_f32(),
        largest.height.as_f32(),
    )
    .into()
}

impl Render for Atlas {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let card_size = self.card_size();

        let render_card = |(index, card): (usize, &Card)| {
            div()
                .w(card_size.width)
                .h(card_size.height)
                .flex()
                .flex_col()
                .border(BORDER)
                .border_color(colors.border)
                .rounded_t(density.radius_md)
                .child(
                    div()
                        .h(HEADER_HEIGHT)
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .bg(colors.surface_raised)
                        .rounded_t(density.radius_md - Dp(BORDER.as_f32()))
                        .border_b(BORDER)
                        .border_color(colors.border_subtle)
                        .child(
                            div()
                                .typeset(density.small)
                                .text_color(colors.text_faint)
                                .child(format!("{:02}", index + 1)),
                        )
                        .child(
                            div()
                                .typeset(density.label)
                                .text_color(colors.text_strong)
                                .child(card.name),
                        ),
                )
                .child(
                    img(card.image.clone())
                        .w(card.size.width)
                        .h(card.size.height),
                )
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(GAP)
            .p(PADDING)
            .bg(colors.surface_app)
            .font_family(cx.theme().fonts.ui.clone())
            .child(
                div()
                    .h(TITLE_HEIGHT)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .typeset(density.heading)
                            .text_color(colors.text_strong)
                            .child(self.subject),
                    )
                    .child(
                        div()
                            .typeset(density.small)
                            .text_color(colors.text_faint)
                            .child(self.summary()),
                    ),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(self.columns() as u16)
                    .gap(GAP)
                    .children(self.cards.iter().enumerate().map(render_card)),
            )
    }
}

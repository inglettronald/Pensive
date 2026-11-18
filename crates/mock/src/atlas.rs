//! One subject's mocks as a grid of numbered cards, drawn into as few images as the GPU can read
//! back.
#![expect(
    clippy::disallowed_methods,
    reason = "the atlas is an image laid out in pixels"
)]

use std::sync::Arc;

use gpui::{
    AppContext, Context, IntoElement, ParentElement, Pixels, Render, RenderImage, Result,
    SharedString, Size, Styled, Window, div, img, px, size,
};
use image::{Frame, RgbaImage};
use theme::{ActiveTheme, Dp, Typeset};

use crate::{
    Mock,
    grid::{columns, largest, summary},
    headless::{Output, SCALE},
};

const PADDING: Pixels = px(32.);
const GAP: Pixels = px(32.);
const TITLE_HEIGHT: Pixels = px(48.);
/// Includes the divider under it, because GPUI boxes size their borders inward.
const HEADER_HEIGHT: Pixels = px(40.);
const BORDER: Pixels = px(1.);

/// The most a headless render can read back from the GPU: wgpu's default buffer limit.
const MAX_IMAGE_BYTES: u64 = 256 << 20;

/// Fills rows left to right in the order mocks are added, with about as many columns as rows.
/// A subject too large for one image splits into pages.
pub struct Atlas {
    subject: &'static str,
    cards: Vec<Card>,
}

/// One image of an atlas: a run of its cards, numbered from `first`.
struct Page {
    subject: &'static str,
    cards: Vec<Card>,
    first: usize,
    number: usize,
    pages: usize,
    total: usize,
    largest: Size<Pixels>,
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

    /// Renders each page like `Mock::render`, at twice its size so each mock keeps the pixels of
    /// its PNG.
    pub fn render(self) -> Result<Option<Vec<RgbaImage>>> {
        self.pages()
            .into_iter()
            .map(|page| {
                let window_size = grid_size(page.cards.len(), page.largest);
                let capture =
                    crate::headless::capture(window_size, Output::Image, |_, cx| cx.new(|_| page))?;
                Ok(capture.map(|capture| capture.image))
            })
            .collect::<Result<Option<Vec<_>>>>()
    }

    /// Splits the cards into as few pages as fit in one image each. Every page but the last holds
    /// the same number of cards.
    fn pages(self) -> Vec<Page> {
        let total = self.cards.len();
        let largest = largest(self.cards.iter().map(|card| card.size));
        let fitting = (1..=total)
            .rev()
            .find(|&count| image_bytes(grid_size(count, largest)) <= MAX_IMAGE_BYTES)
            .unwrap_or(1);
        let pages = total.div_ceil(fitting);
        let per_page = total.div_ceil(pages);
        self.cards
            .chunks(per_page)
            .enumerate()
            .map(|(index, cards)| Page {
                subject: self.subject,
                cards: cards.to_vec(),
                first: index * per_page,
                number: index + 1,
                pages,
                total,
                largest,
            })
            .collect()
    }
}

/// The window an atlas of `count` cards needs, each with room for a `largest` mock.
fn grid_size(count: usize, largest: Size<Pixels>) -> Size<Pixels> {
    let columns = columns(count);
    let rows = count.div_ceil(columns);
    let card = card_size(largest);
    size(
        PADDING * 2. + card.width * columns as f32 + GAP * (columns - 1) as f32,
        PADDING * 2. + TITLE_HEIGHT + (GAP + card.height) * rows as f32,
    )
}

fn card_size(largest: Size<Pixels>) -> Size<Pixels> {
    size(
        largest.width + BORDER * 2.,
        HEADER_HEIGHT + largest.height + BORDER * 2.,
    )
}

/// The bytes a window of `size` reads back as, with each row padded to wgpu's copy alignment.
fn image_bytes(size: Size<Pixels>) -> u64 {
    let width = (size.width.as_f32() * SCALE).ceil() as u64;
    let height = (size.height.as_f32() * SCALE).ceil() as u64;
    (width * 4).next_multiple_of(256) * height
}

impl Page {
    fn summary(&self) -> SharedString {
        let summary = summary(self.subject, self.total, self.largest);
        match self.pages {
            1 => summary,
            pages => format!("{summary} · page {} of {pages}", self.number).into(),
        }
    }
}

impl Render for Page {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let card_size = card_size(self.largest);
        let first = self.first;

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
                                .child(format!("{:02}", first + index + 1)),
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
                    .grid_cols(columns(self.cards.len()) as u16)
                    .gap(GAP)
                    .children(self.cards.iter().enumerate().map(render_card)),
            )
    }
}

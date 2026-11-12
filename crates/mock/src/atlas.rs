//! One subject's rendered mocks as a grid of numbered cards, rendered to a single image.

use std::sync::Arc;

use gpui::{
    AppContext, Context, FontWeight, IntoElement, ParentElement, Pixels, Render, RenderImage,
    Result, SharedString, Size, Styled, Window, div, img, px, size,
};
use image::{Frame, RgbaImage};
use theme::{ActiveTheme, Typeset};

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

    pub fn subject(&self) -> &'static str {
        self.subject
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Adds a mock with the image its `render` returned.
    pub fn push(&mut self, mock: &Mock, image: &RgbaImage) {
        let mut bgra = image.clone();
        for pixel in bgra.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        self.cards.push(Card {
            name: mock.name,
            size: mock.size,
            image: Arc::new(RenderImage::new(vec![Frame::new(bgra)])),
        });
    }

    /// Renders at twice its size, so each mock keeps the pixels of its own PNG. Returns `None`
    /// on platforms with no headless renderer.
    pub fn render(self) -> Result<Option<RgbaImage>> {
        let window_size = self.window_size();
        crate::render_headless(window_size, |_, cx| cx.new(|_| self))
    }

    fn columns(&self) -> usize {
        (self.cards.len() as f64).sqrt().ceil().max(1.) as usize
    }

    fn largest_mock(&self) -> Size<Pixels> {
        self.cards.iter().fold(Size::default(), |largest, card| {
            size(
                largest.width.max(card.size.width),
                largest.height.max(card.size.height),
            )
        })
    }

    fn window_size(&self) -> Size<Pixels> {
        let columns = self.columns();
        let rows = self.cards.len().div_ceil(columns);
        let mock = self.largest_mock();
        let card = size(
            mock.width + BORDER * 2.,
            HEADER_HEIGHT + mock.height + BORDER * 2.,
        );
        size(
            PADDING * 2. + card.width * columns as f32 + GAP * (columns - 1) as f32,
            PADDING * 2. + TITLE_HEIGHT + (GAP + card.height) * rows as f32,
        )
    }

    fn summary(&self) -> SharedString {
        let mock = self.largest_mock();
        format!(
            "{} mocks  ·  {} × {}  ·  cargo mocks {}",
            self.cards.len(),
            mock.width.as_f32(),
            mock.height.as_f32(),
            self.subject,
        )
        .into()
    }
}

impl Render for Atlas {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let summary = self.summary();
        let columns = self.columns();
        let mut cards = std::mem::take(&mut self.cards).into_iter().enumerate();
        let rows = std::iter::from_fn(|| {
            let row: Vec<_> = cards.by_ref().take(columns).collect();
            (!row.is_empty()).then_some(row)
        });

        let card = |(index, card): (usize, Card)| {
            div()
                .flex()
                .flex_col()
                .border(BORDER)
                .border_color(colors.border)
                .rounded(density.radius_md)
                .overflow_hidden()
                .child(
                    div()
                        .h(HEADER_HEIGHT)
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .bg(colors.surface_raised)
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
                                .typeset(density.body)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(colors.text_strong)
                                .child(card.name),
                        ),
                )
                .child(img(card.image).w(card.size.width).h(card.size.height))
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
                            .child(summary),
                    ),
            )
            .children(rows.map(|row| div().flex().gap(GAP).children(row.into_iter().map(card))))
    }
}

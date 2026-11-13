//! A raised box of rows split by hairlines, each row filling on hover, such as a settings page's
//! settings.

use gpui::{AnyElement, App, ElementId, Window, div, prelude::*, px};
use theme::ActiveTheme;

#[derive(IntoElement)]
pub struct Card {
    id: ElementId,
    rows: Vec<AnyElement>,
    hovered: Option<usize>,
}

impl Card {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            rows: Vec::new(),
            hovered: None,
        }
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.rows
            .extend(rows.into_iter().map(IntoElement::into_any_element));
        self
    }

    /// Shows the hover state without a pointer, for mocks.
    pub fn hovered(mut self, hovered: Option<usize>) -> Self {
        self.hovered = hovered;
        self
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        // Inside the border, so a filled row keeps the card's corners.
        let corner = cx.density().radius_lg - px(1.);
        let last = self.rows.len().saturating_sub(1);
        let rows = self.rows.into_iter().enumerate().map(|(index, row)| {
            div()
                .id(index)
                .when(index > 0, |row| {
                    row.border_t_1().border_color(colors.border_subtle)
                })
                .when(index == 0, |row| row.rounded_t(corner))
                .when(index == last, |row| row.rounded_b(corner))
                .map(|row| {
                    if self.hovered == Some(index) {
                        row.bg(colors.surface_raised)
                    } else {
                        row.hover(|row| row.bg(colors.surface_raised))
                    }
                })
                .child(row)
        });
        div()
            .id(self.id)
            .flex()
            .flex_col()
            .rounded(cx.density().radius_lg)
            .border_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_panel)
            .children(rows)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    fn line(text: &'static str) -> impl IntoElement {
        div().px_3().py_2p5().child(text)
    }

    vec![
        mock::Mock::component("card", "gallery", || {
            crate::Gallery::new("Card")
                .section(
                    "Rows",
                    [
                        (
                            "One row",
                            div()
                                .w(px(280.))
                                .child(Card::new("one").rows([line("Theme")]))
                                .into_any_element(),
                        ),
                        (
                            "Hovering the second",
                            div()
                                .w(px(280.))
                                .child(
                                    Card::new("three")
                                        .rows([line("Tabs"), line("Starting tab"), line("Opacity")])
                                        .hovered(Some(1)),
                                )
                                .into_any_element(),
                        ),
                    ],
                )
                .into_any_element()
        })
        .size(size(px(700.), px(260.))),
    ]
}

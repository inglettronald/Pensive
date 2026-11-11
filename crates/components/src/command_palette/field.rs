//! The palette's search row: keyword chips, the query, and the tab strip.

use gpui::{AnyElement, Div, SharedString, div, prelude::*, px};
use theme::{Density, Theme, Typeset};

use crate::{Icon, IconName, TabStrip};

/// A keyword that narrows the results, such as `project:` `harbor`. Drawn sunken, so it reads
/// as the scope the query is typed into.
pub struct PaletteChip {
    keyword: SharedString,
    value: Option<SharedString>,
    removable: bool,
}

impl PaletteChip {
    pub fn new(keyword: impl Into<SharedString>) -> Self {
        Self {
            keyword: keyword.into(),
            value: None,
            removable: false,
        }
    }

    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Shows an x, for a chip the user added rather than one the palette opened with.
    pub fn removable(mut self) -> Self {
        self.removable = true;
        self
    }

    fn render(self, theme: &Theme, density: &Density) -> Div {
        let colors = &theme.colors;
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .h(px(24.))
            .px_2()
            .rounded(density.radius_sm)
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface_app)
            .typeset(density.small)
            .whitespace_nowrap()
            .child(
                div()
                    .font_family(theme.fonts.code.clone())
                    .text_color(colors.text_muted)
                    .child(self.keyword),
            )
            .children(
                self.value
                    .map(|value| div().text_color(colors.text_strong).child(value)),
            )
            .when(self.removable, |chip| {
                chip.child(
                    Icon::new(IconName::X)
                        .size(px(12.))
                        .color(colors.text_faint),
                )
            })
    }
}

pub(super) struct Field {
    pub chips: Vec<PaletteChip>,
    pub query: SharedString,
    pub placeholder: SharedString,
    pub input: Option<AnyElement>,
    pub tabs: Option<TabStrip>,
}

impl Field {
    pub fn render(self, theme: &Theme, density: &Density) -> Div {
        let colors = &theme.colors;
        let (text, color) = match (&self.input, self.query.is_empty()) {
            (None, true) => (self.placeholder, colors.text_faint),
            _ => (self.query, colors.text_strong),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(density.row_height * 2. - px(8.))
            .pl_4()
            .pr_3()
            .border_b_1()
            .border_color(colors.border_subtle)
            .child(
                div()
                    .mr_1()
                    .child(Icon::new(IconName::Search).color(colors.text_faint)),
            )
            .children(
                self.chips
                    .into_iter()
                    .map(|chip| chip.render(theme, density)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .typeset(density.body)
                    .text_color(color)
                    .map(|field| match self.input {
                        Some(input) => field.child(input),
                        None => field.child(text),
                    }),
            )
            .children(self.tabs)
    }
}

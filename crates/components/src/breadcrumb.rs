//! Levels of a location separated by chevrons, such as project › branch or a file's path.

use std::{cell::Cell, rc::Rc};

use gpui::{AnyElement, App, Hsla, Pixels, Window, canvas, div, prelude::*};
use theme::{ActiveTheme, Dp, ThemeColors, dp};

use crate::{
    Icon, IconName,
    fade_text::{Edge, paint_fade},
};

#[derive(IntoElement)]
pub struct Breadcrumb {
    crumbs: Vec<AnyElement>,
    chevron: Dp,
    fade_start: Option<fn(&ThemeColors) -> Hsla>,
}

impl Breadcrumb {
    /// Each crumb draws itself, so a crumb can be plain text or a button.
    pub fn new(crumbs: impl IntoIterator<Item = impl IntoElement>) -> Self {
        Self {
            crumbs: crumbs
                .into_iter()
                .map(IntoElement::into_any_element)
                .collect(),
            chevron: dp(12.),
            fade_start: None,
        }
    }

    /// When the crumbs don't fit, the first ones run off the start and fade into `background`,
    /// so the last crumb always shows.
    pub fn fade_start(mut self, background: fn(&ThemeColors) -> Hsla) -> Self {
        self.fade_start = Some(background);
        self
    }

    /// The chevron's size, 12 px unless set.
    pub fn chevron_size(mut self, size: Dp) -> Self {
        self.chevron = size;
        self
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let faint = cx.theme().colors.text_faint;
        let mut crumbs = Vec::with_capacity(self.crumbs.len() * 2);
        for (index, crumb) in self.crumbs.into_iter().enumerate() {
            if index > 0 {
                crumbs.push(
                    Icon::new(IconName::ChevronRight)
                        .size(self.chevron)
                        .color(faint)
                        .into_any_element(),
                );
            }
            crumbs.push(crumb);
        }
        let row = div()
            .flex()
            .items_center()
            .gap_1()
            .min_w_0()
            .whitespace_nowrap();
        let Some(background) = self.fade_start else {
            return row.children(crumbs);
        };
        let background = background(&cx.theme().colors);
        let width = Rc::new(Cell::new(Pixels::ZERO));
        let measured = width.clone();
        // Laid out in reverse, so crumbs that don't fit run off the start rather than the end,
        // and the spacer keeps crumbs that fit at the start.
        row.relative()
            .flex_row_reverse()
            .overflow_hidden()
            .child(div().flex_1())
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_1()
                    .children(crumbs)
                    .child(
                        canvas(
                            move |bounds, _, _| measured.set(bounds.size.width),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    ),
            )
            .child(
                canvas(
                    move |bounds, _, _| width.get() > bounds.size.width,
                    move |bounds, overflows, window, _| {
                        if overflows {
                            paint_fade(bounds, Edge::Start, background, window);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("breadcrumb", "gallery", || {
            crate::Gallery::new("Breadcrumb")
                .section(
                    "Crumbs",
                    [
                        (
                            "Project and branch",
                            Breadcrumb::new(["pensive", "feat/command-palette"]),
                        ),
                        (
                            "A file's path",
                            Breadcrumb::new(["crates", "components", "src", "palette.rs"]),
                        ),
                        ("One level", Breadcrumb::new(["docs"])),
                    ]
                    .map(|(label, crumb)| (label, crumb.into_any_element())),
                )
                .section(
                    "Fading at the start",
                    [
                        (
                            "Fits",
                            Breadcrumb::new(["crates", "ui", "row.rs"])
                                .fade_start(|colors| colors.surface_app),
                        ),
                        (
                            "Too long",
                            Breadcrumb::new(["crates", "ui", "src", "project", "timeline.rs"])
                                .fade_start(|colors| colors.surface_app),
                        ),
                    ]
                    .map(|(label, crumb)| {
                        (
                            label,
                            div().w(dp(150.)).flex().child(crumb).into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .size(640., 300.),
    ]
}

//! The card at the end of a settings category that leads on to the next one.

use gpui::{App, ElementId, SharedString, Window, div, prelude::*, px};
use theme::{ActiveTheme, Typeset};

use crate::{Callback, Icon, IconName, callback};

#[derive(IntoElement)]
pub struct NextCard {
    id: ElementId,
    icon: IconName,
    name: SharedString,
    description: SharedString,
    hovered: bool,
    on_click: Option<Callback<()>>,
}

impl NextCard {
    pub fn new(
        id: impl Into<ElementId>,
        icon: IconName,
        name: impl Into<SharedString>,
        description: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            icon,
            name: name.into(),
            description: description.into(),
            hovered: false,
            on_click: None,
        }
    }

    /// Shows the hover state without a pointer, for mocks.
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(callback(handler));
        self
    }
}

impl RenderOnce for NextCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let tile = density.row_height + px(4.);
        let fill = if self.hovered {
            colors.surface_raised
        } else {
            colors.surface_panel
        };
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_3()
            .mt_6()
            .px_3p5()
            .py_3()
            .rounded(density.radius_lg)
            .border_1()
            .border_color(colors.border_subtle)
            .bg(fill)
            .hover(|card| card.bg(colors.surface_raised))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(tile)
                    .rounded(density.radius_md)
                    .border_1()
                    .border_color(colors.border_subtle)
                    .bg(colors.surface_raised)
                    .child(Icon::new(self.icon).color(colors.text_muted)),
            )
            .child(
                div()
                    .flex_1()
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .child("Next")
                    .child(
                        div()
                            .typeset(density.body)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.text_strong)
                            .child(self.name),
                    )
                    .child(self.description),
            )
            .child(Icon::new(IconName::ChevronRight).color(colors.text_faint))
            .when_some(self.on_click, |card, handler| {
                card.on_click(move |_, window, cx| handler((), window, cx))
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    fn card(hovered: bool) -> gpui::AnyElement {
        div()
            .w(px(560.))
            .child(
                NextCard::new(
                    "next",
                    IconName::Command,
                    "Command palette",
                    "What Ctrl K opens on, and how it sits over a window",
                )
                .hovered(hovered),
            )
            .into_any_element()
    }
    vec![
        mock::Mock::component("next_card", "gallery", || {
            crate::Gallery::new("NextCard")
                .section(
                    "States",
                    [("Resting", card(false)), ("Hovered", card(true))],
                )
                .into_any_element()
        })
        .size(size(px(640.), px(360.))),
    ]
}

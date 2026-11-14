//! The headings on the settings page: a category's icon tile, name and description, the same
//! smaller for a search result, and a page's title under a rule.

use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div};
use theme::{ActiveTheme, Typeset, dp};

use crate::{Icon, IconName};

#[derive(IntoElement)]
pub struct CategoryHeading {
    icon: IconName,
    name: SharedString,
    description: SharedString,
    compact: bool,
}

impl CategoryHeading {
    pub fn new(
        icon: IconName,
        name: impl Into<SharedString>,
        description: impl Into<SharedString>,
    ) -> Self {
        Self {
            icon,
            name: name.into(),
            description: description.into(),
            compact: false,
        }
    }

    /// Just a small icon and the name, for a category that's not the one being read, such as in
    /// search results.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl RenderOnce for CategoryHeading {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        if self.compact {
            return div()
                .flex()
                .items_center()
                .gap_2()
                .pt_4()
                .pb_0p5()
                .px(density.row_padding)
                .typeset(density.small)
                .text_color(colors.text_faint)
                .child(Icon::new(self.icon).size(dp(14.)).color(colors.text_faint))
                .child(self.name);
        }
        let tile = density.row_height + dp(4.);
        div()
            .flex()
            .items_start()
            .gap_3()
            .pt_7()
            .pb_1p5()
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
                    .child(
                        div()
                            .typeset(density.heading)
                            .text_color(colors.text_strong)
                            .child(self.name),
                    )
                    .child(
                        div()
                            .typeset(density.small)
                            .text_color(colors.text_faint)
                            .child(self.description),
                    ),
            )
    }
}

#[derive(IntoElement)]
pub struct PageHeading {
    name: SharedString,
    rule: bool,
}

impl PageHeading {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            rule: true,
        }
    }

    /// Leaves out the rule, for the first page under a category's heading.
    pub fn first(mut self) -> Self {
        self.rule = false;
        self
    }
}

impl RenderOnce for PageHeading {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let heading = div()
            .ml(density.row_padding)
            .pb_1()
            .typeset(density.title)
            .text_color(colors.text_strong);
        if self.rule {
            heading
                .mt_3p5()
                .pt_3p5()
                .border_t_1()
                .border_color(colors.border_subtle)
                .child(self.name)
        } else {
            heading.mt_1().pt_2p5().child(self.name)
        }
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::{IntoElement, px, size};

    vec![
        mock::Mock::component("settings_heading", "gallery", || {
            div()
                .px_6()
                .w(dp(640.))
                .child(CategoryHeading::new(
                    IconName::Palette,
                    "Appearance",
                    "The theme, the size of everything, and fonts",
                ))
                .child(PageHeading::new("Theme").first())
                .child(PageHeading::new("Size"))
                .child(
                    CategoryHeading::new(IconName::Bell, "Notices", "Toasts and the badge")
                        .compact(),
                )
                .into_any_element()
        })
        .size(size(px(640.), px(260.))),
    ]
}

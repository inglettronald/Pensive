//! One setting in a card: its name and description, its control, and once it's changed, its
//! default and a button that goes back to it.

use std::ops::Range;

use gpui::{
    AnyElement, App, ElementId, HighlightStyle, SharedString, StyledText, Window, div, prelude::*,
    px,
};
use theme::{ActiveTheme, Typeset};

use crate::{Callback, IconButton, IconName, callback};

#[derive(IntoElement)]
pub struct SettingRow {
    id: ElementId,
    name: SharedString,
    description: SharedString,
    matches: Vec<Range<usize>>,
    control: AnyElement,
    default: Option<SharedString>,
    on_reset: Option<Callback<()>>,
}

impl SettingRow {
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        description: impl Into<SharedString>,
        control: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            matches: Vec::new(),
            control: control.into_any_element(),
            default: None,
            on_reset: None,
        }
    }

    /// Byte ranges of the name that matched a search.
    pub fn matches(mut self, matches: Vec<Range<usize>>) -> Self {
        self.matches = matches;
        self
    }

    /// Marks the setting as changed, showing the label of its default before the control.
    pub fn changed_from(mut self, default: Option<SharedString>) -> Self {
        self.default = default;
        self
    }

    pub fn on_reset(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reset = Some(callback(handler));
        self
    }
}

impl RenderOnce for SettingRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        // The reset button's slot is kept on every row, so the controls in a card line up.
        let slot = px(20.);
        let reset = match (&self.default, self.on_reset) {
            (Some(_), Some(handler)) => IconButton::new("reset", IconName::RotateCcw)
                .size(slot)
                .on_click(move |window, cx| handler((), window, cx))
                .into_any_element(),
            _ => div().flex_none().w(slot).into_any_element(),
        };
        let name =
            StyledText::new(self.name).with_highlights(self.matches.into_iter().map(|range| {
                (
                    range,
                    HighlightStyle {
                        color: Some(colors.accent_fg),
                        ..Default::default()
                    },
                )
            }));
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_4()
            .px(density.row_padding)
            .py((density.row_height - px(8.)) / 2.)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .typeset(density.body)
                            .text_color(colors.text)
                            .child(name),
                    )
                    .child(
                        div()
                            .typeset(density.small)
                            .text_color(colors.text_faint)
                            .child(self.description),
                    ),
            )
            .children(self.default.map(|default| {
                div()
                    .flex()
                    .flex_none()
                    .gap_1()
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .child("Default")
                    .child(div().text_color(colors.text_muted).child(default))
            }))
            .child(div().flex_none().child(self.control))
            .child(reset)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    use crate::{Card, SegmentedControl, Select, Toggle};

    vec![
        mock::Mock::component("setting_row", "gallery", || {
            div()
                .p_6()
                .w(px(640.))
                .child(
                    Card::new("rows").rows([
                        SettingRow::new(
                            "theme",
                            "Theme",
                            "The colors, and the fonts unless they're set below",
                            Select::new("theme-select", "Pensive Dark"),
                        )
                        .into_any_element(),
                        SettingRow::new(
                            "density",
                            "Density",
                            "Calm has larger text and 28 px rows",
                            SegmentedControl::new("density-control", ["Calm", "Dense"], Some(1)),
                        )
                        .changed_from(Some("Calm".into()))
                        .on_reset(|_, _| {})
                        .into_any_element(),
                        SettingRow::new(
                            "tabs",
                            "Show tabs",
                            "All, Active and Commands in the search field",
                            Toggle::new("tabs-toggle", true),
                        )
                        .matches(std::iter::once(0..4).collect())
                        .into_any_element(),
                    ]),
                )
                .into_any_element()
        })
        .size(size(px(640.), px(240.))),
    ]
}

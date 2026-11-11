//! One result row: what leads it, its label with fuzzy matches, and what trails it.

use std::ops::Range;

use gpui::{
    AnyElement, Div, FontWeight, HighlightStyle, Hsla, SharedString, Stateful, StyledText, div,
    prelude::*, px,
};
use theme::{Density, Theme, ThemeColors, Typeset};

use super::KeyHint;
use crate::{Icon, IconName, Kbd};

/// What sits at the start of a row.
#[derive(Clone)]
pub enum Leading {
    /// A letter tile, the default project icon.
    Tile(SharedString),
    Icon(IconName),
}

/// Text on the right of a row, before the time. Gray unless a scope has a reason for color.
pub struct TrailingText {
    text: SharedString,
    color: fn(&ThemeColors) -> Hsla,
    code: bool,
}

impl TrailingText {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            color: |colors| colors.text_faint,
            code: false,
        }
    }

    /// Picks the color role, such as `|colors| colors.diff_added`.
    pub fn color(mut self, color: fn(&ThemeColors) -> Hsla) -> Self {
        self.color = color;
        self
    }

    /// In the code font, for branch names.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }
}

pub struct PaletteEntry {
    leading: Option<Leading>,
    label: SharedString,
    code: bool,
    matches: Vec<Range<usize>>,
    detail: Option<SharedString>,
    description: Option<SharedString>,
    trailing: Vec<TrailingText>,
    when: Option<SharedString>,
    kbd: Option<SharedString>,
    control: Option<AnyElement>,
    pub(super) hints: Option<Vec<KeyHint>>,
}

impl PaletteEntry {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            leading: None,
            label: label.into(),
            code: false,
            matches: Vec::new(),
            detail: None,
            description: None,
            trailing: Vec::new(),
            when: None,
            kbd: None,
            control: None,
            hints: None,
        }
    }

    pub fn leading(mut self, leading: Leading) -> Self {
        self.leading = Some(leading);
        self
    }

    pub fn tile(mut self, letter: impl Into<SharedString>) -> Self {
        self.leading = Some(Leading::Tile(letter.into()));
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.leading = Some(Leading::Icon(icon));
        self
    }

    /// Draws the label in the code font, for branch and worktree names.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// Byte ranges of the label that matched the query.
    pub fn matches(mut self, matches: impl IntoIterator<Item = Range<usize>>) -> Self {
        self.matches = matches.into_iter().collect();
        self
    }

    /// Gray text after the label on the same line, such as a path or a project name.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A second line, for information worth the extra height.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn trailing(mut self, text: TrailingText) -> Self {
        self.trailing.push(text);
        self
    }

    /// Time since the last change, such as `4m`.
    pub fn when(mut self, when: impl Into<SharedString>) -> Self {
        self.when = Some(when.into());
        self
    }

    /// The command's own shortcut.
    pub fn kbd(mut self, keys: impl Into<SharedString>) -> Self {
        self.kbd = Some(keys.into());
        self
    }

    /// A control drawn at the end of the row, such as a setting's toggle.
    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    /// Replaces the footer's default `Enter Run` while this entry is selected.
    pub fn hints(mut self, hints: impl IntoIterator<Item = KeyHint>) -> Self {
        self.hints = Some(hints.into_iter().collect());
        self
    }

    pub(super) fn render(
        self,
        index: usize,
        selected: bool,
        theme: &Theme,
        density: &Density,
    ) -> Stateful<Div> {
        let colors = &theme.colors;
        let height = if self.description.is_some() {
            density.row_height + px(16.)
        } else {
            density.row_height
        };
        let highlight = HighlightStyle {
            color: Some(colors.accent_fg),
            font_weight: Some(FontWeight::SEMIBOLD),
            ..Default::default()
        };
        let label = StyledText::new(self.label)
            .with_highlights(self.matches.into_iter().map(|range| (range, highlight)));
        let font = |code: bool| {
            if code {
                theme.fonts.code.clone()
            } else {
                theme.fonts.ui.clone()
            }
        };

        div()
            .id(index)
            .flex()
            .flex_none()
            .items_center()
            .gap(density.row_gap)
            .h(height)
            .px_2()
            .rounded(density.radius_md)
            .when(selected, |row| row.bg(colors.surface_selected))
            .children(self.leading.map(|leading| {
                match leading {
                    Leading::Tile(letter) => div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .rounded(density.radius_sm)
                        .bg(if selected {
                            colors.border
                        } else {
                            colors.surface_selected
                        })
                        .text_color(colors.text_strong)
                        .typeset(density.label)
                        .child(letter),
                    Leading::Icon(icon) => div()
                        .flex()
                        .flex_none()
                        .justify_center()
                        .w(px(20.))
                        .child(Icon::new(icon)),
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(
                                div()
                                    .flex_none()
                                    .whitespace_nowrap()
                                    .font_family(font(self.code))
                                    .typeset(density.body)
                                    .text_color(if selected {
                                        colors.text_strong
                                    } else {
                                        colors.text
                                    })
                                    .child(label),
                            )
                            .children(self.detail.map(|detail| {
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .typeset(density.small)
                                    .line_height(density.body.line_height)
                                    .text_color(colors.text_faint)
                                    .child(detail)
                            })),
                    )
                    .children(self.description.map(|description| {
                        div()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .typeset(density.small)
                            .text_color(colors.text_faint)
                            .child(description)
                    })),
            )
            .children(self.trailing.into_iter().map(|text| {
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .font_family(font(text.code))
                    .typeset(density.small)
                    .text_color((text.color)(colors))
                    .child(text.text)
            }))
            .children(self.when.map(|when| {
                div()
                    .flex_none()
                    .min_w(px(24.))
                    .text_right()
                    .whitespace_nowrap()
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .child(when)
            }))
            .children(self.kbd.map(Kbd::new))
            .children(self.control)
    }
}

//! A labeled button in one of four emphases, with an optional icon and shortcut.

use gpui::{App, ElementId, FontWeight, Hsla, SharedString, Window, div, prelude::*};
use strum::{IntoStaticStr, VariantArray};
use theme::{ActiveTheme, HAIRLINE, ThemeColors, Typeset, dp};

use crate::{Callback, Icon, IconName, OnClick as _, callback};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, VariantArray, IntoStaticStr)]
pub enum ButtonStyle {
    /// The brightest fill, for the one action a screen is for.
    Primary,
    /// Everyday actions.
    #[default]
    Secondary,
    /// Emphasis without a bright fill, so the button doesn't outshine content.
    Outline,
    /// The accent blue.
    Accent,
}

struct Tone {
    fill: Hsla,
    hover: Hsla,
    border: Hsla,
    label: Hsla,
    divider: Hsla,
    hint: Hsla,
}

impl ButtonStyle {
    fn tone(self, colors: &ThemeColors) -> Tone {
        match self {
            ButtonStyle::Primary => Tone {
                fill: colors.button_primary,
                hover: colors.text,
                border: colors.button_primary,
                label: colors.surface_app,
                divider: colors.text_muted,
                hint: colors.text_disabled,
            },
            ButtonStyle::Secondary => Tone {
                fill: colors.button_secondary,
                hover: colors.surface_selected,
                border: colors.border_subtle,
                label: colors.text,
                divider: colors.border,
                hint: colors.text_faint,
            },
            ButtonStyle::Outline => Tone {
                fill: colors.button_secondary,
                hover: colors.surface_selected,
                border: colors.text_muted,
                label: colors.text_strong,
                divider: colors.border,
                hint: colors.text_faint,
            },
            ButtonStyle::Accent => Tone {
                fill: colors.accent_solid,
                hover: colors.accent_solid_hover,
                border: colors.accent_solid,
                label: colors.text_strong,
                divider: colors.accent_border,
                hint: colors.text,
            },
        }
    }
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    style: ButtonStyle,
    icon: Option<IconName>,
    shortcut: Option<SharedString>,
    small: bool,
    disabled: bool,
    on_click: Option<Callback<()>>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            style: ButtonStyle::default(),
            icon: None,
            shortcut: None,
            small: false,
            disabled: false,
            on_click: None,
        }
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The keys that press it, shown inside after a divider so they read as this button's.
    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    /// 24 px tall instead of 28.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// Grayed out, without its shortcut, and ignoring clicks.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(callback(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let tone = self.style.tone(colors);
        let (fill, border, label) = if self.disabled {
            (
                colors.surface_raised,
                colors.border_subtle,
                colors.text_disabled,
            )
        } else {
            (tone.fill, tone.border, tone.label)
        };
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(if self.small { dp(24.) } else { dp(28.) })
            .when_else(self.small, |button| button.px_2(), |button| button.px_3())
            .rounded(if self.small {
                density.radius_sm
            } else {
                density.radius_md
            })
            .border_1()
            .border_color(border)
            .bg(fill)
            .text_color(label)
            .typeset(density.small)
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .children(
                self.icon
                    .map(|icon| Icon::new(icon).size(dp(12.)).color(label)),
            )
            .child(self.label)
            .when(!self.disabled, |button| {
                button
                    .hover(move |button| button.bg(tone.hover))
                    .when_some(self.shortcut, |button, keys| {
                        button
                            .child(div().w(HAIRLINE).h(dp(12.)).bg(tone.divider))
                            .child(
                                div()
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(tone.hint)
                                    .child(keys),
                            )
                    })
                    .on_click_callback(self.on_click)
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    #[derive(Clone, Copy, VariantArray, IntoStaticStr)]
    enum Shape {
        Plain,
        Shortcut,
        Icon,
        Small,
        Disabled,
    }

    vec![
        mock::Mock::component("button", "gallery", || {
            crate::Gallery::new("Button")
                .matrix(
                    "Styles",
                    ButtonStyle::VARIANTS,
                    Shape::VARIANTS,
                    |style, shape| {
                        let button = Button::new("button", "Commit 3 files").style(style);
                        match shape {
                            Shape::Plain => button,
                            Shape::Shortcut => button.shortcut("Ctrl Enter"),
                            Shape::Icon => Button::new("button", "2")
                                .style(style)
                                .icon(IconName::ArrowUpFromLine),
                            Shape::Small => Button::new("button", "2")
                                .style(style)
                                .icon(IconName::ArrowUpFromLine)
                                .small(),
                            Shape::Disabled => Button::new("button", "Nothing staged")
                                .style(style)
                                .disabled(true),
                        }
                        .into_any_element()
                    },
                )
                .into_any_element()
        })
        .size(1280., 320.),
    ]
}

//! A small raised box for what doesn't fit where it's anchored, or for how an action there went.

use gpui::{AnyElement, App, BoxShadow, Hsla, SharedString, Window, div, hsla, prelude::*, px};
use theme::{ActiveTheme, ThemeColors, Typeset};

use crate::{Icon, IconName, Kbd, Tag};

/// What a tooltip reports on. Every tone but `Neutral` leads with its icon and tints the outline
/// with its hue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Neutral,
    Success,
    Warning,
    Error,
}

impl Tone {
    /// The icon the tooltip leads with, and the hue of it and the outline.
    fn mark(self, colors: &ThemeColors) -> Option<(IconName, Hsla)> {
        match self {
            Tone::Neutral => None,
            Tone::Success => Some((IconName::Check, colors.diff_added)),
            Tone::Warning => Some((IconName::TriangleAlert, colors.diff_modified)),
            Tone::Error => Some((IconName::CircleX, colors.diff_removed)),
        }
    }
}

#[derive(IntoElement)]
pub struct Tooltip {
    content: AnyElement,
    tone: Tone,
    tag: Option<Tag>,
    detail: Option<SharedString>,
    shortcut: Option<(SharedString, SharedString)>,
}

impl Tooltip {
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            content: content.into_any_element(),
            tone: Tone::Neutral,
            tag: None,
            detail: None,
            shortcut: None,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Follows the content on its line, such as where a failure came from.
    pub fn tag(mut self, tag: Tag) -> Self {
        self.tag = Some(tag);
        self
    }

    /// A second line in muted text, which wraps where the first never does.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// An action the tooltip offers while it's open, such as `Copy` on `Ctrl Shift C`.
    pub fn shortcut(
        mut self,
        label: impl Into<SharedString>,
        keys: impl Into<SharedString>,
    ) -> Self {
        self.shortcut = Some((label.into(), keys.into()));
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let mark = self.tone.mark(colors);
        div()
            .flex()
            .items_center()
            .gap_2p5()
            .py_1()
            .pl_2()
            .pr_1p5()
            .rounded(density.radius_md)
            .border_1()
            .border_color(mark.map_or(colors.border, |(_, hue)| {
                colors.border.blend(hue.opacity(0.45))
            }))
            .bg(colors.surface_raised)
            .shadow(vec![
                BoxShadow::new(px(0.), px(6.), hsla(0., 0., 0., 0.5)).blur_radius(px(16.)),
            ])
            .typeset(density.small)
            .text_color(if mark.is_some() {
                colors.text_strong
            } else {
                colors.text
            })
            .whitespace_nowrap()
            .when(self.detail.is_some(), |tooltip| tooltip.py_1p5().px_2p5())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .children(
                                mark.map(|(name, hue)| Icon::new(name).size(px(14.)).color(hue)),
                            )
                            .child(self.content)
                            .children(self.tag),
                    )
                    .children(self.detail.map(|detail| {
                        div()
                            .max_w(px(280.))
                            .whitespace_normal()
                            .text_color(colors.text_muted)
                            .child(detail)
                    })),
            )
            .when_some(self.shortcut, |tooltip, (label, keys)| {
                tooltip
                    .child(div().w(px(1.)).self_stretch().bg(colors.border_subtle))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_color(colors.text_faint)
                            .child(label)
                            .child(Kbd::new(keys)),
                    )
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("tooltip", "gallery", || {
            crate::Gallery::new("Tooltip")
                .section(
                    "Kinds",
                    [
                        ("Text", Tooltip::new("Fetch from origin").into_any_element()),
                        (
                            "With a shortcut",
                            Tooltip::new("docs/design.md")
                                .shortcut("Copy", "Ctrl Shift C")
                                .into_any_element(),
                        ),
                    ],
                )
                .section(
                    "Tones",
                    [
                        (Tone::Success, "Copied path"),
                        (Tone::Warning, "Push isn't wired up yet"),
                        (Tone::Error, "Fetch failed"),
                    ]
                    .map(|(tone, text)| {
                        (
                            format!("{tone:?}"),
                            Tooltip::new(text).tone(tone).into_any_element(),
                        )
                    }),
                )
                .section(
                    "Notices",
                    [
                        (
                            "With a tag",
                            Tooltip::new("Push isn't wired up yet")
                                .tone(Tone::Warning)
                                .tag(Tag::new("UI"))
                                .into_any_element(),
                        ),
                        (
                            "With a detail",
                            Tooltip::new("Fetch failed")
                                .tone(Tone::Error)
                                .tag(Tag::new("Fixture").accent())
                                .detail("FakeGit has no remote for ~/Projects/pensive")
                                .into_any_element(),
                        ),
                    ],
                )
                .into_any_element()
        })
        .size(size(px(640.), px(420.))),
    ]
}

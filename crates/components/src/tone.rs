//! How a tooltip, toast or badge reports how something went: an icon, and an outline tinted with
//! the tone's hue.

use gpui::Hsla;
use theme::{ThemeColors, dp};

use crate::{Icon, IconName};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Neutral,
    Success,
    Warning,
    Error,
}

impl Tone {
    /// The icon it leads with, and the hue of it and the outline. Neutral has neither.
    pub(crate) fn mark(self, colors: &ThemeColors) -> Option<(IconName, Hsla)> {
        match self {
            Tone::Neutral => None,
            Tone::Success => Some((IconName::Check, colors.diff_added)),
            Tone::Warning => Some((IconName::TriangleAlert, colors.diff_modified)),
            Tone::Error => Some((IconName::CircleX, colors.diff_removed)),
        }
    }

    pub(crate) fn icon(self, size: f32, colors: &ThemeColors) -> Option<Icon> {
        self.mark(colors)
            .map(|(name, hue)| Icon::new(name).size(dp(size)).color(hue))
    }

    /// The border, 45% of the way to the tone's hue.
    pub(crate) fn outline(self, colors: &ThemeColors) -> Hsla {
        self.mark(colors).map_or(colors.border, |(_, hue)| {
            colors.border.blend(hue.opacity(0.45))
        })
    }
}

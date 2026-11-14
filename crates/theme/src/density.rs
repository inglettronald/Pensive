//! Sizes that change between the Calm and Dense presets. Spacing doesn't change, so it uses
//! GPUI's spacing methods (`gap_1`, `p_3`, ...) directly.

use gpui::{FontWeight, Global, Styled};
use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantArray};

use crate::{Dp, dp};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, VariantArray, IntoStaticStr, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum DensityPreset {
    /// Inter-sized text and 28 px rows. The default.
    Calm,
    /// Smaller body and code text and 24 px rows, for more on screen.
    Dense,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, VariantArray, IntoStaticStr)]
pub enum TextRole {
    Heading,
    Title,
    Body,
    Small,
    Label,
    Code,
}

#[derive(Clone, Copy, Debug)]
pub struct TextSize {
    pub size: Dp,
    pub line_height: Dp,
    pub weight: FontWeight,
}

#[derive(Clone, Debug)]
pub struct Density {
    pub heading: TextSize,
    pub title: TextSize,
    pub body: TextSize,
    pub small: TextSize,
    pub label: TextSize,
    /// Code text. Its line height is also the diff line height.
    pub code: TextSize,
    /// List rows such as files and commits.
    pub row_height: Dp,
    pub row_padding: Dp,
    pub row_gap: Dp,
    pub radius_sm: Dp,
    pub radius_md: Dp,
    pub radius_lg: Dp,
}

impl Global for Density {}

impl DensityPreset {
    pub fn density(self) -> Density {
        let text = |size: f32, line_height: f32, weight| TextSize {
            size: dp(size),
            line_height: dp(line_height),
            weight,
        };
        let heading = text(18., 24., FontWeight::SEMIBOLD);
        let title = text(15., 20., FontWeight::SEMIBOLD);
        let small = text(12., 16., FontWeight::NORMAL);
        let label = text(11., 16., FontWeight::MEDIUM);
        match self {
            Self::Calm => Density {
                heading,
                title,
                body: text(13., 20., FontWeight::NORMAL),
                small,
                label,
                code: text(13., 20., FontWeight::NORMAL),
                row_height: dp(28.),
                row_padding: dp(12.),
                row_gap: dp(10.),
                radius_sm: dp(4.),
                radius_md: dp(6.),
                radius_lg: dp(8.),
            },
            Self::Dense => Density {
                heading,
                title,
                body: text(12., 16., FontWeight::NORMAL),
                small,
                label,
                code: text(12., 18., FontWeight::NORMAL),
                row_height: dp(24.),
                row_padding: dp(8.),
                row_gap: dp(8.),
                radius_sm: dp(3.),
                radius_md: dp(4.),
                radius_lg: dp(6.),
            },
        }
    }
}

impl Density {
    pub fn text(&self, role: TextRole) -> TextSize {
        match role {
            TextRole::Heading => self.heading,
            TextRole::Title => self.title,
            TextRole::Body => self.body,
            TextRole::Small => self.small,
            TextRole::Label => self.label,
            TextRole::Code => self.code,
        }
    }
}

/// Sets font size, line height and weight from one `TextSize`, so text always matches a role.
pub trait Typeset: Styled + Sized {
    fn typeset(self, text: TextSize) -> Self {
        self.text_size(text.size)
            .line_height(text.line_height)
            .font_weight(text.weight)
    }
}

impl<E: Styled> Typeset for E {}

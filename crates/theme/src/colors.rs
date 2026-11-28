//! The color roles components read, each built-in theme's values for them, and the gray palette
//! Pensive Dark builds its neutrals from.

use gpui::{Hsla, rgb, rgba};
use refineable::Refineable;

const GRAYS: [u32; 12] = [
    0x0e0e0e, 0x141414, 0x1b1b1b, 0x242424, 0x2b2b2b, 0x353535, 0x464646, 0x5e5e5e, 0x858585,
    0xa3a3a3, 0xd6d6d6, 0xf3f3f3,
];

/// Step 1 (darkest) to 12 (lightest) of Pensive Dark's neutral palette. Components read roles, never steps.
pub fn gray(step: usize) -> Hsla {
    rgb(GRAYS[step - 1]).into()
}

/// Picks one role out of the theme, such as `|colors| colors.diff_added`, for a component that
/// reads it when it renders.
pub type ColorRole = fn(&ThemeColors) -> Hsla;

#[derive(Refineable, Clone, Debug)]
#[refineable(Debug, serde::Deserialize)]
pub struct ThemeColors {
    /// Window background and diff body.
    pub surface_app: Hsla,
    /// Sidebar, headers and status bar.
    pub surface_panel: Hsla,
    /// Menus, inputs and row hover.
    pub surface_raised: Hsla,
    /// The selected file or commit.
    pub surface_selected: Hsla,
    /// The drop shadow under floating surfaces. Each `Shadow` size scales its alpha.
    pub shadow: Hsla,

    /// Dividers inside a panel.
    pub border_subtle: Hsla,
    /// Panel edges and inputs.
    pub border: Hsla,
    /// The keyboard focus ring.
    pub border_focus: Hsla,

    /// Titles and the selected row's label.
    pub text_strong: Hsla,
    /// Body text and code.
    pub text: Hsla,
    /// Secondary labels.
    pub text_muted: Hsla,
    /// SHAs, hunk headers and hints.
    pub text_faint: Hsla,
    /// Disabled text only. Below the 4.5:1 contrast minimum, which disabled text is exempt from.
    pub text_disabled: Hsla,
    /// Line numbers of unchanged lines. Below 4.5:1 on purpose, so the colored numbers of changed
    /// lines stand out.
    pub diff_line_number: Hsla,

    /// The default action's fill. Its label uses `surface_app`.
    pub button_primary: Hsla,
    /// Every other button's fill. Its label uses `text`.
    pub button_secondary: Hsla,
    /// The outline a switch keeps on and off, between `border` and `text_disabled`.
    pub control_outline: Hsla,

    /// Added lines and words, file status A, success.
    pub diff_added: Hsla,
    /// Removed lines and words, file status D, conflicts and destructive actions.
    pub diff_removed: Hsla,
    /// File status M, moved lines and warnings.
    pub diff_modified: Hsla,

    /// Info callout fill.
    pub accent_subtle: Hsla,
    /// Info callout edge.
    pub accent_border: Hsla,
    /// The accent button's fill.
    pub accent_solid: Hsla,
    /// The accent button when hovered or pressed.
    pub accent_solid_hover: Hsla,
    /// The accent button's label.
    pub accent_label: Hsla,
    /// Links and the info icon.
    pub accent_fg: Hsla,
}

/// A role's name and color, for showing every role at once.
pub struct Role {
    pub group: &'static str,
    pub name: &'static str,
    pub color: Hsla,
}

impl ThemeColors {
    pub fn dark() -> Self {
        Self {
            surface_app: gray(1),
            surface_panel: gray(2),
            surface_raised: gray(3),
            surface_selected: gray(4),
            shadow: rgb(0x000000).into(),
            border_subtle: gray(5),
            border: gray(6),
            border_focus: gray(12),
            text_strong: gray(12),
            text: gray(11),
            text_muted: gray(10),
            text_faint: gray(9),
            text_disabled: gray(8),
            diff_line_number: gray(8),
            button_primary: gray(12),
            button_secondary: gray(3),
            control_outline: gray(7),
            diff_added: rgb(0x4cc38a).into(),
            diff_removed: rgb(0xef6461).into(),
            diff_modified: rgb(0xe5b45a).into(),
            accent_subtle: rgb(0x142131).into(),
            accent_border: rgb(0x2c5a8f).into(),
            accent_solid: rgb(0x2f6fe0).into(),
            accent_solid_hover: rgb(0x285fc8).into(),
            accent_label: gray(12),
            accent_fg: rgb(0x6cb0f5).into(),
        }
    }

    /// Pensive Light: an off-white window whose raised surfaces are white, so what's closer is
    /// lighter, as in Pensive Dark.
    pub fn light() -> Self {
        let hex = |value: u32| -> Hsla { rgb(value).into() };
        Self {
            surface_app: hex(0xfafafa),
            surface_panel: hex(0xf1f1f1),
            surface_raised: hex(0xffffff),
            surface_selected: hex(0xe3e3e3),
            shadow: rgba(0x00000038).into(),
            border_subtle: hex(0xe2e2e2),
            border: hex(0xd2d2d2),
            border_focus: hex(0x0e0e0e),
            text_strong: hex(0x0e0e0e),
            text: hex(0x242424),
            text_muted: hex(0x555555),
            text_faint: hex(0x696969),
            text_disabled: hex(0xa6a6a6),
            diff_line_number: hex(0xa9a9a9),
            button_primary: hex(0x141414),
            button_secondary: hex(0xffffff),
            control_outline: hex(0xbababa),
            diff_added: hex(0x197c4a),
            diff_removed: hex(0xcd2f2b),
            diff_modified: hex(0x926200),
            accent_subtle: hex(0xedf3fc),
            accent_border: hex(0xa9c5ef),
            accent_solid: hex(0x2f6fe0),
            accent_solid_hover: hex(0x285fc8),
            accent_label: hex(0xffffff),
            accent_fg: hex(0x1d5fc2),
        }
    }

    /// Pensive Paper: sepia surfaces and brown ink, with earthier diff colors and a muted blue,
    /// for less glare in a bright room.
    pub fn paper() -> Self {
        let hex = |value: u32| -> Hsla { rgb(value).into() };
        Self {
            surface_app: hex(0xf3e9d2),
            surface_panel: hex(0xeadcbd),
            surface_raised: hex(0xf9f2e2),
            surface_selected: hex(0xdfcea8),
            shadow: rgba(0x3b2a1448).into(),
            border_subtle: hex(0xddcba5),
            border: hex(0xcdb88e),
            border_focus: hex(0x3b2a14),
            text_strong: hex(0x2b1d0c),
            text: hex(0x3b2a16),
            text_muted: hex(0x5f4a2e),
            text_faint: hex(0x6e5838),
            text_disabled: hex(0xa8946e),
            diff_line_number: hex(0xab976f),
            button_primary: hex(0x3b2a16),
            button_secondary: hex(0xf9f2e2),
            control_outline: hex(0xbea77a),
            diff_added: hex(0x3f6a1c),
            diff_removed: hex(0xa3361f),
            diff_modified: hex(0x7f5200),
            accent_subtle: hex(0xdde3e6),
            accent_border: hex(0x9eb0c2),
            accent_solid: hex(0x3a5f8a),
            accent_solid_hover: hex(0x2f4f75),
            accent_label: hex(0xf9f2e2),
            accent_fg: hex(0x2f5580),
        }
    }

    /// Every role in field order, with the group the theme gallery lists it under.
    pub fn roles(&self) -> Vec<Role> {
        let role = |group, name, color| Role { group, name, color };
        vec![
            role("Surfaces", "surface_app", self.surface_app),
            role("Surfaces", "surface_panel", self.surface_panel),
            role("Surfaces", "surface_raised", self.surface_raised),
            role("Surfaces", "surface_selected", self.surface_selected),
            role("Surfaces", "shadow", self.shadow),
            role("Borders", "border_subtle", self.border_subtle),
            role("Borders", "border", self.border),
            role("Borders", "border_focus", self.border_focus),
            role("Text", "text_strong", self.text_strong),
            role("Text", "text", self.text),
            role("Text", "text_muted", self.text_muted),
            role("Text", "text_faint", self.text_faint),
            role("Text", "text_disabled", self.text_disabled),
            role("Text", "diff_line_number", self.diff_line_number),
            role("Controls", "button_primary", self.button_primary),
            role("Controls", "button_secondary", self.button_secondary),
            role("Controls", "control_outline", self.control_outline),
            role("Diff", "diff_added", self.diff_added),
            role("Diff", "diff_removed", self.diff_removed),
            role("Diff", "diff_modified", self.diff_modified),
            role("Accent", "accent_subtle", self.accent_subtle),
            role("Accent", "accent_border", self.accent_border),
            role("Accent", "accent_solid", self.accent_solid),
            role("Accent", "accent_solid_hover", self.accent_solid_hover),
            role("Accent", "accent_label", self.accent_label),
            role("Accent", "accent_fg", self.accent_fg),
        ]
    }
}

#[cfg(test)]
mod tests {
    use gpui::Rgba;

    use super::*;

    /// WCAG's contrast ratio, from 1 (none) to 21 (black on white).
    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let luminance = |color: Hsla| {
            let Rgba { r, g, b, .. } = color.into();
            let linear = |channel: f32| {
                if channel <= 0.03928 {
                    channel / 12.92
                } else {
                    ((channel + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn text_meets_the_contrast_minimum_on_every_surface_it_sits_on() {
        for built_in in &crate::BUILT_IN {
            let colors = built_in.theme().colors;
            let text = [
                ("text", colors.text),
                ("text_muted", colors.text_muted),
                ("text_faint", colors.text_faint),
                ("diff_added", colors.diff_added),
                ("diff_removed", colors.diff_removed),
                ("diff_modified", colors.diff_modified),
                ("accent_fg", colors.accent_fg),
            ];
            let surfaces = [
                ("surface_app", colors.surface_app),
                ("surface_panel", colors.surface_panel),
            ];
            for (text_name, text) in text {
                for (surface_name, surface) in surfaces {
                    let ratio = contrast(text, surface);
                    assert!(
                        ratio >= 4.5,
                        "{text_name} on {surface_name} is {ratio:.2}:1 in {}",
                        built_in.name
                    );
                }
            }
        }
    }
}

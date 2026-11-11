//! The color roles components read, and the gray palette the base theme builds them from.

use gpui::{Hsla, rgb};
use refineable::Refineable;

const GRAYS: [u32; 12] = [
    0x0e0e0e, 0x141414, 0x1b1b1b, 0x242424, 0x2b2b2b, 0x353535, 0x464646, 0x5e5e5e, 0x858585,
    0xa3a3a3, 0xd6d6d6, 0xf3f3f3,
];

/// Step 1 (darkest) to 12 (lightest) of the neutral palette. Components read roles, never steps.
pub fn gray(step: usize) -> Hsla {
    rgb(GRAYS[step - 1]).into()
}

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
            diff_added: rgb(0x4cc38a).into(),
            diff_removed: rgb(0xef6461).into(),
            diff_modified: rgb(0xe5b45a).into(),
            accent_subtle: rgb(0x142131).into(),
            accent_border: rgb(0x2c5a8f).into(),
            accent_solid: rgb(0x2f6fe0).into(),
            accent_solid_hover: rgb(0x285fc8).into(),
            accent_fg: rgb(0x6cb0f5).into(),
        }
    }

    /// Every role in field order, grouped as in `docs/design.md`.
    pub fn roles(&self) -> Vec<Role> {
        let role = |group, name, color| Role { group, name, color };
        vec![
            role("Surfaces", "surface_app", self.surface_app),
            role("Surfaces", "surface_panel", self.surface_panel),
            role("Surfaces", "surface_raised", self.surface_raised),
            role("Surfaces", "surface_selected", self.surface_selected),
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
            role("Diff", "diff_added", self.diff_added),
            role("Diff", "diff_removed", self.diff_removed),
            role("Diff", "diff_modified", self.diff_modified),
            role("Accent", "accent_subtle", self.accent_subtle),
            role("Accent", "accent_border", self.accent_border),
            role("Accent", "accent_solid", self.accent_solid),
            role("Accent", "accent_solid_hover", self.accent_solid_hover),
            role("Accent", "accent_fg", self.accent_fg),
        ]
    }
}

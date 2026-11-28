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

    // Code foregrounds, tuned to remain legible over both diff fills.
    pub syntax_keyword: Hsla,
    pub syntax_string: Hsla,
    pub syntax_number: Hsla,
    pub syntax_comment: Hsla,
    pub syntax_type: Hsla,
    pub syntax_function: Hsla,
    pub syntax_call: Hsla,
    pub syntax_parameter: Hsla,
    pub syntax_property: Hsla,
    pub syntax_constant: Hsla,
    pub syntax_annotation: Hsla,
    pub syntax_operator: Hsla,
    pub syntax_tag: Hsla,
    pub syntax_tag_attribute: Hsla,

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
            control_outline: gray(7),
            diff_added: rgb(0x4cc38a).into(),
            diff_removed: rgb(0xef6461).into(),
            diff_modified: rgb(0xe5b45a).into(),
            syntax_keyword: rgb(0xd9b3f2).into(),
            syntax_string: rgb(0xc2dca9).into(),
            syntax_number: rgb(0xf3c38d).into(),
            syntax_comment: rgb(0xb0b8bd).into(),
            syntax_type: rgb(0xa4d9dc).into(),
            syntax_function: rgb(0xa9c9ff).into(),
            syntax_call: rgb(0xbed5f8).into(),
            syntax_parameter: rgb(0xd6c4b3).into(),
            syntax_property: rgb(0xb9d0d9).into(),
            syntax_constant: rgb(0xf5b9af).into(),
            syntax_annotation: rgb(0xe2bded).into(),
            syntax_operator: rgb(0xc6c0df).into(),
            syntax_tag: rgb(0xf3b5c6).into(),
            syntax_tag_attribute: rgb(0xb5cbf2).into(),
            accent_subtle: rgb(0x142131).into(),
            accent_border: rgb(0x2c5a8f).into(),
            accent_solid: rgb(0x2f6fe0).into(),
            accent_solid_hover: rgb(0x285fc8).into(),
            accent_fg: rgb(0x6cb0f5).into(),
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
            role("Syntax", "syntax_keyword", self.syntax_keyword),
            role("Syntax", "syntax_string", self.syntax_string),
            role("Syntax", "syntax_number", self.syntax_number),
            role("Syntax", "syntax_comment", self.syntax_comment),
            role("Syntax", "syntax_type", self.syntax_type),
            role("Syntax", "syntax_function", self.syntax_function),
            role("Syntax", "syntax_call", self.syntax_call),
            role("Syntax", "syntax_parameter", self.syntax_parameter),
            role("Syntax", "syntax_property", self.syntax_property),
            role("Syntax", "syntax_constant", self.syntax_constant),
            role("Syntax", "syntax_annotation", self.syntax_annotation),
            role("Syntax", "syntax_operator", self.syntax_operator),
            role("Syntax", "syntax_tag", self.syntax_tag),
            role("Syntax", "syntax_tag_attribute", self.syntax_tag_attribute),
            role("Accent", "accent_subtle", self.accent_subtle),
            role("Accent", "accent_border", self.accent_border),
            role("Accent", "accent_solid", self.accent_solid),
            role("Accent", "accent_solid_hover", self.accent_solid_hover),
            role("Accent", "accent_fg", self.accent_fg),
        ]
    }
}

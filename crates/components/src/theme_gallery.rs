//! The theme's gallery: every color role, both fonts, and the type scale in each density.

use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, Rgba, Styled, Window, div, px,
    size,
};
use mock::Mock;
use theme::{ActiveTheme, DensityPreset, TextRole, ThemeOverride, Typeset, VariantArray, dp, gray};

use crate::Gallery;

/// The override example from the design canvas, using the other bundled font pair.
const SAMPLE_OVERRIDE: &str = r##"
[colors]
text_muted = "#9a9a9a"
diff_removed = "#f0954a"
diff_added = "#3fb8c4"

[fonts]
ui = "Geist"
code = "Geist Mono"
"##;

pub(crate) fn mocks() -> Vec<Mock> {
    let size = size(px(1280.), px(1240.));
    vec![
        Mock::component("theme", "gallery", || ThemeGallery.into_any_element()).size(size),
        Mock::new("theme", "override", |_, cx| {
            let (theme_override, _) =
                ThemeOverride::from_toml(SAMPLE_OVERRIDE).expect("the sample override is valid");
            let theme = cx.theme().with_override(&theme_override);
            cx.set_global(theme);
            mock::element_view(cx, || ThemeGallery.into_any_element())
        })
        .size(size),
    ]
}

#[derive(IntoElement)]
struct ThemeGallery;

impl RenderOnce for ThemeGallery {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let fonts = theme.fonts.clone();
        let border = theme.colors.border_subtle;
        let text = theme.colors.text;
        let roles = theme.colors.roles();

        let mut gallery = Gallery::new(format!("{} · {} + {}", theme.name, fonts.ui, fonts.code))
            .section(
                "Palette",
                (1..=12).map(|step| (format!("gray {step}"), swatch(gray(step), border, 64.))),
            );
        for group in roles.chunk_by(|a, b| a.group == b.group) {
            gallery = gallery.section(
                group[0].group,
                group.iter().map(|role| {
                    (
                        format!("{} {}", role.name, hex(role.color)),
                        swatch(role.color, border, 160.),
                    )
                }),
            );
        }
        gallery.matrix(
            "Type",
            TextRole::VARIANTS,
            DensityPreset::VARIANTS,
            |role, preset| {
                let size = preset.density().text(role);
                let font = match role {
                    TextRole::Code => fonts.code.clone(),
                    _ => fonts.ui.clone(),
                };
                div()
                    .font_family(font)
                    .typeset(size)
                    .text_color(text)
                    .child(format!(
                        "{} · {}/{}",
                        sample(role),
                        size.size,
                        size.line_height
                    ))
                    .into_any_element()
            },
        )
    }
}

fn swatch(color: Hsla, border: Hsla, width: f32) -> AnyElement {
    div()
        .w(dp(width))
        .h(dp(40.))
        .rounded(dp(6.))
        .border_1()
        .border_color(border)
        .bg(color)
        .into_any_element()
}

fn sample(role: TextRole) -> &'static str {
    match role {
        TextRole::Heading => "theme-roles",
        TextRole::Title => "crates/theme/src/theme.rs",
        TextRole::Body => "Split Colors into semantic roles",
        TextRole::Small => "3 files changed · 2 hours ago",
        TextRole::Label => "CHANGES",
        TextRole::Code => "surface_raised: gray(3),",
    }
}

fn hex(color: Hsla) -> String {
    let Rgba { r, g, b, .. } = color.into();
    let channel = |value: f32| (value * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", channel(r), channel(g), channel(b))
}

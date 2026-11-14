//! A change's added and removed line counts, each left out when it's zero.

use gpui::{App, Window, div, prelude::*};
use theme::{ActiveTheme, Typeset};

#[derive(IntoElement)]
pub struct DiffStat {
    added: usize,
    removed: usize,
}

impl DiffStat {
    pub fn new(added: usize, removed: usize) -> Self {
        Self { added, removed }
    }
}

impl RenderOnce for DiffStat {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let count = |text: String, color| div().text_color(color).child(text);
        div()
            .flex()
            .flex_none()
            .gap_1p5()
            .font_family(theme.fonts.code.clone())
            .typeset(cx.density().small)
            .whitespace_nowrap()
            .when(self.added > 0, |stat| {
                stat.child(count(format!("+{}", self.added), colors.diff_added))
            })
            .when(self.removed > 0, |stat| {
                stat.child(count(format!("−{}", self.removed), colors.diff_removed))
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("diff_stat", "gallery", || {
            crate::Gallery::new("DiffStat")
                .section(
                    "Counts",
                    [
                        ("Both", (12, 3)),
                        ("Added only", (52, 0)),
                        ("Removed only", (0, 64)),
                        ("Nothing", (0, 0)),
                    ]
                    .map(|(label, (added, removed))| {
                        (label, DiffStat::new(added, removed).into_any_element())
                    }),
                )
                .into_any_element()
        })
        .size(640., 200.),
    ]
}

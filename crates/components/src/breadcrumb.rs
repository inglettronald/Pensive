//! Levels of a location separated by chevrons, such as project › branch or a file's path.

use gpui::{AnyElement, App, Pixels, Window, div, prelude::*, px};
use theme::ActiveTheme;

use crate::{Icon, IconName};

#[derive(IntoElement)]
pub struct Breadcrumb {
    crumbs: Vec<AnyElement>,
    chevron: Pixels,
}

impl Breadcrumb {
    /// Each crumb draws itself, so a crumb can be plain text or a button.
    pub fn new(crumbs: impl IntoIterator<Item = impl IntoElement>) -> Self {
        Self {
            crumbs: crumbs
                .into_iter()
                .map(IntoElement::into_any_element)
                .collect(),
            chevron: px(12.),
        }
    }

    /// The chevron's size, 12 px unless set.
    pub fn chevron_size(mut self, size: Pixels) -> Self {
        self.chevron = size;
        self
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let faint = cx.theme().colors.text_faint;
        let mut crumbs = Vec::with_capacity(self.crumbs.len() * 2);
        for (index, crumb) in self.crumbs.into_iter().enumerate() {
            if index > 0 {
                crumbs.push(
                    Icon::new(IconName::ChevronRight)
                        .size(self.chevron)
                        .color(faint)
                        .into_any_element(),
                );
            }
            crumbs.push(crumb);
        }
        div()
            .flex()
            .items_center()
            .gap_1()
            .min_w_0()
            .whitespace_nowrap()
            .children(crumbs)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("breadcrumb", "gallery", || {
            crate::Gallery::new("Breadcrumb")
                .section(
                    "Crumbs",
                    [
                        (
                            "Project and branch",
                            Breadcrumb::new(["pensive", "feat/command-palette"]),
                        ),
                        (
                            "A file's path",
                            Breadcrumb::new(["crates", "components", "src", "palette.rs"]),
                        ),
                        ("One level", Breadcrumb::new(["docs"])),
                    ]
                    .map(|(label, crumb)| (label, crumb.into_any_element())),
                )
                .into_any_element()
        })
        .size(size(px(640.), px(200.))),
    ]
}

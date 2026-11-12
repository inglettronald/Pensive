//! A row of mutually exclusive options in a track, labeled with text or icons, with at most one
//! selected.

use std::rc::Rc;

use gpui::{App, ElementId, SharedString, Window, div, prelude::*, px};
use theme::{ActiveTheme, Typeset};

use crate::{Callback, Icon, IconName};

enum Segment {
    Text(SharedString),
    Icon(IconName),
}

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    options: Vec<Segment>,
    selected: Option<usize>,
    on_select: Option<Callback<usize>>,
}

impl SegmentedControl {
    pub fn new(
        id: impl Into<ElementId>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: impl Into<Option<usize>>,
    ) -> Self {
        Self {
            id: id.into(),
            options: options
                .into_iter()
                .map(|option| Segment::Text(option.into()))
                .collect(),
            selected: selected.into(),
            on_select: None,
        }
    }

    /// Options drawn as icons only, such as views of the same list.
    pub fn icons(
        id: impl Into<ElementId>,
        icons: impl IntoIterator<Item = IconName>,
        selected: impl Into<Option<usize>>,
    ) -> Self {
        Self {
            id: id.into(),
            options: icons.into_iter().map(Segment::Icon).collect(),
            selected: selected.into(),
            on_select: None,
        }
    }

    /// Called with the index of the option that was clicked.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let on_select = self.on_select;
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .gap_0p5()
            .p_0p5()
            .rounded(density.radius_md)
            .border_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_app)
            .typeset(density.small)
            .children(self.options.into_iter().enumerate().map(|(index, option)| {
                let selected = self.selected == Some(index);
                let segment = div()
                    .id(index)
                    .rounded(density.radius_sm)
                    .when(selected, |segment| segment.bg(colors.border));
                match option {
                    Segment::Text(text) => segment
                        .px_2()
                        .line_height(px(20.))
                        .whitespace_nowrap()
                        .text_color(if selected {
                            colors.text_strong
                        } else {
                            colors.text_muted
                        })
                        .child(text),
                    Segment::Icon(icon) => segment
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(24.))
                        .h(px(20.))
                        .child(Icon::new(icon).size(px(14.)).color(if selected {
                            colors.text_strong
                        } else {
                            colors.text_faint
                        })),
                }
                .when_some(on_select.clone(), |option, handler| {
                    option.on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        handler(index, window, cx)
                    })
                })
            }))
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("segmented_control", "gallery", || {
            const OPTIONS: [&str; 3] = ["Off", "New file", "Both"];
            crate::Gallery::new("SegmentedControl")
                .section(
                    "Selected",
                    OPTIONS.iter().enumerate().map(|(index, &label)| {
                        (
                            label,
                            SegmentedControl::new(label, OPTIONS, index).into_any_element(),
                        )
                    }),
                )
                .section(
                    "Icons",
                    [("List", 0), ("Tree", 1), ("Symbols", 2)].map(|(label, index)| {
                        let icons = [IconName::List, IconName::ListTree, IconName::Braces];
                        (
                            label,
                            SegmentedControl::icons(label, icons, index).into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .size(size(px(640.), px(280.))),
    ]
}

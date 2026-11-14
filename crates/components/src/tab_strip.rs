//! The tabs that Tab cycles through: a segmented control labeled with the Tab key.

use std::rc::Rc;

use gpui::{App, ElementId, SharedString, Window, div, prelude::*};

use crate::{Callback, Kbd, SegmentedControl};

#[derive(IntoElement)]
pub struct TabStrip {
    id: ElementId,
    tabs: Vec<SharedString>,
    selected: Option<usize>,
    on_select: Option<Callback<usize>>,
}

impl TabStrip {
    /// With no tab selected, the strip dims to show that something else is narrowing the results.
    pub fn new(
        id: impl Into<ElementId>,
        tabs: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: Option<usize>,
    ) -> Self {
        Self {
            id: id.into(),
            tabs: tabs.into_iter().map(Into::into).collect(),
            selected,
            on_select: None,
        }
    }

    /// Called with the index of the tab that was clicked.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TabStrip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let tabs = SegmentedControl::new(self.id, self.tabs, self.selected).compact();
        let tabs = match self.on_select {
            Some(handler) => tabs.on_select(move |index, window, cx| handler(index, window, cx)),
            None => tabs,
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .when(self.selected.is_none(), |strip| strip.opacity(0.7))
            .child(Kbd::new("Tab"))
            .child(tabs)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("tab_strip", "gallery", || {
            const TABS: [&str; 3] = ["All", "Active", "Commands"];
            let selections = [
                ("All", Some(0)),
                ("Active", Some(1)),
                ("Commands", Some(2)),
                ("None, a keyword is narrowing", None),
            ];
            crate::Gallery::new("TabStrip")
                .section(
                    "Selected",
                    selections.map(|(label, selected)| {
                        (
                            label,
                            TabStrip::new(label, TABS, selected).into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .size(960., 200.),
    ]
}

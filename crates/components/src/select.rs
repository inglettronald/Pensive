//! A dropdown: the current value and a chevron, which opens a menu of the options with a check on
//! the current one.

use std::rc::Rc;

use gpui::{App, ElementId, SharedString, Window, div, prelude::*};
use theme::{ActiveTheme, Typeset, dp};

use crate::{
    Callback, Icon, IconName, child_id,
    menu::{Leading, MIN_WIDTH, menu, menu_row},
};

#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    value: SharedString,
    options: Vec<SharedString>,
    selected: Option<usize>,
    open: Option<bool>,
    hovered: Option<usize>,
    on_choose: Option<Callback<usize>>,
}

impl Select {
    pub fn new(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            options: Vec::new(),
            selected: None,
            open: None,
            hovered: None,
            on_choose: None,
        }
    }

    /// What the menu lists, and which of them is the current value.
    pub fn options(
        mut self,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: Option<usize>,
    ) -> Self {
        self.options = options.into_iter().map(Into::into).collect();
        self.selected = selected;
        self
    }

    /// Shows the menu open with `hovered` filled, for a mock. Clicking opens it otherwise.
    pub fn open(mut self, hovered: Option<usize>) -> Self {
        self.open = Some(true);
        self.hovered = hovered;
        self
    }

    /// Called with the index of the option picked from the menu.
    pub fn on_choose(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_choose = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let opened = window.use_keyed_state(child_id(&self.id, "open"), cx, |_, _| false);
        let open = self.open.unwrap_or(*opened.read(cx)) && !self.options.is_empty();
        let menu = open.then(|| {
            let rows = self.options.into_iter().enumerate().map(|(index, option)| {
                let opened = opened.clone();
                let on_choose = self.on_choose.clone();
                let leading = if self.selected == Some(index) {
                    Leading::Check
                } else {
                    Leading::Blank
                };
                menu_row(index, leading, self.hovered == Some(index), cx)
                    .child(option)
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        opened.write(cx, false);
                        if let Some(handler) = &on_choose {
                            handler(index, window, cx)
                        }
                    })
                    .into_any_element()
            });
            let close = opened.clone();
            menu(rows, move |cx| close.write(cx, false), cx)
        });
        let colors = &cx.theme().colors;
        let density = cx.density();
        div()
            .id(self.id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap_2()
            .min_w(MIN_WIDTH)
            .h(density.row_height)
            .pl_2p5()
            .pr_2()
            .rounded(density.radius_md)
            .border_1()
            .border_color(if open {
                colors.text_faint
            } else {
                colors.border
            })
            .bg(colors.surface_raised)
            .hover(|select| select.bg(colors.surface_selected))
            .text_color(colors.text)
            .typeset(density.body)
            .whitespace_nowrap()
            .child(self.value)
            .child(
                Icon::new(if open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .size(dp(14.))
                .color(if open {
                    colors.text_muted
                } else {
                    colors.text_faint
                }),
            )
            .children(menu)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                opened.update(cx, |open, cx| {
                    *open = !*open;
                    cx.notify();
                });
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    const FONTS: [&str; 4] = ["From the theme", "Inter", "Geist", "System UI"];
    vec![
        mock::Mock::component("select", "gallery", || {
            crate::Gallery::new("Select")
                .section(
                    "Values",
                    ["Edge bar", "Calm", "Pensive Dark"]
                        .map(|value| (value, Select::new(value, value).into_any_element())),
                )
                .into_any_element()
        })
        .size(640., 200.),
        mock::Mock::component("select", "open", || {
            crate::Gallery::new("Select")
                .section(
                    "Open, with Inter hovered",
                    [(
                        "",
                        Select::new("font", FONTS[0])
                            .options(FONTS, Some(0))
                            .open(Some(1))
                            .into_any_element(),
                    )],
                )
                .into_any_element()
        })
        .size(480., 360.),
    ]
}

#[cfg(test)]
#[expect(clippy::disallowed_methods, reason = "tests click at window pixels")]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{Context, Modifiers, TestAppContext, point, px, size};

    use super::*;

    struct Field {
        chosen: Rc<Cell<Option<usize>>>,
    }

    impl Render for Field {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let chosen = self.chosen.clone();
            div().size_full().child(
                Select::new("days", "1 day")
                    .options(["1 day", "7 days", "30 days"], Some(0))
                    .on_choose(move |index, _, _| chosen.set(Some(index))),
            )
        }
    }

    #[gpui::test]
    fn clicking_opens_the_menu_and_an_option_picks_it(cx: &mut TestAppContext) {
        cx.update(theme::init);
        let chosen = Rc::new(Cell::new(None));
        let (_, cx) = cx.add_window_view(|_, _| Field {
            chosen: chosen.clone(),
        });
        cx.simulate_resize(size(px(400.), px(300.)));
        // The menu starts 4 px under the 28 px field, and each option is a 28 px row inside its
        // 1 px border and 4 px of padding, so the second option's middle is at 79 px.
        cx.simulate_click(point(px(20.), px(79.)), Modifiers::none());
        assert_eq!(
            chosen.get(),
            None,
            "the menu is closed until the field is clicked"
        );
        cx.simulate_click(point(px(20.), px(14.)), Modifiers::none());
        cx.simulate_click(point(px(20.), px(79.)), Modifiers::none());
        assert_eq!(chosen.get(), Some(1));
    }
}

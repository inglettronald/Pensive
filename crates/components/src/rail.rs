//! The settings rail: a button per category down the left edge, with the current one filled and
//! marked by a bar that slides when it changes, and tools at its foot.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, SharedString, Window, deferred, div,
    ease_out_quint, prelude::*,
};
use theme::{ActiveTheme, Dp, dp};

use crate::{Callback, Icon, IconName, OnClick as _, Tooltip, callback};

/// How long the fill and the bar take to reach a new category.
const SLIDE: Duration = Duration::from_millis(150);
const ICON: Dp = dp(18.);
/// Between buttons, and between the bar and the button it marks.
const GAP: Dp = dp(4.);

/// A button is a row and 8 px tall, so it grows and shrinks with the density.
fn button_size(cx: &App) -> Dp {
    cx.density().row_height + dp(8.)
}

/// One button on the rail, with its name in a tooltip while hovered.
#[derive(IntoElement)]
pub struct RailButton {
    id: ElementId,
    icon: IconName,
    name: SharedString,
    selected: bool,
    filled: bool,
    dimmed: bool,
    hovered: bool,
    notice: Option<AnyElement>,
    on_click: Option<Callback<()>>,
    on_hover: Option<Callback<bool>>,
}

impl RailButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon,
            name: name.into(),
            selected: false,
            filled: false,
            dimmed: false,
            hovered: false,
            notice: None,
            on_click: None,
            on_hover: None,
        }
    }

    /// The icon in `text_strong`. The rail draws the fill, so it can slide.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The icon in `text_strong` on its own fill, for a tool that marks where you are, such as
    /// the gear in Settings. The rail draws its items' fill instead, so it can slide.
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Shown beside the button in place of its name, such as a notice about clicking it.
    pub fn notice(mut self, notice: Option<AnyElement>) -> Self {
        self.notice = notice;
        self
    }

    /// The icon in `text_disabled`, such as for a category with no search results.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// Fills the button and shows its tooltip. The view sets it from `on_hover`, since a
    /// component keeps no state.
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(callback(handler));
        self
    }

    pub fn on_hover(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(std::rc::Rc::new(handler));
        self
    }
}

impl RenderOnce for RailButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let color = if self.selected || self.filled {
            colors.text_strong
        } else if self.dimmed {
            colors.text_disabled
        } else if self.hovered {
            colors.text
        } else {
            colors.text_faint
        };
        div()
            .id(self.id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(button_size(cx))
            .rounded(cx.density().radius_md)
            .when(self.filled, |button| button.bg(colors.surface_selected))
            .when(self.hovered && !self.selected && !self.filled, |button| {
                button.bg(colors.surface_raised)
            })
            .child(Icon::new(self.icon).size(ICON).color(color))
            .children(
                self.notice
                    .or_else(|| {
                        self.hovered
                            .then(|| Tooltip::new(self.name).into_any_element())
                    })
                    .map(|tooltip| {
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left_full()
                            .ml_1p5()
                            .flex()
                            .items_center()
                            .child(deferred(tooltip).priority(1))
                    }),
            )
            .when_some(self.on_hover, |button, handler| {
                button.on_hover(move |hovered, window, cx| handler(*hovered, window, cx))
            })
            .on_click_callback(self.on_click)
    }
}

/// A category's button on the rail.
pub struct RailItem {
    pub icon: IconName,
    pub name: SharedString,
    /// Drawn in `text_disabled`, such as when a search finds nothing in it.
    pub dimmed: bool,
}

#[derive(IntoElement)]
pub struct Rail {
    items: Vec<RailItem>,
    selected: usize,
    /// The category the fill slides from, when it just changed.
    previous: Option<usize>,
    hovered: Option<usize>,
    notice: Option<(usize, AnyElement)>,
    tools: Vec<AnyElement>,
    on_select: Option<Callback<usize>>,
    on_hover: Option<Callback<Option<usize>>>,
}

impl Rail {
    pub fn new(items: Vec<RailItem>, selected: usize) -> Self {
        Self {
            items,
            selected,
            previous: None,
            hovered: None,
            notice: None,
            tools: Vec::new(),
            on_select: None,
            on_hover: None,
        }
    }

    /// Slides the fill and the bar from `previous` to the selected category.
    pub fn previous(mut self, previous: Option<usize>) -> Self {
        self.previous = previous;
        self
    }

    pub fn hovered(mut self, hovered: Option<usize>) -> Self {
        self.hovered = hovered;
        self
    }

    /// Shown beside the item at its index in place of the item's name, such as a notice about
    /// clicking it.
    pub fn notice(mut self, notice: Option<(usize, AnyElement)>) -> Self {
        self.notice = notice;
        self
    }

    /// Buttons at the rail's foot, usually [`RailButton`]s.
    pub fn tools(mut self, tools: impl IntoIterator<Item = AnyElement>) -> Self {
        self.tools = tools.into_iter().collect();
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(handler));
        self
    }

    /// Called with the hovered category, or `None` when the pointer leaves it.
    pub fn on_hover(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hover = Some(std::rc::Rc::new(handler));
        self
    }
}

impl RenderOnce for Rail {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let size = button_size(cx);
        let inset = dp(8.);
        let top = move |index: usize| inset + (size + GAP) * index as f32;
        let fill = div()
            .absolute()
            .left(inset)
            .size(size)
            .rounded(density.radius_md)
            .bg(colors.surface_selected);
        let bar = div()
            .absolute()
            .left_0()
            .w(dp(2.))
            .h(size - dp(16.))
            .rounded_r(dp(2.))
            .bg(colors.text_strong);
        let to = self.selected;
        let marks = match self.previous.filter(|from| *from != to) {
            Some(from) => {
                let slide = Animation::new(SLIDE).with_easing(ease_out_quint());
                let between = move |delta: f32| top(from) + (top(to) - top(from)) * delta;
                vec![
                    fill.with_animation(("rail-fill", from * 1000 + to), slide.clone(), {
                        move |fill, delta| fill.top(between(delta))
                    })
                    .into_any_element(),
                    bar.with_animation(("rail-bar", from * 1000 + to), slide, {
                        move |bar, delta| bar.top(between(delta) + dp(8.))
                    })
                    .into_any_element(),
                ]
            }
            None => vec![
                fill.top(top(to)).into_any_element(),
                bar.top(top(to) + dp(8.)).into_any_element(),
            ],
        };
        let mut notice = self.notice;
        let buttons = self.items.into_iter().enumerate().map(|(index, item)| {
            let notice = notice
                .take_if(|(at, _)| *at == index)
                .map(|(_, notice)| notice);
            let on_select = self.on_select.clone();
            let on_hover = self.on_hover.clone();
            RailButton::new(("rail", index), item.icon, item.name)
                .selected(index == self.selected)
                .dimmed(item.dimmed)
                .hovered(self.hovered == Some(index) && notice.is_none())
                .notice(notice)
                .on_click(move |window, cx| {
                    if let Some(handler) = &on_select {
                        handler(index, window, cx)
                    }
                })
                .on_hover(move |hovered, window, cx| {
                    if let Some(handler) = &on_hover {
                        handler(hovered.then_some(index), window, cx)
                    }
                })
        });
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            // From the left, like the fill, so the border on the right doesn't shift the buttons.
            .items_start()
            .gap(GAP)
            .w(size + inset * 2.)
            .h_full()
            .p(inset)
            .bg(colors.surface_app)
            .border_r_1()
            .border_color(colors.border_subtle)
            .children(marks)
            .children(buttons)
            .child(div().flex_1())
            .children(self.tools)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    fn items(dimmed: &[usize]) -> Vec<RailItem> {
        [
            (IconName::Palette, "Appearance"),
            (IconName::Command, "Command palette"),
            (IconName::Bell, "Notices"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (icon, name))| RailItem {
            icon,
            name: name.into(),
            dimmed: dimmed.contains(&index),
        })
        .collect()
    }
    fn rail(selected: usize, hovered: Option<usize>, dimmed: &[usize]) -> AnyElement {
        div()
            .h(dp(240.))
            .child(Rail::new(items(dimmed), selected).hovered(hovered).tools([
                RailButton::new("file", IconName::FileCog, "Open settings.toml").into_any_element(),
            ]))
            .into_any_element()
    }

    vec![
        mock::Mock::component("rail", "gallery", || {
            crate::Gallery::new("Rail")
                .section(
                    "States",
                    [
                        ("First selected", rail(0, None, &[])),
                        ("Hovering Notices", rail(1, Some(2), &[])),
                        ("Searching", rail(1, None, &[0, 2])),
                    ],
                )
                .into_any_element()
        })
        .size(520., 360.),
    ]
}

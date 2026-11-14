//! The settings sidebar: the category rail, the search field, and the current category's pages.

use components::{Icon, IconName, Kbd, NavItem, NavList, Rail, RailButton, RailItem};
use gpui::{
    Animation, AnimationExt, AnyElement, Context, Focusable, Window, deferred, div, ease_out_quint,
    prelude::*,
};
use theme::{ActiveTheme, Typeset, dp};

use super::{CATEGORY_IN, CATEGORY_OUT, SettingsView, Spot, Tool, outline::Category, search::Hits};
use crate::{notices, view_callback, view_callback_with};

impl SettingsView {
    pub(super) fn rail(
        &self,
        outline: &[Category],
        hits: Option<&Hits>,
        cx: &mut Context<Self>,
    ) -> Rail {
        let items = outline
            .iter()
            .enumerate()
            .map(|(index, category)| RailItem {
                icon: category.icon,
                name: category.name.into(),
                dimmed: hits.is_some_and(|hits| !hits.in_category(index)),
            })
            .collect();
        let selected = if hits.is_some() {
            self.home()
        } else {
            self.current.category
        };
        let tool = |tool: Tool, icon: IconName, name: &'static str| {
            let notice = self
                .notice
                .as_ref()
                .filter(|(shown, ..)| *shown == tool)
                .map(|(_, notice, _)| notices::tooltip(notice));
            div()
                .relative()
                .child(
                    RailButton::new(name, icon, name)
                        .hovered(self.hovered_tool == Some(tool) && notice.is_none())
                        .on_hover(view_callback_with(
                            cx,
                            move |view: &mut Self, on: bool, _, cx| {
                                view.hovered_tool = on.then_some(tool);
                                cx.notify();
                            },
                        ))
                        .on_click(view_callback(cx, move |view: &mut Self, _, cx| {
                            view.use_tool(tool, cx)
                        })),
                )
                .children(notice.map(|tooltip| {
                    div()
                        .absolute()
                        .bottom_0()
                        .left_full()
                        .ml_1p5()
                        .child(deferred(tooltip).priority(1))
                }))
                .into_any_element()
        };
        Rail::new(items, selected)
            .previous(
                self.previous
                    .as_ref()
                    .filter(|_| hits.is_none())
                    .map(|(spot, _)| spot.category),
            )
            .hovered(self.hovered)
            .tools([
                tool(Tool::OpenFile, IconName::FileCog, "Open settings.toml"),
                tool(Tool::ShowNames, IconName::PanelLeft, "Show names"),
            ])
            .on_select(view_callback_with(cx, |view: &mut Self, index, _, cx| {
                view.show_category(index, cx)
            }))
            .on_hover(view_callback_with(cx, |view: &mut Self, index, _, cx| {
                view.hovered = index;
                cx.notify();
            }))
    }

    pub(super) fn search_field(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let focused = self.search.focus_handle(cx).is_focused(window);
        let hint = if self.query(cx).is_empty() {
            "/"
        } else {
            "Esc"
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(density.row_height)
            .mx_2p5()
            .mb_2p5()
            .px_2()
            .rounded(density.radius_md)
            .border_1()
            .border_color(if focused {
                colors.border_focus
            } else {
                colors.border
            })
            .bg(colors.surface_app)
            .typeset(density.body)
            .child(
                Icon::new(IconName::Search)
                    .size(dp(14.))
                    .color(colors.text_faint),
            )
            .child(div().flex_1().min_w_0().child(self.search.clone()))
            .child(Kbd::new(hint))
            .into_any_element()
    }

    /// The current category's title and pages, crossfading from the last category's.
    pub(super) fn pages_list(&self, outline: &[Category], cx: &mut Context<Self>) -> AnyElement {
        let list = |category: usize, cx: &mut Context<Self>| {
            let colors = &cx.theme().colors;
            let density = cx.density();
            let Some(entry) = outline.get(category) else {
                return div().into_any_element();
            };
            let current = self.current;
            let selected = (current.category == category).then_some(current.page);
            let previous = self
                .previous
                .as_ref()
                .filter(|(spot, _)| spot.category == category)
                .map(|(spot, _)| spot.page);
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .pt_1()
                        .pb_3()
                        .px_4()
                        .child(
                            div()
                                .typeset(density.title)
                                .text_color(colors.text_strong)
                                .child(entry.name),
                        )
                        .child(
                            div()
                                .typeset(density.small)
                                .text_color(colors.text_faint)
                                .child(entry.description),
                        ),
                )
                .child(
                    NavList::new(
                        ("pages", category),
                        entry
                            .pages
                            .iter()
                            .map(|page| NavItem::new(page.name))
                            .collect(),
                    )
                    .selected(selected)
                    .previous(previous)
                    .on_select(view_callback_with(
                        cx,
                        move |view: &mut Self, page, _, cx| {
                            view.show_page(Spot { category, page }, cx)
                        },
                    )),
                )
                .into_any_element()
        };
        let to = self.current.category;
        let Some(from) = self.fading.as_ref().map(|(from, ..)| *from) else {
            return list(to, cx);
        };
        let total = (CATEGORY_OUT + CATEGORY_IN).as_secs_f32();
        let out_share = CATEGORY_OUT.as_secs_f32() / total;
        let ease = ease_out_quint();
        div()
            .relative()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(list(from, cx))
                    .with_animation(
                        ("list-out", self.changes),
                        Animation::new(CATEGORY_OUT),
                        |layer, delta| layer.opacity(1. - delta),
                    ),
            )
            .child(div().child(list(to, cx)).with_animation(
                ("list-in", self.changes),
                Animation::new(CATEGORY_OUT + CATEGORY_IN),
                move |layer, delta| {
                    let fading_in = ((delta - out_share) / (1. - out_share)).max(0.);
                    layer.opacity(ease(fading_in))
                },
            ))
            .into_any_element()
    }
}

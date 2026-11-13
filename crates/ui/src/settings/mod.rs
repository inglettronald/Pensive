//! The settings view: a rail of categories, the current category's pages, and every setting in
//! one scroll that the rail and the list follow, with a search across all of them.

#[cfg(feature = "mocks")]
mod mocks;
mod outline;
mod search;

use std::time::Duration;

use components::{
    Card, CategoryHeading, Icon, IconName, Kbd, NavItem, NavList, NextCard, PageHeading, Rail,
    RailButton, RailItem, SettingRow,
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, Context, Entity, EventEmitter, FocusHandle,
    Focusable, Pixels, Point, ScrollEvent, SharedString, Subscription, Task, Window, actions,
    deferred, div, ease_out_quint, prelude::*, px,
};
#[cfg(feature = "mocks")]
pub(crate) use mocks::mocks;
use model::Notice;
use outline::{Category, outline};
use search::{Hit, Hits, search};
use settings::{AnySetting, Settings};
use theme::{ActiveTheme, Typeset};

use crate::{
    clear_after, key_binding, notices,
    setting_control::setting_control,
    smooth_scroll::SmoothScroll,
    text_input::{Changed, TextInput},
    view_callback, view_callback_with,
};

actions!(settings_view, [Dismiss, FocusSearch]);

const CONTEXT: &str = "SettingsView";
/// The rail and the list together, the same width as the project's sidebar.
const SIDEBAR: Pixels = px(280.);
/// A page becomes current once its heading crosses this far below the top of the page.
const TRACKING_LINE: Pixels = px(60.);
/// Rows stop here, so a control stays near its name on a wide window. PEN-40 revisits it.
const READING_WIDTH: Pixels = px(680.);
/// How long a click takes to glide to its heading.
const CLICK_GLIDE: Duration = Duration::from_millis(200);
/// How long a wheel notch takes to glide. A trackpad moves the page as it reports.
const WHEEL_GLIDE: Duration = Duration::from_millis(150);
/// When the category changes, its old list fades out, then the new one fades in.
const LIST_OUT: Duration = Duration::from_millis(100);
const LIST_IN: Duration = Duration::from_millis(150);
/// How long a tool's notice stays on it.
const NOTICE_FOR: Duration = Duration::from_secs(3);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        key_binding("escape", Dismiss, Some(CONTEXT), cx),
        key_binding("/", FocusSearch, Some("SettingsView && !TextInput"), cx),
    ]);
}

pub(crate) enum SettingsEvent {
    /// Esc with nothing typed: back to the project.
    Close,
}

/// A page within a category, by their places in the outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Spot {
    category: usize,
    page: usize,
}

impl Spot {
    const START: Spot = Spot {
        category: 0,
        page: 0,
    };
}

/// What a child of the scroll shows, so a scroll position maps to a page.
#[derive(Clone, Copy, PartialEq)]
enum Anchor {
    Category(usize),
    Page(Spot),
    Other,
}

/// The buttons at the rail's foot, which have nothing behind them yet.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    OpenFile,
    ShowNames,
}

impl Tool {
    fn notice(self) -> Notice {
        Notice::not_wired(match self {
            Tool::OpenFile => "Opening settings.toml",
            Tool::ShowNames => "Showing names on the rail",
        })
    }
}

pub(crate) struct SettingsView {
    focus_handle: FocusHandle,
    search: Entity<TextInput>,
    scroll: SmoothScroll,
    /// The scroll's children as of the last render, which the scroll's bounds describe.
    anchors: Vec<Anchor>,
    current: Spot,
    /// Where the current page was before, which the list and the rail slide from.
    previous: Option<Spot>,
    /// The category whose list is fading out.
    fading: Option<(usize, Task<()>)>,
    /// Where a click scrolled to. It stays current until the wheel moves the page, since a page
    /// near the bottom can't reach the tracking line.
    pinned: Option<Spot>,
    /// A page to show as soon as the scroll has laid it out.
    pending: Option<Spot>,
    /// The category being read, and the scroll position, when a search started.
    search_from: Option<(usize, Point<Pixels>)>,
    hovered: Option<usize>,
    hovered_tool: Option<Tool>,
    notice: Option<(Tool, Notice, Task<()>)>,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            let mut input = TextInput::new(cx);
            input.set_placeholder("Search all settings", cx);
            input
        });
        let typed = cx.subscribe(&search, |view, input, _: &Changed, cx| {
            let empty = input.read(cx).text().is_empty();
            match (empty, view.search_from) {
                (false, None) => {
                    let offset = view.scroll.handle().offset();
                    view.search_from = Some((view.current.category, offset));
                    view.scroll.jump_to(px(0.));
                }
                (true, Some((_, offset))) => {
                    view.search_from = None;
                    view.scroll.jump_to(offset.y);
                }
                _ => {}
            }
            cx.notify();
        });
        let changed = cx.observe_global::<Settings>(|_, cx| cx.notify());
        Self {
            focus_handle: cx.focus_handle(),
            search,
            scroll: SmoothScroll::new(),
            anchors: Vec::new(),
            current: Spot::START,
            previous: None,
            fading: None,
            pinned: None,
            pending: None,
            search_from: None,
            hovered: None,
            hovered_tool: None,
            notice: None,
            _subscriptions: [typed, changed],
        }
    }

    fn query(&self, cx: &App) -> SharedString {
        self.search.read(cx).text().clone()
    }

    /// The category a search ranks first: the one being read when it started.
    fn home(&self) -> usize {
        self.search_from
            .map_or(self.current.category, |(category, _)| category)
    }

    /// Makes `spot` current. When `slide`, the list and the rail move to it from the old one, as
    /// they should when the page scrolls. A jump just shows it.
    fn set_current(&mut self, spot: Spot, slide: bool, cx: &mut Context<Self>) {
        if spot == self.current {
            return;
        }
        if !slide {
            self.previous = None;
            self.fading = None;
            self.current = spot;
            return;
        }
        if spot.category != self.current.category {
            let gone = clear_after(LIST_OUT + LIST_IN, |view: &mut Self| view.fading = None, cx);
            self.fading = Some((self.current.category, gone));
        }
        self.previous = Some(self.current);
        self.current = spot;
    }

    /// Makes current the last page whose heading is above the tracking line, as the scroll was
    /// last laid out. At the bottom, it's the last page, which may never reach the line.
    fn track(&mut self, cx: &mut Context<Self>) {
        if self.pinned.is_some() || self.search_from.is_some() {
            return;
        }
        let scroll = self.scroll.handle();
        let scrolled = -scroll.offset().y;
        let line = scroll.bounds().top() + scrolled + TRACKING_LINE;
        let pages = self
            .anchors
            .iter()
            .enumerate()
            .filter_map(|(index, anchor)| match anchor {
                Anchor::Page(spot) => Some((index, *spot)),
                _ => None,
            });
        let mut current = None;
        let mut last = None;
        for (index, spot) in pages {
            last = Some(spot);
            let reached = scroll
                .bounds_for_item(index)
                .is_some_and(|bounds| bounds.top() <= line);
            if reached {
                current = Some(spot);
            }
        }
        let bottom = scroll.max_offset().y;
        if bottom > px(0.) && scrolled >= bottom - px(1.) {
            current = last;
        }
        if let Some(spot) = current {
            self.set_current(spot, true, cx);
        }
    }

    /// Scrolls so `anchor` is at the top, and makes `spot` current until the wheel moves.
    fn scroll_to(&mut self, anchor: Anchor, spot: Spot, smooth: bool, cx: &mut Context<Self>) {
        let scroll = self.scroll.handle();
        let index = self.anchors.iter().position(|known| *known == anchor);
        let Some(item) = index.and_then(|index| scroll.bounds_for_item(index)) else {
            return;
        };
        let to = scroll.bounds().top() - item.top();
        self.set_current(spot, smooth, cx);
        self.pinned = Some(spot);
        if smooth {
            self.scroll.glide_to(to, CLICK_GLIDE, cx);
        } else {
            self.scroll.jump_to(to);
        }
        cx.notify();
    }

    /// A wheel notch glides the page, and a trackpad moves it as it reports. Either way the page
    /// is back to following the scroll.
    fn wheel(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.pinned = None;
        let delta = event.delta.pixel_delta(window.line_height()).y;
        if event.delta.precise() {
            let offset = self.scroll.handle().offset().y;
            self.scroll.jump_to(offset + delta);
        } else {
            self.scroll.glide_by(delta, WHEEL_GLIDE, cx);
        }
        cx.notify();
    }

    fn show_category(&mut self, category: usize, cx: &mut Context<Self>) {
        let spot = Spot { category, page: 0 };
        if self.search_from.is_some() {
            return self.show_after_search(spot, cx);
        }
        self.scroll_to(Anchor::Category(category), spot, true, cx);
    }

    fn show_page(&mut self, spot: Spot, cx: &mut Context<Self>) {
        self.scroll_to(Anchor::Page(spot), spot, true, cx);
    }

    /// Clears the search and shows `spot` among the rest, once they're laid out again.
    fn show_after_search(&mut self, spot: Spot, cx: &mut Context<Self>) {
        self.search_from = None;
        self.search.update(cx, |input, cx| input.set_text("", cx));
        self.pending = Some(spot);
        cx.notify();
    }

    /// Shows the pending page once the scroll's last layout had every page in it.
    fn show_pending(&mut self, cx: &mut Context<Self>) {
        let Some(spot) = self.pending else { return };
        let laid_out = self.anchors.contains(&Anchor::Page(spot))
            && self.scroll.handle().children_count() == self.anchors.len();
        if laid_out {
            self.pending = None;
            let anchor = if spot.page == 0 {
                Anchor::Category(spot.category)
            } else {
                Anchor::Page(spot)
            };
            self.scroll_to(anchor, spot, false, cx);
        } else {
            // Not laid out yet: render again once it is.
            cx.notify();
        }
    }

    fn use_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        let clear = clear_after(NOTICE_FOR, |view: &mut Self| view.notice = None, cx);
        self.notice = Some((tool, tool.notice(), clear));
        cx.notify();
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.query(cx).is_empty() {
            cx.emit(SettingsEvent::Close);
        } else {
            self.search.update(cx, |input, cx| input.set_text("", cx));
        }
    }

    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.search.focus_handle(cx), cx);
    }

    fn rail(&self, outline: &[Category], hits: Option<&Hits>, cx: &mut Context<Self>) -> Rail {
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
                    .filter(|_| hits.is_none())
                    .map(|spot| spot.category),
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

    fn search_field(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
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
                    .size(px(14.))
                    .color(colors.text_faint),
            )
            .child(div().flex_1().min_w_0().child(self.search.clone()))
            .child(Kbd::new(hint))
            .into_any_element()
    }

    /// The current category's title and pages, crossfading from the last category's.
    fn pages_list(&self, outline: &[Category], cx: &mut Context<Self>) -> AnyElement {
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
                .filter(|spot| spot.category == category)
                .map(|spot| spot.page);
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
        let Some(from) = self.fading.as_ref().map(|(from, _)| *from) else {
            return list(to, cx);
        };
        let total = (LIST_OUT + LIST_IN).as_secs_f32();
        let out_share = LIST_OUT.as_secs_f32() / total;
        div()
            .relative()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(list(from, cx))
                    .with_animation(
                        ("list-out", from * 1000 + to),
                        Animation::new(LIST_OUT),
                        |layer, delta| layer.opacity(1. - delta),
                    ),
            )
            .child(div().child(list(to, cx)).with_animation(
                ("list-in", from * 1000 + to),
                Animation::new(LIST_OUT + LIST_IN).with_easing(ease_out_quint()),
                move |layer, delta| layer.opacity(((delta - out_share) / (1. - out_share)).max(0.)),
            ))
            .into_any_element()
    }

    /// The search results, the home category's first.
    fn results_list(
        &self,
        outline: &[Category],
        hits: &Hits,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let here = self.results(
            &hits.here,
            |hit| outline[hit.category].pages[hit.page].name,
            cx,
        );
        let elsewhere = self.results(&hits.elsewhere, |hit| outline[hit.category].name, cx);
        let colors = &cx.theme().colors;
        let density = cx.density();
        let label = |text: SharedString, icon: Option<IconName>| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .pt_2p5()
                .pb_1()
                .px(px(18.))
                .typeset(density.label)
                .text_color(colors.text_faint)
                .children(icon.map(|icon| Icon::new(icon).size(px(14.)).color(colors.text_faint)))
                .child(text.to_uppercase())
        };
        let home = &outline[self.home()];
        let both = !hits.here.is_empty() && !hits.elsewhere.is_empty();
        div()
            .flex()
            .flex_col()
            .when_some(here, |list, here| {
                list.child(label(format!("In {}", home.name).into(), Some(home.icon)))
                    .child(here)
            })
            .when(both, |list| {
                list.child(div().h(px(1.)).mx_4().mt_2p5().bg(colors.border_subtle))
            })
            .when_some(elsewhere, |list, elsewhere| {
                list.child(label("Other categories".into(), None))
                    .child(elsewhere)
            })
            .into_any_element()
    }

    /// `hits` as a list, each naming where it is on the right. Clicking one shows it in place.
    fn results(
        &self,
        hits: &[Hit],
        place: impl Fn(&Hit) -> &'static str,
        cx: &mut Context<Self>,
    ) -> Option<NavList> {
        if hits.is_empty() {
            return None;
        }
        let items = hits
            .iter()
            .map(|hit| NavItem {
                label: hit.setting.name().into(),
                matches: hit.matches.clone(),
                trailing: Some(place(hit).into()),
            })
            .collect();
        let spots: Vec<Spot> = hits
            .iter()
            .map(|hit| Spot {
                category: hit.category,
                page: hit.page,
            })
            .collect();
        let id = if hits[0].category == self.home() {
            "here"
        } else {
            "elsewhere"
        };
        Some(NavList::new(id, items).on_select(view_callback_with(
            cx,
            move |view: &mut Self, index: usize, _, cx| view.show_after_search(spots[index], cx),
        )))
    }

    fn row(setting: &'static dyn AnySetting, hit: Option<&Hit>, cx: &App) -> SettingRow {
        SettingRow::new(
            setting.key(),
            setting.name(),
            setting.description(),
            setting_control(setting, &setting.state(cx)),
        )
        .matches(hit.map(|hit| hit.matches.clone()).unwrap_or_default())
        .changed_from(setting.changed_from(cx))
        .on_reset(move |_, cx| setting.reset(cx))
    }

    /// Every category with its pages, then the card that leads to the next category. Each is
    /// one child of the scroll, with its anchor.
    fn every_page(
        &self,
        outline: &[Category],
        cx: &mut Context<Self>,
    ) -> Vec<(Anchor, AnyElement)> {
        let mut blocks = Vec::new();
        for (index, category) in outline.iter().enumerate() {
            blocks.push((
                Anchor::Category(index),
                CategoryHeading::new(category.icon, category.name, category.description)
                    .into_any_element(),
            ));
            for (page_index, page) in category.pages.iter().enumerate() {
                let heading = PageHeading::new(page.name);
                let heading = if page_index == 0 {
                    heading.first()
                } else {
                    heading
                };
                let rows = page
                    .settings
                    .iter()
                    .map(|setting| Self::row(*setting, None, cx));
                blocks.push((
                    Anchor::Page(Spot {
                        category: index,
                        page: page_index,
                    }),
                    div()
                        .child(heading)
                        .child(
                            div()
                                .mt_1p5()
                                .child(Card::new(("page", index * 100 + page_index)).rows(rows)),
                        )
                        .into_any_element(),
                ));
            }
            if let Some(next) = outline.get(index + 1) {
                let card = NextCard::new(("next", index), next.icon, next.name, next.description)
                    .on_click(view_callback(cx, move |view: &mut Self, _, cx| {
                        view.show_category(index + 1, cx)
                    }));
                blocks.push((Anchor::Other, card.into_any_element()));
            }
        }
        blocks
    }

    /// The matching settings, the home category's under its heading and the others' under
    /// their names, in the list's order.
    fn matching(
        &self,
        outline: &[Category],
        hits: &Hits,
        cx: &mut Context<Self>,
    ) -> Vec<(Anchor, AnyElement)> {
        let colors = &cx.theme().colors;
        let density = cx.density();
        if hits.is_empty() {
            return vec![(
                Anchor::Other,
                div()
                    .pt_7()
                    .px(density.row_padding)
                    .typeset(density.body)
                    .text_color(colors.text_faint)
                    .child(format!("No settings match {}", self.query(cx)))
                    .into_any_element(),
            )];
        }
        let card = |id: &'static str, hits: &[&Hit], cx: &App| {
            Card::new(id).rows(hits.iter().map(|hit| Self::row(hit.setting, Some(hit), cx)))
        };
        let mut blocks = Vec::new();
        if !hits.here.is_empty() {
            let home = &outline[self.home()];
            let here: Vec<&Hit> = hits.here.iter().collect();
            blocks.push((
                Anchor::Other,
                div()
                    .child(CategoryHeading::new(home.icon, home.name, home.description))
                    .child(div().mt_2().child(card("here", &here, cx)))
                    .into_any_element(),
            ));
        }
        if !hits.elsewhere.is_empty() {
            blocks.push((
                Anchor::Other,
                div()
                    .mt_6()
                    .pt_3p5()
                    .border_t_1()
                    .border_color(colors.border_subtle)
                    .px(density.row_padding)
                    .typeset(density.label)
                    .text_color(colors.text_faint)
                    .child("OTHER CATEGORIES")
                    .into_any_element(),
            ));
            let mut start = 0;
            while start < hits.elsewhere.len() {
                let category = hits.elsewhere[start].category;
                let group: Vec<&Hit> = hits.elsewhere[start..]
                    .iter()
                    .take_while(|hit| hit.category == category)
                    .collect();
                start += group.len();
                let entry = &outline[category];
                blocks.push((
                    Anchor::Other,
                    div()
                        .child(CategoryHeading::new(entry.icon, entry.name, "").compact())
                        .child(div().mt_1p5().child(card(entry.name, &group, cx)))
                        .into_any_element(),
                ));
            }
        }
        blocks
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let outline = outline(cx);
        self.track(cx);
        self.show_pending(cx);
        let query = self.query(cx);
        let hits = (!query.is_empty()).then(|| search(&outline, &query, self.home()));
        let blocks = match &hits {
            Some(hits) => self.matching(&outline, hits, cx),
            None => self.every_page(&outline, cx),
        };
        self.anchors = blocks.iter().map(|(anchor, _)| *anchor).collect();
        let list = match &hits {
            Some(hits) => self.results_list(&outline, hits, cx),
            None => self.pages_list(&outline, cx),
        };
        let rail = self.rail(&outline, hits.as_ref(), cx);
        let search_field = self.search_field(window, cx);
        let colors = &cx.theme().colors;
        let density = cx.density();
        let gutter = density.row_padding * 2.;
        let last = blocks.len().saturating_sub(1);
        div()
            .id("settings")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::focus_search))
            .flex()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .w(SIDEBAR)
                    .h_full()
                    .bg(colors.surface_panel)
                    .border_r_1()
                    .border_color(colors.border_subtle)
                    .child(rail)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .pt_2p5()
                            .child(search_field)
                            .child(list),
                    ),
            )
            .child(
                div()
                    .id("settings-page")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .debug_selector(|| "settings-page".into())
                    // Hidden, so the wheel is ours to glide. The tracked handle still offsets the
                    // children and keeps them clipped.
                    .overflow_hidden()
                    .track_scroll(self.scroll.handle())
                    .on_scroll(cx.listener(Self::wheel))
                    .children(blocks.into_iter().enumerate().map(|(index, (_, block))| {
                        div()
                            .max_w(READING_WIDTH)
                            .px(gutter)
                            .when(index == last, |block| block.pb(gutter))
                            .child(block)
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Modifiers, ScrollDelta, TestAppContext, VisualTestContext, point};
    use model::fixtures::{self, PENSIVE};

    use super::*;
    use crate::root::Pensive;

    fn settings_window(cx: &mut TestAppContext) -> (Entity<SettingsView>, &mut VisualTestContext) {
        cx.update(|cx| {
            theme::init(cx);
            crate::init(cx);
        });
        let projects = cx.new(|_| fixtures::projects());
        let (root, cx) =
            cx.add_window_view(|window, cx| Pensive::new(projects, Some(PENSIVE), window, cx));
        root.update_in(cx, |pensive, window, cx| pensive.open_settings(window, cx));
        cx.run_until_parked();
        let view = root.read_with(cx, |pensive, _| pensive.settings_view().unwrap());
        (view, cx)
    }

    fn offset(view: &Entity<SettingsView>, cx: &mut VisualTestContext) -> Pixels {
        view.read_with(cx, |view, _| view.scroll.handle().offset().y)
    }

    fn scroll(delta: ScrollDelta, cx: &mut VisualTestContext) {
        let position = cx.debug_bounds("settings-page").unwrap().center();
        cx.simulate_mouse_move(position, None, Modifiers::none());
        cx.simulate_event(ScrollEvent {
            position,
            delta,
            ..Default::default()
        });
    }

    #[gpui::test]
    fn a_wheel_notch_glides_and_a_trackpad_moves_at_once(cx: &mut TestAppContext) {
        let (view, cx) = settings_window(cx);
        scroll(ScrollDelta::Lines(point(0., -2.)), cx);
        cx.executor().advance_clock(WHEEL_GLIDE / 3);
        let partway = offset(&view, cx);
        cx.executor().advance_clock(WHEEL_GLIDE);
        let done = offset(&view, cx);
        assert!(
            done < partway && partway < px(0.),
            "{partway:?} then {done:?}"
        );

        scroll(ScrollDelta::Pixels(point(px(0.), px(-30.))), cx);
        assert_eq!(offset(&view, cx), done - px(30.));
    }
}

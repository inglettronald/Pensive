//! The settings view: a rail of categories, the current category's pages, and every setting in
//! one scroll that the rail and the list follow, with a search across all of them.

#[cfg(feature = "mocks")]
mod mocks;
mod outline;
mod page;
mod search;
mod sidebar;

use std::time::Duration;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels, Point, ScrollEvent,
    SharedString, Subscription, Task, Window, actions, div, prelude::*, px,
};
#[cfg(feature = "mocks")]
pub(crate) use mocks::mocks;
use model::Notice;
use outline::outline;
use search::search;
use settings::Settings;
use theme::ActiveTheme;

use crate::{
    clear_after, key_binding,
    smooth_scroll::SmoothScroll,
    text_input::{Changed, TextInput},
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
/// How long the list's highlight and the rail's fill take to slide, with a frame to spare.
const SLIDE: Duration = Duration::from_millis(170);
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

/// What to show once every page is laid out again, such as after a search.
#[derive(Clone, Copy)]
enum Pending {
    Spot(Spot),
    /// Where the page was scrolled to before the search.
    Offset(Pixels),
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
    /// Where the current page was before, which the list and the rail slide from, until the
    /// slide is over.
    previous: Option<(Spot, Task<()>)>,
    /// The category whose list is fading out.
    fading: Option<(usize, Task<()>)>,
    /// Where a click scrolled to. It stays current until the wheel moves the page, since a page
    /// near the bottom can't reach the tracking line.
    pinned: Option<Spot>,
    /// What to show as soon as the scroll has laid every page out again.
    pending: Option<Pending>,
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
                    view.pending = Some(Pending::Offset(offset.y));
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
        let done = clear_after(SLIDE, |view: &mut Self| view.previous = None, cx);
        self.previous = Some((self.current, done));
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
        self.pending = Some(Pending::Spot(spot));
        cx.notify();
    }

    /// Shows what's pending once the scroll's last layout had every page in it, so its bounds
    /// and how far it can scroll are the full page's.
    fn show_pending(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending else { return };
        let laid_out = self.scroll.handle().children_count() == self.anchors.len()
            && self
                .anchors
                .iter()
                .any(|anchor| matches!(anchor, Anchor::Page(_)));
        if !laid_out {
            // Render again once it is.
            cx.notify();
            return;
        }
        self.pending = None;
        match pending {
            Pending::Spot(spot) => {
                let anchor = if spot.page == 0 {
                    Anchor::Category(spot.category)
                } else {
                    Anchor::Page(spot)
                };
                self.scroll_to(anchor, spot, false, cx);
            }
            Pending::Offset(offset) => {
                self.scroll.jump_to(offset);
                cx.notify();
            }
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
                            .child(
                                div()
                                    .id("settings-list")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .child(list),
                            ),
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

    use super::*;
    use crate::test_support::pensive_window;

    fn settings_window(cx: &mut TestAppContext) -> (Entity<SettingsView>, &mut VisualTestContext) {
        let (root, cx) = pensive_window(cx);
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

    #[gpui::test]
    fn clearing_a_search_goes_back_to_where_the_page_was(cx: &mut TestAppContext) {
        let (view, cx) = settings_window(cx);
        scroll(ScrollDelta::Pixels(point(px(0.), px(-300.))), cx);
        let before = offset(&view, cx);
        assert!(before < px(0.));

        let search = view.read_with(cx, |view, _| view.search.clone());
        search.update(cx, |input, cx| input.set_text("show", cx));
        cx.run_until_parked();
        assert_eq!(offset(&view, cx), px(0.));

        view.update_in(cx, |view, window, cx| view.dismiss(&Dismiss, window, cx));
        // The page is put back once every page is laid out again, a frame or two later.
        for _ in 0..3 {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        }
        assert_eq!(offset(&view, cx), before);
    }
}

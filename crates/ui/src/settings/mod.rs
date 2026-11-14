//! The settings view: a rail of categories, the current category's pages, and that category's
//! settings in a scroll the list follows, with a search across every category.

#[cfg(feature = "mocks")]
mod mocks;
mod outline;
mod page;
mod search;
mod sidebar;

use std::time::Duration;

use components::NextCard;
use gpui::{
    Animation, AnimationExt, AnyElement, App, Context, Entity, EventEmitter, FocusHandle,
    Focusable, Pixels, Point, ScrollEvent, SharedString, Subscription, Task, Window, actions, div,
    ease_out_quint, linear_color_stop, linear_gradient, prelude::*, relative,
};
#[cfg(feature = "mocks")]
pub(crate) use mocks::mocks;
use model::Notice;
use outline::outline;
use search::search;
use settings::Settings;
use theme::{ActiveTheme, Dp, dp};

use crate::{
    clear_after, key_binding,
    smooth_scroll::SmoothScroll,
    text_input::{Changed, TextInput},
    view_callback,
};

actions!(settings_view, [Dismiss, FocusSearch]);

const CONTEXT: &str = "SettingsView";
/// The rail and the list together, the same width as the project's sidebar.
const SIDEBAR: Dp = dp(280.);
/// A page becomes current once its heading crosses this far below the top of the page.
const TRACKING_LINE: Dp = dp(60.);
/// Rows stop here, so a control stays near its name on a wide window. PEN-40 revisits it.
const READING_WIDTH: Dp = dp(680.);
/// How long a click takes to glide to its heading.
const CLICK_GLIDE: Duration = Duration::from_millis(200);
/// How long a wheel notch takes to glide. A trackpad moves the page as it reports.
const WHEEL_GLIDE: Duration = Duration::from_millis(150);
/// How long the list's highlight and the rail's fill take to slide, with a frame to spare.
const SLIDE: Duration = Duration::from_millis(170);
/// When the category changes, its old list and page fade out, then the new ones fade in.
const CATEGORY_OUT: Duration = Duration::from_millis(100);
const CATEGORY_IN: Duration = Duration::from_millis(150);
/// How far the new category's page rises as it fades in, and the next card as it docks.
const RISE: Dp = dp(16.);
/// How long the next card takes to dock under the last page, or to leave.
const DOCK: Duration = Duration::from_millis(150);
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

/// What a child of the scroll shows, so a scroll position maps to a page. A category's heading
/// is part of its first page.
#[derive(Clone, Copy, PartialEq)]
enum Anchor {
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
    /// The category fading out after a change, with where its page was scrolled to.
    fading: Option<(usize, Pixels, Task<()>)>,
    /// Counts category changes, so each one's fade starts over.
    changes: usize,
    /// Where a click scrolled to. It stays current until the glide lands, so the list doesn't
    /// step through the pages on the way.
    pinned: Option<Spot>,
    /// Whether the next card was docked when last drawn. `None` shows it as it is without
    /// sliding, such as after a jump.
    docked: Option<bool>,
    /// The next card docking or leaving, until it's done.
    docking: Option<Task<()>>,
    /// Counts the next card's moves, so each one's slide starts over.
    dock_moves: usize,
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
                    view.scroll.jump_to(Pixels::ZERO);
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
            changes: 0,
            pinned: None,
            docked: None,
            docking: None,
            dock_moves: 0,
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
            self.docked = None;
            self.current = spot;
            return;
        }
        if spot.category != self.current.category {
            let gone = clear_after(
                CATEGORY_OUT + CATEGORY_IN,
                |view: &mut Self| view.fading = None,
                cx,
            );
            let offset = self.scroll.handle().offset().y;
            self.fading = Some((self.current.category, offset, gone));
            self.changes += 1;
        }
        let done = clear_after(SLIDE, |view: &mut Self| view.previous = None, cx);
        self.previous = Some((self.current, done));
        self.current = spot;
    }

    /// Makes current the last page whose heading is above the tracking line, as the scroll was
    /// last laid out. The last page can always reach it, since the page leaves room below it.
    fn track(&mut self, window: &Window, cx: &mut Context<Self>) {
        let gliding_to_click = self.pinned.is_some() && self.scroll.is_gliding();
        if self.search_from.is_some() || gliding_to_click {
            return;
        }
        self.pinned = None;
        let scroll = self.scroll.handle();
        let line =
            scroll.bounds().top() - scroll.offset().y + TRACKING_LINE.to_pixels(window.rem_size());
        let category = self.current.category;
        let mut current = None;
        for (index, anchor) in self.anchors.iter().enumerate() {
            let Anchor::Page(spot) = *anchor else {
                continue;
            };
            // Right after a change, the last layout is still the old category's.
            if spot.category != category {
                return;
            }
            let reached = scroll
                .bounds_for_item(index)
                .is_some_and(|bounds| bounds.top() <= line);
            if reached || current.is_none() {
                current = Some(spot);
            }
        }
        if let Some(spot) = current {
            self.set_current(spot, true, cx);
        }
    }

    /// Scrolls so `anchor` is at the top, and makes `spot` current.
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

    /// Fades the page over to `category`, from its top. The current category scrolls to its top.
    fn show_category(&mut self, category: usize, cx: &mut Context<Self>) {
        let spot = Spot { category, page: 0 };
        if self.search_from.is_some() {
            return self.show_after_search(spot, cx);
        }
        if category == self.current.category {
            return self.scroll_to(Anchor::Page(spot), spot, true, cx);
        }
        self.pinned = None;
        self.set_current(spot, true, cx);
        self.scroll.jump_to(Pixels::ZERO);
        cx.notify();
    }

    fn show_page(&mut self, spot: Spot, cx: &mut Context<Self>) {
        self.scroll_to(Anchor::Page(spot), spot, true, cx);
    }

    /// Shows `spot` without a glide, once its category is laid out.
    fn jump_to(&mut self, spot: Spot, cx: &mut Context<Self>) {
        self.set_current(spot, false, cx);
        self.pending = Some(Pending::Spot(spot));
        cx.notify();
    }

    /// Clears the search and shows `spot` in its category.
    fn show_after_search(&mut self, spot: Spot, cx: &mut Context<Self>) {
        self.search_from = None;
        self.search.update(cx, |input, cx| input.set_text("", cx));
        self.jump_to(spot, cx);
    }

    /// Slides the next card in or out when `docked` changes, unless it was just jumped to.
    fn dock(&mut self, docked: bool, cx: &mut Context<Self>) {
        if self.docked.is_some_and(|was| was != docked) {
            self.dock_moves += 1;
            self.docking = Some(clear_after(DOCK, |view: &mut Self| view.docking = None, cx));
        } else if self.docked.is_none() {
            self.docking = None;
        }
        self.docked = Some(docked);
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
            Pending::Spot(spot) => self.scroll_to(Anchor::Page(spot), spot, false, cx),
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

impl SettingsView {
    /// The page beside the sidebar, fading in after a category change.
    fn page_column(
        &self,
        blocks: Vec<(Anchor, AnyElement)>,
        outline: &[outline::Category],
        browsing: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let density = cx.density();
        let gutter = density.row_padding * 2.;
        let docked_room = NextCard::height(density) + gutter * 2.;
        let category = self.current.category;
        let has_next = |category: usize| browsing && category + 1 < outline.len();
        let end = |category: usize| {
            browsing.then(|| match has_next(category) {
                true => docked_room,
                false => gutter,
            })
        };
        let page = div()
            .id("settings-page")
            .relative()
            .size_full()
            .debug_selector(|| "settings-page".into())
            // Hidden, so the wheel is ours to glide. The tracked handle still offsets the
            // children and keeps them clipped.
            .overflow_hidden()
            .track_scroll(self.scroll.handle())
            .on_scroll(cx.listener(Self::wheel))
            .children(column(blocks, end(category), gutter));
        let fading = self.fading.as_ref().filter(|_| browsing);
        let page = match fading {
            Some(_) => {
                let total = (CATEGORY_OUT + CATEGORY_IN).as_secs_f32();
                let out_share = CATEGORY_OUT.as_secs_f32() / total;
                let ease = ease_out_quint();
                page.with_animation(
                    ("page-in", self.changes),
                    Animation::new(CATEGORY_OUT + CATEGORY_IN),
                    move |page, delta| {
                        let shown = ease(((delta - out_share) / (1. - out_share)).max(0.));
                        page.opacity(shown).top(RISE * (1. - shown))
                    },
                )
                .into_any_element()
            }
            None => page.into_any_element(),
        };
        let leaving = fading.map(|(from, offset, _)| {
            let old = self.category_page(outline, *from, cx);
            div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(*offset)
                        .children(column(old, end(*from), gutter)),
                )
                .with_animation(
                    ("page-out", self.changes),
                    Animation::new(CATEGORY_OUT),
                    |layer, delta| layer.opacity(1. - delta),
                )
        });
        let shown = self.docked == Some(true);
        let dock = (has_next(category) && (shown || self.docking.is_some()))
            .then(|| self.next_card(&outline[category + 1], category + 1, gutter, cx))
            .map(|dock| match self.docking {
                Some(_) => dock
                    .with_animation(
                        ("dock", self.dock_moves),
                        Animation::new(DOCK).with_easing(ease_out_quint()),
                        move |dock, delta| {
                            let seen = if shown { delta } else { 1. - delta };
                            dock.opacity(seen).bottom(-(RISE * (1. - seen)))
                        },
                    )
                    .into_any_element(),
                None => dock.into_any_element(),
            });
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(page)
            .children(leaving)
            .children(dock)
            .into_any_element()
    }

    /// The card that leads to `next`, docked under the page with the page fading out above it.
    fn next_card(
        &self,
        next: &outline::Category,
        index: usize,
        gutter: Dp,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let surface = cx.theme().colors.surface_app;
        let card = NextCard::new(("next", index), next.icon, next.name, next.description).on_click(
            view_callback(cx, move |view: &mut Self, _, cx| {
                view.show_category(index, cx)
            }),
        );
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .child(div().h(gutter).bg(linear_gradient(
                180.,
                linear_color_stop(surface.opacity(0.), 0.),
                linear_color_stop(surface, 1.),
            )))
            .child(
                div()
                    .bg(surface)
                    .pb(gutter)
                    .child(div().max_w(READING_WIDTH).px(gutter).child(card)),
            )
    }
}

/// The page's blocks at the reading width. Given `end`, the last block is at least as tall as the
/// page, so it can scroll to the top, and leaves `end` under it for the next card.
fn column(
    blocks: Vec<(Anchor, AnyElement)>,
    end: Option<Dp>,
    gutter: Dp,
) -> impl Iterator<Item = AnyElement> {
    let last = blocks.len().saturating_sub(1);
    blocks
        .into_iter()
        .enumerate()
        .map(move |(index, (_, block))| {
            div()
                .max_w(READING_WIDTH)
                .px(gutter)
                .when(index == last, |block| match end {
                    Some(end) => block.min_h(relative(1.)).pb(end),
                    None => block.pb(gutter),
                })
                .child(block)
                .into_any_element()
        })
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let outline = outline(cx);
        self.track(window, cx);
        self.show_pending(cx);
        let query = self.query(cx);
        let hits = (!query.is_empty()).then(|| search(&outline, &query, self.home()));
        let blocks = match &hits {
            Some(hits) => self.matching(&outline, hits, cx),
            None => self.category_page(&outline, self.current.category, cx),
        };
        self.anchors = blocks.iter().map(|(anchor, _)| *anchor).collect();
        let browsing = hits.is_none();
        let last_page = outline
            .get(self.current.category)
            .map_or(0, |category| category.pages.len().saturating_sub(1));
        self.dock(browsing && self.current.page == last_page, cx);
        let list = match &hits {
            Some(hits) => self.results_list(&outline, hits, cx),
            None => self.pages_list(&outline, cx),
        };
        let rail = self.rail(&outline, hits.as_ref(), cx);
        let search_field = self.search_field(window, cx);
        let page = self.page_column(blocks, &outline, browsing, cx);
        let colors = &cx.theme().colors;
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
            .child(page)
    }
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "tests click and scroll at window pixels"
)]
mod tests {
    use gpui::{Modifiers, ScrollDelta, TestAppContext, VisualTestContext, point, px};

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

    /// Lets every glide and fade finish a frame at a time, and draws the frames that follow.
    fn settle(cx: &mut VisualTestContext) {
        for _ in 0..30 {
            cx.executor().advance_clock(Duration::from_millis(16));
            cx.run_until_parked();
        }
        for _ in 0..3 {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        }
    }

    fn current(view: &Entity<SettingsView>, cx: &mut VisualTestContext) -> Spot {
        view.read_with(cx, |view, _| view.current)
    }

    #[gpui::test]
    fn clicking_any_page_scrolls_it_to_the_top(cx: &mut TestAppContext) {
        let (view, cx) = settings_window(cx);
        let pages: Vec<usize> = view.update(cx, |_, cx| {
            outline(cx)
                .iter()
                .map(|category| category.pages.len())
                .collect()
        });
        for (category, count) in pages.into_iter().enumerate() {
            view.update(cx, |view, cx| view.show_category(category, cx));
            settle(cx);
            for page in 0..count {
                let spot = Spot { category, page };
                view.update(cx, |view, cx| view.show_page(spot, cx));
                settle(cx);
                // Items keep their unscrolled bounds, so add the offset to see where one shows.
                let (top, shown) = view.read_with(cx, |view, _| {
                    let scroll = view.scroll.handle();
                    let index = view.anchors.iter().position(|a| *a == Anchor::Page(spot));
                    let item = index.and_then(|index| scroll.bounds_for_item(index));
                    let shown = item.map(|item| item.top() + scroll.offset().y);
                    (scroll.bounds().top(), shown)
                });
                assert_eq!(shown, Some(top), "{spot:?}");
                assert_eq!(current(&view, cx), spot);
            }
        }
    }

    #[gpui::test]
    fn the_wheel_stops_at_the_end_of_a_category(cx: &mut TestAppContext) {
        let (view, cx) = settings_window(cx);
        let last = view.update(cx, |_, cx| outline(cx)[0].pages.len() - 1);
        scroll(ScrollDelta::Pixels(point(px(0.), px(-10_000.))), cx);
        settle(cx);
        assert_eq!(
            current(&view, cx),
            Spot {
                category: 0,
                page: last
            }
        );
    }

    #[gpui::test]
    fn a_category_heading_is_part_of_its_first_page(cx: &mut TestAppContext) {
        let (view, cx) = settings_window(cx);
        view.update(cx, |view, cx| view.show_category(1, cx));
        settle(cx);
        scroll(ScrollDelta::Pixels(point(px(0.), px(-1.))), cx);
        settle(cx);
        assert_eq!(
            current(&view, cx),
            Spot {
                category: 1,
                page: 0
            }
        );
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

//! The mock browser: every mock by subject with its thumbnail, and the mock opened from it in
//! place of the window's content.

use std::{collections::HashMap, sync::Arc};

use components::{Breadcrumb, Icon, IconName, Kbd, ListRow, TitleBar};
use gpui::{
    AnyElement, AnyView, App, Context, Entity, EventEmitter, FocusHandle, Focusable, Global,
    ObjectFit, RenderImage, SharedString, Subscription, Window, actions, div, img, prelude::*, px,
};
use mock::{Mock, atlas};
use theme::{ActiveTheme, Density, Theme, Typeset};

use crate::{
    key_binding,
    text_input::{self, TextInput},
    view_callback,
    window_frame::draws_window_controls,
};

actions!(
    mock_browser,
    [ViewMocks, NextSubject, PreviousSubject, OpenMock, Back]
);

const CONTEXT: &str = "MockBrowser";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        key_binding("secondary-shift-m", ViewMocks, None, cx),
        key_binding("down", NextSubject, Some(CONTEXT), cx),
        key_binding("up", PreviousSubject, Some(CONTEXT), cx),
        key_binding("enter", OpenMock, Some(CONTEXT), cx),
        key_binding("escape", Back, Some(CONTEXT), cx),
    ]);
}

/// Each mock's thumbnail by its id, filled in as they render.
#[derive(Default)]
pub struct MockThumbnails(HashMap<String, Arc<RenderImage>>);

impl Global for MockThumbnails {}

impl MockThumbnails {
    pub fn insert(id: String, thumbnail: Arc<RenderImage>, cx: &mut App) {
        cx.default_global::<Self>().0.insert(id, thumbnail);
    }
}

/// The views' mocks, then the components'.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Views,
    Components,
}

struct Subject {
    name: &'static str,
    group: Group,
    mocks: Vec<Mock>,
}

pub enum MockBrowserEvent {
    Open { mock: Mock, component: bool },
    Back,
}

pub struct MockBrowser {
    subjects: Vec<Subject>,
    input: Entity<TextInput>,
    /// Index into `subjects`.
    selected: usize,
    /// Index into the selected subject's mocks that match the filter.
    focused: usize,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<MockBrowserEvent> for MockBrowser {}

/// Focus goes to the filter, so typing narrows the mocks at once.
impl Focusable for MockBrowser {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl MockBrowser {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut subjects: Vec<Subject> = Vec::new();
        let groups = [
            (Group::Views, crate::mocks()),
            (Group::Components, components::mocks()),
        ];
        for (group, mocks) in groups {
            for mock in mocks {
                match subjects.last_mut() {
                    Some(subject) if subject.name == mock.subject => subject.mocks.push(mock),
                    _ => subjects.push(Subject {
                        name: mock.subject,
                        group,
                        mocks: vec![mock],
                    }),
                }
            }
        }
        let input = cx.new(|cx| {
            let mut input = TextInput::new(cx);
            input.set_placeholder("Filter mocks", cx);
            input
        });
        let subscriptions = vec![
            cx.subscribe(&input, |this: &mut Self, _, _, cx| this.filter_changed(cx)),
            cx.observe_global::<MockThumbnails>(|_, cx| cx.notify()),
        ];
        Self {
            subjects,
            input,
            selected: 0,
            focused: 0,
            _subscriptions: subscriptions,
        }
    }

    /// The mocks of `subject` whose `<subject>/<name>` contains the filter, as `cargo mocks`
    /// matches them.
    fn matching<'a>(&self, subject: &'a Subject, cx: &App) -> Vec<&'a Mock> {
        let filter = self.input.read(cx).text();
        subject
            .mocks
            .iter()
            .filter(|mock| mock.id().contains(filter.as_ref()))
            .collect()
    }

    /// Indexes of the subjects with a mock that matches the filter.
    fn visible(&self, cx: &App) -> Vec<usize> {
        (0..self.subjects.len())
            .filter(|&index| !self.matching(&self.subjects[index], cx).is_empty())
            .collect()
    }

    fn filter_changed(&mut self, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        if !visible.contains(&self.selected)
            && let Some(&first) = visible.first()
        {
            self.selected = first;
        }
        self.focused = 0;
        cx.notify();
    }

    /// Shows the subject called `name`, as clicking it in the list does.
    pub fn select_subject(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Some(index) = self
            .subjects
            .iter()
            .position(|subject| subject.name == name)
        {
            self.select(index, cx);
        }
    }

    pub fn set_filter(&mut self, filter: &str, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_text(filter, cx));
        self.filter_changed(cx);
    }

    fn select(&mut self, subject: usize, cx: &mut Context<Self>) {
        self.selected = subject;
        self.focused = 0;
        cx.notify();
    }

    fn step_subject(&mut self, step: isize, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        let Some(position) = visible.iter().position(|&index| index == self.selected) else {
            return;
        };
        let position = position.saturating_add_signed(step).min(visible.len() - 1);
        self.select(visible[position], cx);
    }

    fn step_mock(&mut self, step: isize, cx: &mut Context<Self>) {
        let count = self.matching(&self.subjects[self.selected], cx).len();
        self.focused = self
            .focused
            .saturating_add_signed(step)
            .min(count.saturating_sub(1));
        cx.notify();
    }

    fn open(&mut self, index: usize, cx: &mut Context<Self>) {
        let subject = &self.subjects[self.selected];
        if let Some(mock) = self.matching(subject, cx).get(index) {
            cx.emit(MockBrowserEvent::Open {
                mock: (*mock).clone(),
                component: subject.group == Group::Components,
            });
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut group = None;
        for index in self.visible(cx) {
            let subject = &self.subjects[index];
            if group != Some(subject.group) {
                group = Some(subject.group);
                rows.push(
                    div()
                        .pt_2p5()
                        .pb_1()
                        .px_4()
                        .typeset(density.label)
                        .text_color(colors.text_faint)
                        .child(match subject.group {
                            Group::Views => "Views",
                            Group::Components => "Components",
                        })
                        .into_any_element(),
                );
            }
            rows.push(
                div()
                    .px_2()
                    .child(
                        ListRow::new(("subject", index), subject.name)
                            .trailing(
                                div()
                                    .typeset(density.small)
                                    .text_color(colors.text_faint)
                                    .child(self.matching(subject, cx).len().to_string()),
                            )
                            .selected(index == self.selected)
                            .on_click(view_callback(cx, move |this: &mut Self, _, cx| {
                                this.select(index, cx)
                            })),
                    )
                    .into_any_element(),
            );
        }
        div()
            .id("subjects")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(220.))
            .pb_2()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_panel)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .m_2()
                    .h(px(28.))
                    .px_2()
                    .rounded(density.radius_md)
                    .border_1()
                    .border_color(colors.border_subtle)
                    .bg(colors.surface_raised)
                    .typeset(density.body)
                    .child(
                        Icon::new(IconName::Search)
                            .size(px(14.))
                            .color(colors.text_faint),
                    )
                    .child(div().flex_1().child(self.input.clone())),
            )
            .children(rows)
    }

    fn grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let subject = &self.subjects[self.selected];
        let mocks = self.matching(subject, cx);
        let largest = atlas::largest(mocks.iter().map(|mock| mock.size));
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let thumbnails = cx.try_global::<MockThumbnails>();
        let cards = mocks.iter().enumerate().map(|(index, mock)| {
            let thumbnail = thumbnails.and_then(|thumbnails| thumbnails.0.get(&mock.id()));
            div()
                .id(("mock", index))
                .flex()
                .flex_col()
                .min_w_0()
                .rounded(density.radius_md)
                .border_1()
                .border_color(if index == self.focused {
                    colors.border_focus
                } else {
                    colors.border
                })
                .overflow_hidden()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.open(index, cx)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .h(px(30.))
                        .px_2p5()
                        .border_b_1()
                        .border_color(colors.border_subtle)
                        .bg(colors.surface_raised)
                        .typeset(density.small)
                        .child(
                            div()
                                .text_color(colors.text_faint)
                                .child(format!("{:02}", index + 1)),
                        )
                        .child(
                            div()
                                .typeset(density.label)
                                .text_color(colors.text_strong)
                                .truncate()
                                .child(mock.name),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w_full()
                        .aspect_ratio(largest.width / largest.height)
                        .bg(colors.surface_app)
                        .map(|thumb| match thumbnail {
                            Some(image) => thumb.child(
                                img(image.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Contain),
                            ),
                            None => thumb
                                .bg(colors.surface_panel)
                                .typeset(density.small)
                                .text_color(colors.text_disabled)
                                .child("Rendering"),
                        }),
                )
        });
        div()
            .id("mocks")
            .flex_1()
            .min_w_0()
            .pt_5()
            .px_6()
            .pb_6()
            .overflow_y_scroll()
            .child(
                div()
                    .typeset(density.heading)
                    .text_color(colors.text_strong)
                    .child(subject.name),
            )
            .child(
                div()
                    .mt_0p5()
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .child(atlas::summary(subject.name, mocks.len(), largest)),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(atlas::columns(mocks.len()) as u16)
                    .gap_4()
                    .mt_4()
                    .children(cards),
            )
    }
}

impl Render for MockBrowser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors.clone();
        let density = cx.density().clone();
        let hint = |keys: &'static str, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .child(Kbd::new(keys))
                .child(label)
        };
        let location = Breadcrumb::new([
            div().text_color(colors.text_muted).child("Mocks"),
            div()
                .text_color(colors.text_strong)
                .child(SharedString::from(self.subjects[self.selected].name)),
        ])
        .chevron_size(px(14.));
        div()
            .key_context(CONTEXT)
            .on_action(cx.listener(|this, _: &NextSubject, _, cx| this.step_subject(1, cx)))
            .on_action(cx.listener(|this, _: &PreviousSubject, _, cx| this.step_subject(-1, cx)))
            .on_action(cx.listener(|this, _: &OpenMock, _, cx| this.open(this.focused, cx)))
            .on_action(cx.listener(|_, _: &Back, _, cx| cx.emit(MockBrowserEvent::Back)))
            // Left and right pick a mock rather than move the filter's cursor.
            .capture_action(cx.listener(|this, _: &text_input::Left, _, cx| {
                this.step_mock(-1, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &text_input::Right, _, cx| {
                this.step_mock(1, cx);
                cx.stop_propagation();
            }))
            .flex()
            .flex_col()
            .size_full()
            .child(
                TitleBar::new(None)
                    .location(div().pl_1p5().typeset(density.body).child(location))
                    .controls(draws_window_controls(window)),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(self.grid(cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_4()
                    .h(px(36.))
                    .px_3()
                    .border_t_1()
                    .border_color(colors.border_subtle)
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .child(hint("Enter", "Open"))
                    .child(hint("↑↓", "Subject"))
                    .child(hint("←→", "Mock"))
                    .child(hint("Esc", "Back to where you were")),
            )
    }
}

/// A mock opened from the browser.
pub(crate) struct ShownMock {
    mock: Mock,
    view: AnyView,
    component: bool,
}

/// A window's visit to the mocks: the browser, the mock opened from it, and the theme and
/// density to put back, since mocks such as the Dense ones change them for the whole app.
pub(crate) struct MockMode {
    pub browser: Entity<MockBrowser>,
    pub shown: Option<ShownMock>,
    saved: (Theme, Density),
    _subscription: Subscription,
}

impl MockMode {
    pub fn new(browser: Entity<MockBrowser>, subscription: Subscription, cx: &App) -> Self {
        Self {
            browser,
            shown: None,
            saved: (cx.theme().clone(), cx.density().clone()),
            _subscription: subscription,
        }
    }

    /// Opens `mock` on the theme and density the window had before the mocks.
    pub fn show(&mut self, mock: Mock, component: bool, window: &mut Window, cx: &mut App) {
        self.restore(cx);
        let view = mock.build(window, cx);
        self.shown = Some(ShownMock {
            mock,
            view,
            component,
        });
    }

    /// Back to the browser, or out of the mocks when `self` is dropped next.
    pub fn restore(&mut self, cx: &mut App) {
        self.shown = None;
        cx.set_global(self.saved.0.clone());
        cx.set_global(self.saved.1.clone());
    }

    /// A view mock fills the window. A component mock sits centered at its size, labeled, so
    /// its edges show.
    pub fn render(&self, cx: &App) -> AnyElement {
        let Some(shown) = &self.shown else {
            return self.browser.clone().into_any_element();
        };
        if !shown.component {
            return shown.view.clone().into_any_element();
        }
        let colors = &cx.theme().colors;
        let density = cx.density();
        let size = shown.mock.size;
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                div()
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .child(format!(
                        "{} · {} × {}",
                        shown.mock.id().replace('/', " / "),
                        size.width.as_f32(),
                        size.height.as_f32()
                    )),
            )
            .child(
                div()
                    .w(size.width)
                    .h(size.height)
                    .flex_none()
                    .rounded(density.radius_md)
                    .border_1()
                    .border_color(colors.border_subtle)
                    .overflow_hidden()
                    .child(shown.view.clone()),
            )
            .into_any_element()
    }
}

pub(crate) fn mocks() -> Vec<Mock> {
    use model::fixtures::{self, PENSIVE};

    use crate::{palette::declare_settings, root::Pensive};

    /// The pensive project's window after Ctrl Shift M, set up by `setup`.
    fn browsing(
        setup: fn(&mut Pensive, &mut Window, &mut Context<Pensive>),
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView {
        declare_settings(cx);
        let projects = cx.new(|_| fixtures::projects());
        let pensive = cx.new(|cx| Pensive::new(projects, Some(PENSIVE), window, cx));
        pensive.update(cx, |pensive, cx| {
            pensive.view_mocks(window, cx);
            setup(pensive, window, cx);
        });
        pensive.into()
    }

    fn browser(
        pensive: &Pensive,
        cx: &mut App,
        update: impl FnOnce(&mut MockBrowser, &mut Context<MockBrowser>),
    ) {
        if let Some(browser) = pensive.mock_browser() {
            browser.update(cx, update);
        }
    }

    vec![
        Mock::new("mock_browser", "browsing", |window, cx| {
            browsing(
                |pensive, _, cx| {
                    browser(pensive, cx, |browser, cx| {
                        browser.select_subject("project", cx)
                    })
                },
                window,
                cx,
            )
        }),
        Mock::new("mock_browser", "filtered", |window, cx| {
            browsing(
                |pensive, _, cx| {
                    browser(pensive, cx, |browser, cx| browser.set_filter("staged", cx))
                },
                window,
                cx,
            )
        }),
        Mock::new("mock_browser", "component_open", |window, cx| {
            browsing(
                |pensive, window, cx| {
                    let tooltip = components::mocks()
                        .into_iter()
                        .find(|mock| mock.subject == "tooltip");
                    if let Some(tooltip) = tooltip {
                        pensive.open_mock(tooltip, true, window, cx);
                    }
                },
                window,
                cx,
            )
        }),
    ]
}

//! The view that fills each window: the title bar over the landing screen with no project open,
//! otherwise over the project, or over Settings in place of either, with the command palette over
//! them when Ctrl K opens it.

#[cfg(feature = "mocks")]
mod mocks;

use components::{TitleBar, TitleBarProject};
use gpui::{
    App, Bounds, Context, DisplayId, Entity, FocusHandle, Focusable, MouseButton, Pixels, Size,
    Subscription, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions, actions,
    div, prelude::*, px, size, svg,
};
#[cfg(feature = "mocks")]
pub(crate) use mocks::mocks;
use model::{ProjectId, Projects};
use theme::{ActiveTheme, Typeset, dp};

#[cfg(feature = "mocks")]
use crate::mock_browser::{MockBrowser, MockBrowserEvent, MockMode, ViewMocks};
use crate::{
    key_binding,
    notices::NoticeOverlay,
    palette::{Keyword, OPEN_ON, PaletteEvent, PaletteView, Tab, scrim},
    project::ProjectView,
    settings::{SettingsEvent, SettingsView},
    view_callback, view_callback_with,
    window_frame::{draws_window_controls, window_frame},
};

actions!(
    pensive,
    [
        OpenPalette,
        OpenActive,
        ClosePalette,
        OpenSettings,
        ZoomIn,
        ZoomOut,
        ResetZoom
    ]
);

/// Set on the window while the palette is open over a project, so Escape closes it wherever
/// focus is.
const PALETTE_OVERLAY: &str = "PaletteOverlay";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        key_binding("secondary-k", OpenPalette, None, cx),
        key_binding("secondary-shift-k", OpenActive, None, cx),
        key_binding("secondary-,", OpenSettings, None, cx),
        key_binding("secondary-=", ZoomIn, None, cx),
        key_binding("secondary--", ZoomOut, None, cx),
        key_binding("secondary-0", ResetZoom, None, cx),
        key_binding("escape", ClosePalette, Some(PALETTE_OVERLAY), cx),
    ]);
}

pub struct Pensive {
    projects: Entity<Projects>,
    /// The project shown, with its view.
    project: Option<(ProjectId, Entity<ProjectView>)>,
    palette: Entity<PaletteView>,
    palette_open: bool,
    /// Shown in place of the project or the landing screen while open.
    settings: Option<(Entity<SettingsView>, Subscription)>,
    notices: Entity<NoticeOverlay>,
    /// Set while the window shows the mock browser, or a mock opened from it.
    #[cfg(feature = "mocks")]
    mocks: Option<MockMode>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

impl Pensive {
    /// A window on `project`, or the landing screen when it's `None`.
    pub fn new(
        projects: Entity<Projects>,
        project: Option<ProjectId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let palette = cx.new(|cx| PaletteView::new(projects.clone(), cx));
        let notices = cx.new(NoticeOverlay::new);
        cx.observe(&notices, |_, _, cx| cx.notify()).detach();
        let subscription = cx.subscribe_in(&palette, window, Self::on_palette_event);
        let mut pensive = Self {
            projects,
            project: None,
            palette,
            palette_open: false,
            settings: None,
            notices,
            #[cfg(feature = "mocks")]
            mocks: None,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
        };
        match project {
            None => pensive.open_palette(None, window, cx),
            Some(project) => pensive.show_project(project, window, cx),
        }
        pensive
    }

    /// Shows Settings in place of the project or the landing screen.
    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(feature = "mocks")]
        self.leave_mocks(cx);
        let view = match &self.settings {
            Some((view, _)) => view.clone(),
            None => {
                let view = cx.new(SettingsView::new);
                let subscription =
                    cx.subscribe_in(&view, window, |pensive, _, event, window, cx| match event {
                        SettingsEvent::Close => pensive.close_settings(window, cx),
                    });
                self.settings = Some((view.clone(), subscription));
                view
            }
        };
        self.palette_open = false;
        window.focus(&view.focus_handle(cx), cx);
        cx.notify();
    }

    /// Back to the project, or to the landing screen and its palette.
    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = None;
        self.focus_content(window, cx);
        cx.notify();
    }

    #[cfg(any(test, feature = "mocks"))]
    pub(crate) fn settings_view(&self) -> Option<Entity<SettingsView>> {
        Some(self.settings.as_ref()?.0.clone())
    }

    #[cfg(feature = "mocks")]
    pub(crate) fn project_view(&self) -> Option<Entity<ProjectView>> {
        Some(self.project.as_ref()?.1.clone())
    }

    fn project_id(&self) -> Option<ProjectId> {
        self.project.as_ref().map(|(id, _)| *id)
    }

    /// Opens the palette on `tab`, or on the tab the `palette.open_on` setting names. On the
    /// landing screen it's already open, so this starts over, and Settings closes to show it.
    pub fn open_palette(&mut self, tab: Option<Tab>, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.is_none() {
            self.settings = None;
        }
        let tab = tab.unwrap_or_else(|| OPEN_ON.get(cx));
        let (project, overlay) = (self.project_id(), self.project.is_some());
        self.palette
            .update(cx, |palette, cx| palette.reset(project, tab, overlay, cx));
        self.palette_open = true;
        window.focus(&self.palette.focus_handle(cx), cx);
        cx.notify();
    }

    /// Opens the palette narrowed to `keyword`, as the title bar's crumbs do.
    fn open_palette_scoped(
        &mut self,
        keyword: Keyword,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_palette(None, window, cx);
        self.palette
            .update(cx, |palette, cx| palette.set_keyword(Some(keyword), cx));
    }

    /// Shows `project`, with a fresh view unless it's the one already shown.
    fn show_project(&mut self, project: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(feature = "mocks")]
        self.leave_mocks(cx);
        self.settings = None;
        if self.project_id() != Some(project) {
            let projects = self.projects.clone();
            let view = cx.new(|cx| ProjectView::new(projects, project, window, cx));
            self.project = Some((project, view));
        }
        if let Some((_, view)) = &self.project {
            window.focus(&view.focus_handle(cx), cx);
        }
        cx.notify();
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.is_none() {
            return;
        }
        self.palette_open = false;
        self.focus_content(window, cx);
        cx.notify();
    }

    /// Focuses what the window shows: Settings, the project, or the landing screen's palette.
    fn focus_content(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match (&self.settings, &self.project) {
            (Some((view, _)), _) => window.focus(&view.focus_handle(cx), cx),
            (None, Some((_, view))) => window.focus(&view.focus_handle(cx), cx),
            (None, None) => self.open_palette(None, window, cx),
        }
    }

    fn on_palette_event(
        &mut self,
        _: &Entity<PaletteView>,
        event: &PaletteEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match *event {
            PaletteEvent::Open {
                project,
                new_window: true,
            } => {
                open_beside(window, self.projects.clone(), Some(project), false, cx);
                self.close_palette(window, cx);
            }
            PaletteEvent::Open { project, .. } => {
                self.show_project(project, window, cx);
                self.close_palette(window, cx);
            }
            PaletteEvent::OpenSettings => self.open_settings(window, cx),
            PaletteEvent::Dismiss => self.close_palette(window, cx),
        }
    }

    /// Whether this is the window's own view rather than one inside a mock.
    fn is_window_root(&self, window: &Window, cx: &Context<Self>) -> bool {
        window.root::<Self>().flatten() == Some(cx.entity())
    }

    /// Ctrl Shift M: the browser from anywhere, back to it from a mock, or out of it.
    #[cfg(feature = "mocks")]
    pub(crate) fn view_mocks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &mut self.mocks {
            None => {
                let browser = cx.new(MockBrowser::new);
                let subscription = cx.subscribe_in(&browser, window, Self::on_mock_browser_event);
                self.mocks = Some(MockMode::new(browser, subscription, cx));
            }
            Some(mode) if mode.shown.is_some() => mode.restore(cx),
            Some(_) => return self.back_from_mocks(window, cx),
        }
        if let Some(mode) = &self.mocks {
            window.focus(&mode.browser.focus_handle(cx), cx);
        }
        cx.notify();
    }

    #[cfg(feature = "mocks")]
    fn on_mock_browser_event(
        &mut self,
        _: &Entity<MockBrowser>,
        event: &MockBrowserEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            MockBrowserEvent::Open { mock, component } => {
                self.open_mock(mock.clone(), *component, window, cx)
            }
            MockBrowserEvent::Back => self.back_from_mocks(window, cx),
        }
    }

    #[cfg(feature = "mocks")]
    pub(crate) fn open_mock(
        &mut self,
        mock: mock::Mock,
        component: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(mode) = &mut self.mocks {
            window.focus(&self.focus_handle, cx);
            mode.show(mock, component, window, cx);
            cx.notify();
        }
    }

    #[cfg(feature = "mocks")]
    pub(crate) fn notices(&self) -> Entity<NoticeOverlay> {
        self.notices.clone()
    }

    #[cfg(feature = "mocks")]
    pub(crate) fn mock_browser(&self) -> Option<Entity<MockBrowser>> {
        Some(self.mocks.as_ref()?.browser.clone())
    }

    /// Out of the mocks, to the project or landing screen as it was.
    #[cfg(feature = "mocks")]
    fn back_from_mocks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_mocks(cx);
        self.focus_content(window, cx);
    }

    #[cfg(feature = "mocks")]
    fn leave_mocks(&mut self, cx: &mut Context<Self>) {
        if let Some(mut mode) = self.mocks.take() {
            mode.restore(cx);
            cx.notify();
        }
    }

    fn title_bar(&self, window: &Window, cx: &mut Context<Self>) -> TitleBar {
        let projects = self.projects.read(cx);
        let project = self.project_id().and_then(|id| {
            let project = projects.project(id)?;
            Some(TitleBarProject {
                initial: project.initial(),
                name: project.name.clone(),
                branch: projects.current_branch(id).cloned(),
            })
        });
        let has_project = project.is_some();
        let open = |keyword: Option<Keyword>| {
            view_callback(cx, move |this: &mut Self, window, cx| {
                match keyword.clone() {
                    Some(keyword) => this.open_palette_scoped(keyword, window, cx),
                    None => this.open_palette(None, window, cx),
                }
            })
        };
        let (errors, warnings, list_open) = self.notices.read(cx).badge(cx);
        let settings_open = self.settings.is_some();
        let title_bar = TitleBar::new(project)
            .controls(draws_window_controls(window))
            .notices(errors, warnings, list_open)
            .on_notices(view_callback(cx, |this: &mut Self, _, cx| {
                this.notices.update(cx, NoticeOverlay::toggle_list)
            }))
            .settings_open(settings_open)
            .on_settings(view_callback_with(
                cx,
                |this: &mut Self, new_window: bool, window, cx| {
                    if new_window {
                        open_beside(window, this.projects.clone(), this.project_id(), true, cx);
                    } else if this.settings.is_some() {
                        this.close_settings(window, cx);
                    } else {
                        this.open_settings(window, cx);
                    }
                },
            ));
        if settings_open {
            let back = view_callback(cx, |this: &mut Self, window, cx| {
                this.close_settings(window, cx)
            });
            return if has_project {
                title_bar.on_project(back).on_palette(open(None))
            } else {
                title_bar
            };
        }
        if has_project {
            title_bar
                .on_project(open(Some(Keyword::Project(None))))
                .on_branch(open(Some(Keyword::Branch)))
                .on_palette(open(None))
        } else {
            title_bar
        }
    }

    fn landing(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let density = cx.density();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .pt_12()
            .pb_8()
            .child(
                svg()
                    .path("logo.svg")
                    .size(dp(64.))
                    .flex_none()
                    .mb_2()
                    .text_color(theme.colors.text_strong),
            )
            .child(self.palette.clone())
            .child(
                div()
                    .flex_none()
                    .typeset(density.small)
                    .text_color(theme.colors.text_faint)
                    .child("Ctrl K brings this palette back from anywhere in Pensive"),
            )
    }
}

/// The share of a display's usable area that a windowed window takes.
const WINDOWED_SHARE: f32 = 0.75;
/// The windowed size when GPUI reports no display.
#[expect(
    clippy::disallowed_methods,
    reason = "window sizes are in window pixels"
)]
const FALLBACK_SIZE: Size<Pixels> = size(px(1280.), px(800.));
/// The smallest a window can be resized to.
#[expect(
    clippy::disallowed_methods,
    reason = "window sizes are in window pixels"
)]
const MIN_SIZE: Size<Pixels> = size(px(720.), px(480.));

/// Opens the app's first window on the landing screen.
pub fn open_window(projects: Entity<Projects>, cx: &mut App) {
    open_with(projects, None, false, first_window_bounds(cx), cx);
}

/// Opens a window on `project`, with Settings open in it if `settings`. It opens windowed on
/// `window`'s display, so it doesn't hide the window it was opened from.
fn open_beside(
    window: &Window,
    projects: Entity<Projects>,
    project: Option<ProjectId>,
    settings: bool,
    cx: &mut App,
) {
    let bounds = WindowBounds::Windowed(windowed_bounds(window.display(cx).map(|d| d.id()), cx));
    open_with(projects, project, settings, bounds, cx);
}

/// The first window fills the screen, and un-maximizing it gives the windowed size.
pub(crate) fn first_window_bounds(cx: &App) -> WindowBounds {
    WindowBounds::Maximized(windowed_bounds(None, cx))
}

/// A window centered on `display`, sized as a share of it. With no display given it uses the
/// primary one, or the first GPUI knows, since Wayland reports no primary display.
fn windowed_bounds(display: Option<DisplayId>, cx: &App) -> Bounds<Pixels> {
    let display = display
        .and_then(|id| cx.find_display(id))
        .or_else(|| cx.primary_display())
        .or_else(|| cx.displays().into_iter().next());
    let size = display
        .as_ref()
        .map_or(FALLBACK_SIZE, |display| {
            display
                .visible_bounds()
                .size
                .map(|side| side * WINDOWED_SHARE)
        })
        .max(&MIN_SIZE);
    Bounds::centered(display.map(|display| display.id()), size, cx)
}

/// A Pensive window that opens at `bounds`.
pub(crate) fn window_options(bounds: WindowBounds) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(bounds),
        window_min_size: Some(MIN_SIZE),
        titlebar: Some(TitlebarOptions {
            title: Some("Pensive".into()),
            appears_transparent: true,
            traffic_light_position: Some(TitleBar::TRAFFIC_LIGHTS),
        }),
        window_decorations: Some(WindowDecorations::Client),
        app_id: Some("pensive".into()),
        ..Default::default()
    }
}

/// Opens a window at `bounds` on `project` or the landing screen, with Settings open in it if
/// `settings`.
fn open_with(
    projects: Entity<Projects>,
    project: Option<ProjectId>,
    settings: bool,
    bounds: WindowBounds,
    cx: &mut App,
) {
    cx.open_window(window_options(bounds), |window, cx| {
        cx.new(|cx| {
            let mut pensive = Pensive::new(projects, project, window, cx);
            if settings {
                pensive.open_settings(window, cx);
            }
            pensive
        })
    })
    .expect("failed to open a window");
}

impl Render for Pensive {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(theme::rem_size(cx));
        #[cfg(feature = "mocks")]
        let mocks = self.mocks.as_ref().map(|mode| mode.render(window, cx));
        #[cfg(not(feature = "mocks"))]
        let mocks: Option<gpui::AnyElement> = None;
        let screen = match mocks {
            Some(mocks) => mocks,
            None => {
                let title_bar = self.title_bar(window, cx);
                let content = match (&self.settings, &self.project) {
                    (Some((view, _)), _) => view.clone().into_any_element(),
                    (None, Some((_, view))) => view.clone().into_any_element(),
                    (None, None) => self.landing(cx).into_any_element(),
                };
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .child(title_bar)
                    .child(div().flex().flex_col().flex_1().min_h_0().child(content))
                    .into_any_element()
            }
        };
        let theme = cx.theme();
        let root = div()
            .id("pensive")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus_handle)
            .bg(theme.colors.surface_app)
            .text_color(theme.colors.text)
            .font_family(theme.fonts.ui.clone())
            .on_action(cx.listener(|pensive, _: &OpenPalette, window, cx| {
                pensive.open_palette(None, window, cx)
            }))
            .on_action(cx.listener(|pensive, _: &OpenActive, window, cx| {
                pensive.open_palette(Some(Tab::Active), window, cx)
            }))
            .on_action(cx.listener(|pensive, _: &ClosePalette, window, cx| {
                pensive.close_palette(window, cx)
            }))
            .on_action(cx.listener(|pensive, _: &OpenSettings, window, cx| {
                pensive.open_settings(window, cx)
            }))
            .on_action(|_: &ZoomIn, _, cx| theme::step_zoom(1, cx))
            .on_action(|_: &ZoomOut, _, cx| theme::step_zoom(-1, cx))
            .on_action(|_: &ResetZoom, _, cx| theme::reset_zoom(cx))
            .child(screen)
            .child(self.notices.clone())
            .when(self.project.is_some() && self.palette_open, |root| {
                root.key_context(PALETTE_OVERLAY).child(
                    div()
                        .id("scrim")
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|pensive, _, window, cx| pensive.close_palette(window, cx)),
                        )
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .pt_16()
                        .pb_16()
                        .bg(scrim(cx))
                        .child(self.palette.clone()),
                )
            });
        #[cfg(feature = "mocks")]
        let root = root.on_action(cx.listener(|pensive, _: &ViewMocks, window, cx| {
            if pensive.is_window_root(window, cx) {
                pensive.view_mocks(window, cx)
            } else {
                cx.propagate()
            }
        }));
        // A window shown inside a mock leaves the frame to the real one.
        if self.is_window_root(window, cx) {
            window_frame(root.into_any_element(), window, cx)
        } else {
            root.into_any_element()
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "tests click and scroll at window pixels"
)]
mod tests {
    use gpui::{TestAppContext, VisualTestContext, point};
    use model::fixtures::HARBOR;

    use super::*;
    use crate::test_support::{click, pensive_window};

    fn palette_open(view: &Entity<Pensive>, cx: &mut VisualTestContext) -> bool {
        view.read_with(cx, |pensive, _| pensive.palette_open)
    }

    fn input_focused(view: &Entity<Pensive>, cx: &mut VisualTestContext) -> bool {
        cx.update(|window, cx| view.read(cx).palette.focus_handle(cx).is_focused(window))
    }

    #[gpui::test]
    fn a_crumb_opens_the_palette_scoped_with_its_input_focused(cx: &mut TestAppContext) {
        let (view, cx) = pensive_window(cx);
        for (crumb, scope) in [
            ("project", Keyword::Project(None)),
            ("branch", Keyword::Branch),
        ] {
            let bounds = cx.debug_bounds(crumb).unwrap();
            click(cx, bounds.center());
            assert!(palette_open(&view, cx));
            let keyword = view.read_with(cx, |pensive, cx| {
                pensive.palette.read(cx).keyword().cloned()
            });
            assert_eq!(keyword, Some(scope));
            assert!(input_focused(&view, cx));
            cx.simulate_keystrokes("escape");
            assert!(!palette_open(&view, cx));
        }
    }

    #[gpui::test]
    fn typing_a_project_and_pressing_enter_opens_it(cx: &mut TestAppContext) {
        let (view, cx) = pensive_window(cx);
        cx.dispatch_action(OpenPalette);
        cx.simulate_input("harbor");
        cx.simulate_keystrokes("enter");
        assert!(!palette_open(&view, cx));
        let project = view.read_with(cx, |pensive, _| pensive.project_id());
        assert_eq!(project, Some(HARBOR));
    }

    #[gpui::test]
    fn a_click_outside_the_palette_closes_it(cx: &mut TestAppContext) {
        let (view, cx) = pensive_window(cx);
        cx.dispatch_action(OpenPalette);
        let center = cx.update(|window, _| window.viewport_size().width / 2.);

        click(cx, point(center, px(90.)));
        assert!(palette_open(&view, cx));
        assert!(input_focused(&view, cx));

        click(cx, point(px(20.), px(300.)));
        assert!(!palette_open(&view, cx));
    }

    #[gpui::test]
    fn escape_closes_the_palette_wherever_focus_is(cx: &mut TestAppContext) {
        let (view, cx) = pensive_window(cx);
        cx.dispatch_action(OpenPalette);
        view.update_in(cx, |pensive, window, cx| {
            window.focus(&pensive.focus_handle, cx)
        });
        assert!(!input_focused(&view, cx));

        cx.simulate_keystrokes("escape");
        assert!(!palette_open(&view, cx));
    }

    #[gpui::test]
    fn ctrl_equals_zooms_in_and_ctrl_zero_resets(cx: &mut TestAppContext) {
        let (_, cx) = pensive_window(cx);
        let rem = |cx: &mut VisualTestContext| cx.update(|window, _| window.rem_size());
        let at_100 = rem(cx);
        cx.simulate_keystrokes("ctrl-= ctrl-=");
        cx.run_until_parked();
        assert_eq!(rem(cx), at_100 * 1.25);
        cx.simulate_keystrokes("ctrl--");
        cx.run_until_parked();
        assert_eq!(rem(cx), at_100 * 1.1);
        cx.simulate_keystrokes("ctrl-0");
        cx.run_until_parked();
        assert_eq!(rem(cx), at_100);
    }

    #[gpui::test]
    fn the_gear_opens_settings_and_escape_goes_back(cx: &mut TestAppContext) {
        let (view, cx) = pensive_window(cx);
        let settings_open = |cx: &mut VisualTestContext| {
            view.read_with(cx, |pensive, _| pensive.settings.is_some())
        };
        let gear = cx.debug_bounds("settings").unwrap();
        click(cx, gear.center());
        assert!(settings_open(cx));

        cx.simulate_keystrokes("escape");
        assert!(!settings_open(cx));
        let project_focused = cx.update(|window, cx| {
            let (_, project) = view.read(cx).project.as_ref().unwrap();
            project.focus_handle(cx).is_focused(window)
        });
        assert!(project_focused);

        cx.dispatch_action(OpenSettings);
        assert!(settings_open(cx));

        // The palette over Settings gives focus back to Settings, not the hidden project.
        cx.dispatch_action(OpenPalette);
        cx.simulate_keystrokes("escape");
        assert!(settings_open(cx));
        let settings_focused = cx.update(|window, cx| {
            let (settings, _) = view.read(cx).settings.as_ref().unwrap();
            settings.focus_handle(cx).contains_focused(window, cx)
        });
        assert!(settings_focused);
    }

    #[cfg(feature = "mocks")]
    #[gpui::test]
    fn leaving_a_mock_puts_back_the_settings_it_changed(cx: &mut TestAppContext) {
        use settings::Settings;

        use crate::mock_browser::ViewMocks;

        let (view, cx) = pensive_window(cx);
        let source =
            |cx: &mut VisualTestContext| cx.update(|_, cx| cx.global::<Settings>().source().into());
        let before: String = source(cx);
        cx.dispatch_action(ViewMocks);
        for mock in crate::mocks() {
            let id = mock.id();
            view.update_in(cx, |pensive, window, cx| {
                pensive.open_mock(mock, false, window, cx)
            });
            cx.run_until_parked();
            cx.dispatch_action(ViewMocks);
            assert_eq!(source(cx), before, "{id} left its settings behind");
        }
    }

    #[cfg(feature = "mocks")]
    #[gpui::test]
    fn the_gear_over_a_component_mock_leaves_the_mocks_for_settings(cx: &mut TestAppContext) {
        use crate::mock_browser::ViewMocks;

        let (view, cx) = pensive_window(cx);
        cx.dispatch_action(ViewMocks);
        let tooltips = components::mocks()
            .into_iter()
            .find(|mock| mock.id() == "tooltip/gallery")
            .unwrap();
        view.update_in(cx, |pensive, window, cx| {
            pensive.open_mock(tooltips, true, window, cx)
        });
        cx.run_until_parked();

        let gear = cx.debug_bounds("settings").unwrap();
        click(cx, gear.center());
        view.read_with(cx, |pensive, _| {
            assert!(pensive.mocks.is_none());
            assert!(pensive.settings.is_some());
        });
    }

    #[cfg(feature = "mocks")]
    #[gpui::test]
    fn a_demo_starts_on_the_mock_it_names(cx: &mut TestAppContext) {
        cx.update(|cx| {
            theme::init(cx);
            crate::init(cx);
        });
        let projects = cx.new(|_| model::fixtures::projects());
        // GPUI's test windows can't maximize, so the demo opens windowed.
        let size = Some(size(px(960.), px(640.)));
        let opened = cx.update(|cx| {
            crate::open_demo_window(projects.clone(), Some("project/unsaved"), size, cx)
        });
        assert_eq!(opened, Ok(()));
        cx.run_until_parked();
        let window = cx.update(|cx| cx.windows()[0]);
        let shown = window
            .update(cx, |root, _, cx| {
                let pensive = root.downcast::<Pensive>().unwrap();
                pensive.read(cx).mocks.as_ref().and_then(MockMode::shown_id)
            })
            .unwrap();
        assert_eq!(shown.as_deref(), Some("project/unsaved"));

        let missing =
            cx.update(|cx| crate::open_demo_window(projects, Some("project/nope"), size, cx));
        assert!(missing.unwrap_err().contains("`project/nope`"));
    }

    #[cfg(feature = "mocks")]
    #[gpui::test]
    fn the_mock_browser_opens_a_mock_and_puts_the_app_back(cx: &mut TestAppContext) {
        use crate::mock_browser::ViewMocks;

        let (view, cx) = pensive_window(cx);
        let mocks = |cx: &mut VisualTestContext| {
            view.read_with(cx, |pensive, _| {
                pensive.mocks.as_ref().map(|mode| mode.shown.is_some())
            })
        };
        let row_height = |cx: &mut VisualTestContext| cx.update(|_, cx| cx.density().row_height);
        let calm = row_height(cx);

        cx.dispatch_action(ViewMocks);
        assert_eq!(mocks(cx), Some(false));

        let browser = view.read_with(cx, |pensive, _| pensive.mock_browser().unwrap());
        browser.update(cx, |browser, cx| browser.select_subject("project", cx));
        // The last project mock is the Dense one, which changes the density for the whole app.
        // Right stops at the last mock, however many there are.
        cx.simulate_keystrokes(&["right"; 20].join(" "));
        cx.simulate_keystrokes("enter");
        assert_eq!(mocks(cx), Some(true));
        assert_ne!(row_height(cx), calm);

        cx.dispatch_action(ViewMocks);
        assert_eq!(mocks(cx), Some(false));
        assert_eq!(row_height(cx), calm);

        cx.simulate_keystrokes("escape");
        assert_eq!(mocks(cx), None);
        let project_focused = cx.update(|window, cx| {
            let (_, project) = view.read(cx).project.as_ref().unwrap();
            project.focus_handle(cx).is_focused(window)
        });
        assert!(project_focused);
    }
}

//! The view that fills each window: the landing screen with no project open, otherwise the
//! project, with the command palette over it when Ctrl K opens it.

use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, Subscription, Window, WindowBounds,
    WindowOptions, actions, div, hsla, prelude::*, px, size, svg,
};
use model::{ProjectId, Projects};
use theme::{ActiveTheme, Typeset};

use crate::{
    key_binding,
    palette::{PaletteEvent, PaletteView, Tab},
};

actions!(pensive, [OpenPalette, OpenActive]);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        key_binding("secondary-k", OpenPalette, None, cx),
        key_binding("secondary-shift-k", OpenActive, None, cx),
    ]);
}

pub struct Pensive {
    projects: Entity<Projects>,
    project: Option<ProjectId>,
    palette: Entity<PaletteView>,
    palette_open: bool,
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
        let subscription = cx.subscribe_in(&palette, window, Self::on_palette_event);
        let mut pensive = Self {
            projects,
            project,
            palette,
            palette_open: false,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
        };
        match project {
            None => pensive.open_palette(Tab::All, window, cx),
            Some(_) => window.focus(&pensive.focus_handle, cx),
        }
        pensive
    }

    /// Opens the palette on `tab`. On the landing screen it's already open, so this starts over.
    pub fn open_palette(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        let (project, overlay) = (self.project, self.project.is_some());
        self.palette
            .update(cx, |palette, cx| palette.reset(project, tab, overlay, cx));
        self.palette_open = true;
        window.focus(&self.palette.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.is_some() {
            self.palette_open = false;
            window.focus(&self.focus_handle, cx);
            cx.notify();
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
                open_window(self.projects.clone(), Some(project), cx);
                self.close_palette(window, cx);
            }
            PaletteEvent::Open { project, .. } => {
                self.project = Some(project);
                self.close_palette(window, cx);
            }
            PaletteEvent::Dismiss => self.close_palette(window, cx),
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
                    .size(px(64.))
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

    /// Stands in for the project window until the app shell is designed.
    fn project_window(&self, project: ProjectId, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let density = cx.density();
        let projects = self.projects.read(cx);
        let name = projects
            .project(project)
            .map(|project| project.name.clone())
            .unwrap_or_default();
        let branch = projects
            .current_branch(project)
            .cloned()
            .unwrap_or_default();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .child(
                div()
                    .typeset(density.heading)
                    .text_color(theme.colors.text_strong)
                    .child(name),
            )
            .child(
                div()
                    .font_family(theme.fonts.code.clone())
                    .typeset(density.small)
                    .text_color(theme.colors.text_muted)
                    .child(branch),
            )
    }
}

/// Opens a window on `project`, or on the landing screen.
pub fn open_window(projects: Entity<Projects>, project: Option<ProjectId>, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(960.), px(640.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Pensive".into()),
                ..Default::default()
            }),
            app_id: Some("pensive".into()),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| Pensive::new(projects, project, window, cx)),
    )
    .expect("failed to open a window");
}

impl Render for Pensive {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let root = div()
            .id("pensive")
            .relative()
            .size_full()
            .track_focus(&self.focus_handle)
            .bg(theme.colors.surface_app)
            .text_color(theme.colors.text)
            .font_family(theme.fonts.ui.clone())
            .on_action(cx.listener(|pensive, _: &OpenPalette, window, cx| {
                pensive.open_palette(Tab::All, window, cx)
            }))
            .on_action(cx.listener(|pensive, _: &OpenActive, window, cx| {
                pensive.open_palette(Tab::Active, window, cx)
            }));
        match self.project {
            None => root.child(self.landing(cx)),
            Some(project) => {
                root.child(self.project_window(project, cx))
                    .when(self.palette_open, |root| {
                        root.child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .flex_col()
                                .items_center()
                                .pt_16()
                                .pb_16()
                                .bg(hsla(0., 0., 0., 0.5))
                                .child(self.palette.clone()),
                        )
                    })
            }
        }
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use std::sync::Arc;

    use mock::Mock;
    use model::fixtures::{self, FakeGit, HARBOR, PENSIVE};

    use crate::palette::Keyword;

    /// A window on the sample projects, with the palette open on `tab` and set up by `setup`.
    fn window(
        project: Option<ProjectId>,
        tab: Option<Tab>,
        setup: fn(&mut PaletteView, &mut Context<PaletteView>),
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::AnyView {
        let projects = cx.new(|_| fixtures::projects());
        let view = cx.new(|cx| Pensive::new(projects, project, window, cx));
        view.update(cx, |pensive, cx| {
            if let Some(tab) = tab {
                pensive.open_palette(tab, window, cx);
            }
            pensive.palette.update(cx, setup);
        });
        view.into()
    }
    fn untouched(_: &mut PaletteView, _: &mut Context<PaletteView>) {}

    vec![
        Mock::new("root", "empty", |window, cx| {
            let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
            cx.new(|cx| Pensive::new(projects, None, window, cx)).into()
        }),
        Mock::new("root", "landing", |w, cx| {
            window(None, None, untouched, w, cx)
        }),
        Mock::new("root", "landing_typing", |w, cx| {
            window(
                None,
                None,
                |palette, cx| palette.set_query("gui", cx),
                w,
                cx,
            )
        }),
        Mock::new("root", "project", |w, cx| {
            window(Some(PENSIVE), None, untouched, w, cx)
        }),
        Mock::new("root", "palette", |w, cx| {
            window(Some(PENSIVE), Some(Tab::All), untouched, w, cx)
        }),
        Mock::new("root", "active", |w, cx| {
            window(Some(PENSIVE), Some(Tab::Active), untouched, w, cx)
        }),
        Mock::new("root", "commands", |w, cx| {
            window(Some(PENSIVE), Some(Tab::Commands), untouched, w, cx)
        }),
        Mock::new("root", "scoped", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| {
                    palette.set_keyword(Some(Keyword::Project(Some(HARBOR))), cx);
                    palette.set_query("fix", cx);
                },
                w,
                cx,
            )
        }),
        Mock::new("root", "no_results", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| {
                    palette.set_keyword(Some(Keyword::Project(Some(HARBOR))), cx);
                    palette.set_query("qwerty", cx);
                },
                w,
                cx,
            )
        }),
        Mock::new("root", "clone", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| palette.set_query("https://github.com/zed-industries/zed.git", cx),
                w,
                cx,
            )
        }),
        Mock::new("root", "find_settings", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| palette.set_query("sett", cx),
                w,
                cx,
            )
        }),
    ]
}

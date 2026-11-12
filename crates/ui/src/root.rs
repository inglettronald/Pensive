//! The view that fills each window: the title bar over the landing screen with no project open,
//! otherwise over the project, with the command palette over both when Ctrl K opens it.

use components::{TitleBar, TitleBarProject};
use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, Subscription, TitlebarOptions, Window,
    WindowBounds, WindowDecorations, WindowOptions, actions, div, hsla, prelude::*, px, size, svg,
};
use model::{Changes, ProjectId, Projects};
use theme::{ActiveTheme, Typeset};

use crate::{
    key_binding,
    palette::{Keyword, PaletteEvent, PaletteView, Tab},
    project::ProjectView,
    view_callback,
    window_frame::{draws_window_controls, window_frame},
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
    changes: Entity<Changes>,
    project: Option<ProjectId>,
    project_view: Option<Entity<ProjectView>>,
    palette: Entity<PaletteView>,
    palette_open: bool,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

impl Pensive {
    /// A window on `project`, or the landing screen when it's `None`. Until a Git backend reads
    /// each project's own changes, every project shows `changes`.
    pub fn new(
        projects: Entity<Projects>,
        changes: Entity<Changes>,
        project: Option<ProjectId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let palette = cx.new(|cx| PaletteView::new(projects.clone(), cx));
        let subscription = cx.subscribe_in(&palette, window, Self::on_palette_event);
        let mut pensive = Self {
            projects,
            changes,
            project: None,
            project_view: None,
            palette,
            palette_open: false,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
        };
        match project {
            None => pensive.open_palette(Tab::All, window, cx),
            Some(project) => pensive.show_project(project, window, cx),
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

    /// Opens the palette narrowed to `keyword`, as the title bar's crumbs do.
    fn open_palette_scoped(
        &mut self,
        keyword: Keyword,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_palette(Tab::All, window, cx);
        self.palette
            .update(cx, |palette, cx| palette.set_keyword(Some(keyword), cx));
    }

    fn show_project(&mut self, project: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        self.project = Some(project);
        let changes = self.changes.clone();
        let view = self
            .project_view
            .get_or_insert_with(|| cx.new(|cx| ProjectView::new(changes, cx)))
            .clone();
        window.focus(&view.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.is_none() {
            return;
        }
        self.palette_open = false;
        match &self.project_view {
            Some(view) => window.focus(&view.focus_handle(cx), cx),
            None => window.focus(&self.focus_handle, cx),
        }
        cx.notify();
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
                open_window(
                    self.projects.clone(),
                    self.changes.clone(),
                    Some(project),
                    cx,
                );
                self.close_palette(window, cx);
            }
            PaletteEvent::Open { project, .. } => {
                self.show_project(project, window, cx);
                self.close_palette(window, cx);
            }
            PaletteEvent::Dismiss => self.close_palette(window, cx),
        }
    }

    fn title_bar(&self, window: &Window, cx: &mut Context<Self>) -> TitleBar {
        let projects = self.projects.read(cx);
        let project = self.project.and_then(|id| {
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
                    None => this.open_palette(Tab::All, window, cx),
                }
            })
        };
        let title_bar = TitleBar::new(project).controls(draws_window_controls(window));
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
}

/// Opens a window on `project`, or on the landing screen.
pub fn open_window(
    projects: Entity<Projects>,
    changes: Entity<Changes>,
    project: Option<ProjectId>,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(960.), px(640.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Pensive".into()),
                appears_transparent: true,
                traffic_light_position: Some(TitleBar::TRAFFIC_LIGHTS),
            }),
            window_decorations: Some(WindowDecorations::Client),
            app_id: Some("pensive".into()),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| Pensive::new(projects, changes, project, window, cx)),
    )
    .expect("failed to open a window");
}

impl Render for Pensive {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_bar = self.title_bar(window, cx);
        let content = match &self.project_view {
            Some(view) if self.project.is_some() => view.clone().into_any_element(),
            _ => self.landing(cx).into_any_element(),
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
                pensive.open_palette(Tab::All, window, cx)
            }))
            .on_action(cx.listener(|pensive, _: &OpenActive, window, cx| {
                pensive.open_palette(Tab::Active, window, cx)
            }))
            .child(title_bar)
            .child(div().flex().flex_col().flex_1().min_h_0().child(content))
            .when(self.project.is_some() && self.palette_open, |root| {
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
            });
        window_frame(root.into_any_element(), window, cx)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use mock::Mock;
    use model::fixtures::{self, HARBOR, PENSIVE};

    /// A window on the sample projects, with the palette open on `tab` and set up by `setup`.
    fn window(
        project: Option<ProjectId>,
        tab: Option<Tab>,
        setup: fn(&mut PaletteView, &mut Context<PaletteView>),
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::AnyView {
        let view = pensive(project, window, cx);
        view.update(cx, |pensive, cx| {
            if let Some(tab) = tab {
                pensive.open_palette(tab, window, cx);
            }
            pensive.palette.update(cx, setup);
        });
        view.into()
    }
    fn untouched(_: &mut PaletteView, _: &mut Context<PaletteView>) {}

    fn pensive(project: Option<ProjectId>, window: &mut Window, cx: &mut App) -> Entity<Pensive> {
        let projects = cx.new(|_| fixtures::projects());
        let changes = cx.new(|_| fixtures::changes());
        cx.new(|cx| Pensive::new(projects, changes, project, window, cx))
    }

    vec![
        Mock::new("root", "empty", |window, cx| {
            let projects = cx.new(|_| Projects::default());
            let changes = cx.new(|_| Changes::default());
            cx.new(|cx| Pensive::new(projects, changes, None, window, cx))
                .into()
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
            pensive(Some(PENSIVE), w, cx).into()
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

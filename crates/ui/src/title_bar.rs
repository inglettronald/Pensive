//! The title bar Pensive draws: project › branch, the palette trigger, the gear and the window
//! controls, with the empty space dragging the window.

use std::rc::Rc;

use components::{Breadcrumb, Callback, Icon, IconButton, IconName, Kbd};
use gpui::{
    App, Decorations, MouseButton, SharedString, Window, WindowControlArea, div, prelude::*, px,
};
use theme::{ActiveTheme, Typeset};

/// Where the macOS window buttons sit, inside the title bar's left end.
pub(crate) const TRAFFIC_LIGHTS: gpui::Point<gpui::Pixels> = gpui::point(px(14.), px(13.));
/// Room left of the crumbs for the macOS window buttons.
const TRAFFIC_LIGHTS_WIDTH: f32 = 76.;

/// The project shown in the title bar.
pub(crate) struct Location {
    pub initial: SharedString,
    pub project: SharedString,
    pub branch: Option<SharedString>,
}

#[derive(IntoElement)]
pub(crate) struct TitleBar {
    location: Option<Location>,
    controls: bool,
    on_project: Option<Callback<()>>,
    on_branch: Option<Callback<()>>,
    on_palette: Option<Callback<()>>,
}

impl TitleBar {
    /// With no location, the bar is empty apart from the window controls, as on the landing
    /// screen, where the palette is the page.
    pub fn new(location: Option<Location>) -> Self {
        Self {
            location,
            controls: false,
            on_project: None,
            on_branch: None,
            on_palette: None,
        }
    }

    /// Whether Pensive draws minimize, maximize and close. See [`draws_window_controls`].
    pub fn controls(mut self, controls: bool) -> Self {
        self.controls = controls;
        self
    }

    pub fn on_project(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_project = Some(Rc::new(move |(), window, cx| handler(window, cx)));
        self
    }

    pub fn on_branch(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_branch = Some(Rc::new(move |(), window, cx| handler(window, cx)));
        self
    }

    pub fn on_palette(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_palette = Some(Rc::new(move |(), window, cx| handler(window, cx)));
        self
    }
}

/// macOS keeps its own window buttons, and so does a Linux compositor that decorates the window
/// itself. Everywhere else Pensive draws them.
pub(crate) fn draws_window_controls(window: &Window) -> bool {
    if cfg!(target_os = "macos") {
        false
    } else if cfg!(target_os = "windows") {
        true
    } else {
        matches!(window.window_decorations(), Decorations::Client { .. })
    }
}

impl RenderOnce for TitleBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let crumb = |id: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .h(px(28.))
                .px_1p5()
                .rounded(density.radius_md)
                .hover(|crumb| crumb.bg(colors.surface_raised))
        };

        let location = self.location.map(|location| {
            let project = crumb("project")
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .rounded(density.radius_sm)
                        .bg(colors.surface_selected)
                        .text_color(colors.text_strong)
                        .typeset(density.label)
                        .child(location.initial),
                )
                .child(
                    div()
                        .typeset(density.body)
                        .text_color(colors.text_strong)
                        .child(location.project),
                )
                .when_some(self.on_project, |crumb, handler| {
                    crumb.on_click(move |_, window, cx| handler((), window, cx))
                });
            let branch = location.branch.map(|branch| {
                crumb("branch")
                    .gap_1p5()
                    .child(
                        Icon::new(IconName::GitBranch)
                            .size(px(14.))
                            .color(colors.text_muted),
                    )
                    .child(
                        div()
                            .font_family(theme.fonts.code.clone())
                            .typeset(density.small)
                            .text_color(colors.text)
                            .child(branch),
                    )
                    .when_some(self.on_branch, |crumb, handler| {
                        crumb.on_click(move |_, window, cx| handler((), window, cx))
                    })
            });
            Breadcrumb::new(
                std::iter::once(project.into_any_element())
                    .chain(branch.map(IntoElement::into_any_element)),
            )
            .chevron_size(px(14.))
        });

        let palette = self.on_palette.map(|handler| {
            div()
                .id("palette")
                .flex()
                .flex_none()
                .items_center()
                .gap_2()
                .w(px(280.))
                .h(px(28.))
                .pl_2()
                .pr_1()
                .rounded(density.radius_md)
                .border_1()
                .border_color(colors.border_subtle)
                .bg(colors.surface_raised)
                .text_color(colors.text_faint)
                .hover(|palette| palette.border_color(colors.border))
                .child(
                    Icon::new(IconName::Search)
                        .size(px(14.))
                        .color(colors.text_faint),
                )
                .child(
                    div()
                        .flex_1()
                        .typeset(density.small)
                        .child("Search or run a command"),
                )
                .child(Kbd::new("Ctrl K"))
                .on_click(move |_, window, cx| handler((), window, cx))
        });

        let controls = self.controls.then(|| {
            div()
                .flex()
                .items_center()
                .gap_0p5()
                .child(div().w(px(1.)).h(px(16.)).mx_1p5().bg(colors.border_subtle))
                .child(window_control(
                    "minimize",
                    IconName::Minus,
                    WindowControlArea::Min,
                ))
                .child(window_control(
                    "maximize",
                    IconName::Square,
                    WindowControlArea::Max,
                ))
                .child(window_control(
                    "close",
                    IconName::X,
                    WindowControlArea::Close,
                ))
        });

        div()
            .flex()
            .flex_none()
            .items_center()
            .h(density.row_height + px(12.))
            .pl_2()
            .pr_1()
            .when(cfg!(target_os = "macos"), |bar| {
                bar.pl(px(TRAFFIC_LIGHTS_WIDTH))
            })
            .border_b_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_panel)
            .font_family(theme.fonts.ui.clone())
            .child(
                drag_area("drag-left")
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .children(location),
            )
            .children(palette)
            .child(
                drag_area("drag-right")
                    .flex()
                    .items_center()
                    .justify_end()
                    .flex_1()
                    .min_w_0()
                    // Settings opens from here once its window exists (PEN-11).
                    .child(IconButton::new("settings", IconName::Settings).size(px(28.)))
                    .children(controls),
            )
    }
}

/// Space that moves the window when dragged and maximizes it when double-clicked.
fn drag_area(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_full()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |event, window, _| {
            if event.click_count == 1 && !cfg!(target_os = "windows") {
                window.start_window_move();
            }
        })
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                if cfg!(target_os = "macos") {
                    window.titlebar_double_click();
                } else {
                    window.zoom_window();
                }
            }
        })
}

/// Windows handles these itself through the control area, for snap layouts. Linux needs the
/// click.
fn window_control(id: &'static str, icon: IconName, area: WindowControlArea) -> impl IntoElement {
    div().window_control_area(area).child(
        IconButton::new(id, icon)
            .size(px(28.))
            .color(|colors| colors.text_faint)
            .on_click(move |window, _| {
                if cfg!(target_os = "windows") {
                    return;
                }
                match area {
                    WindowControlArea::Min => window.minimize_window(),
                    WindowControlArea::Max => window.zoom_window(),
                    WindowControlArea::Close => window.remove_window(),
                    WindowControlArea::Drag => {}
                }
            }),
    )
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::size;

    vec![
        mock::Mock::component("title_bar", "gallery", || {
            let pensive = || Location {
                initial: "P".into(),
                project: "pensive".into(),
                branch: Some("feat/command-palette".into()),
            };
            let bar = |title_bar: TitleBar| div().w(px(960.)).child(title_bar).into_any_element();
            components::Gallery::new("TitleBar")
                .section(
                    "Project open",
                    [(
                        "With window controls, as on Linux and Windows",
                        bar(TitleBar::new(Some(pensive()))
                            .controls(true)
                            .on_palette(|_, _| {})),
                    )],
                )
                .section(
                    "Landing",
                    [(
                        "No project: only the controls",
                        bar(TitleBar::new(None).controls(true)),
                    )],
                )
                .into_any_element()
        })
        .size(size(px(1088.), px(400.))),
    ]
}

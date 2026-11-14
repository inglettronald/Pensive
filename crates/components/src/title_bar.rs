//! The title bar Pensive draws: project › branch, the palette trigger, the notice badge and the window
//! controls, with the empty space dragging the window.

use gpui::{
    AnyElement, App, Div, ElementId, Entity, MouseButton, Pixels, Point, SharedString, Stateful,
    Window, WindowControlArea, div, point, prelude::*, px,
};
use theme::{ActiveTheme, Dp, HAIRLINE, Typeset, dp};

use crate::{Breadcrumb, Callback, Icon, IconButton, IconName, Kbd, Tone, callback};

/// Room left of the crumbs for the macOS window buttons.
const TRAFFIC_LIGHTS_WIDTH: Dp = dp(76.);

/// The project shown in the title bar.
pub struct TitleBarProject {
    /// The letter on its tile.
    pub initial: SharedString,
    pub name: SharedString,
    pub branch: Option<SharedString>,
}

#[derive(IntoElement)]
pub struct TitleBar {
    project: Option<TitleBarProject>,
    location: Option<AnyElement>,
    notices: (usize, usize),
    notices_open: bool,
    on_notices: Option<Callback<()>>,
    controls: bool,
    on_project: Option<Callback<()>>,
    on_branch: Option<Callback<()>>,
    on_palette: Option<Callback<()>>,
    settings_open: bool,
}

impl TitleBar {
    /// Where the macOS window buttons sit, inside the title bar's left end.
    #[expect(
        clippy::disallowed_methods,
        reason = "macOS places its buttons in window pixels"
    )]
    pub const TRAFFIC_LIGHTS: Point<Pixels> = point(px(14.), px(13.));

    /// With no project, the bar is empty apart from the window controls, as on the
    /// landing screen, where the palette is the page.
    pub fn new(project: Option<TitleBarProject>) -> Self {
        Self {
            project,
            location: None,
            notices: (0, 0),
            notices_open: false,
            on_notices: None,
            controls: false,
            on_project: None,
            on_branch: None,
            on_palette: None,
            settings_open: false,
        }
    }

    /// Shown in place of project › branch, on screens that aren't a project.
    pub fn location(mut self, location: impl IntoElement) -> Self {
        self.location = Some(location.into_any_element());
        self
    }

    /// The badge at the right end: an icon and a count for the errors and for the warnings held,
    /// outlined in the worse one. It's gone while both are zero.
    pub fn notices(mut self, errors: usize, warnings: usize, open: bool) -> Self {
        self.notices = (errors, warnings);
        self.notices_open = open;
        self
    }

    pub fn on_notices(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_notices = Some(callback(handler));
        self
    }

    /// Draws minimize, maximize and close, for platforms that leave them to the app.
    pub fn controls(mut self, controls: bool) -> Self {
        self.controls = controls;
        self
    }

    pub fn on_project(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_project = Some(callback(handler));
        self
    }

    pub fn on_branch(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_branch = Some(callback(handler));
        self
    }

    /// Names Settings after the project in place of the branch, or alone with no project.
    pub fn settings_open(mut self, open: bool) -> Self {
        self.settings_open = open;
        self
    }

    /// Shows the palette trigger, which calls this when clicked.
    pub fn on_palette(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_palette = Some(callback(handler));
        self
    }
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let crumb = |id: &'static str| {
            div()
                .id(id)
                .debug_selector(|| id.into())
                .occlude()
                .flex()
                .items_center()
                .gap_2()
                .h(dp(28.))
                .px_1p5()
                .rounded(density.radius_md)
                .hover(|crumb| crumb.bg(colors.surface_raised))
        };

        let mut settings = self.settings_open.then(|| {
            crumb("settings-crumb")
                .child(
                    div()
                        .typeset(density.body)
                        .text_color(colors.text_strong)
                        .child("Settings"),
                )
                .into_any_element()
        });
        let location = self.project.map(|project| {
            let name = crumb("project")
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(dp(20.))
                        .rounded(density.radius_sm)
                        .bg(colors.surface_selected)
                        .text_color(colors.text_strong)
                        .typeset(density.label)
                        .child(project.initial),
                )
                .child(
                    div()
                        .typeset(density.body)
                        .text_color(colors.text_strong)
                        .child(project.name),
                )
                .when_some(self.on_project, |crumb, handler| {
                    crumb.on_click(move |_, window, cx| handler((), window, cx))
                });
            let branch = project
                .branch
                .filter(|_| !self.settings_open)
                .map(|branch| {
                    crumb("branch")
                        .gap_1p5()
                        .child(
                            Icon::new(IconName::GitBranch)
                                .size(dp(14.))
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
                std::iter::once(name.into_any_element())
                    .chain(branch.map(IntoElement::into_any_element))
                    .chain(settings.take()),
            )
            .chevron_size(dp(14.))
            .into_any_element()
        });
        // With no project, Settings stands alone.
        let location = self.location.or(location).or(settings);

        let palette = self.on_palette.map(|handler| {
            div()
                .id("palette")
                .occlude()
                .flex()
                .flex_none()
                .items_center()
                .gap_2()
                .w(dp(280.))
                .h(dp(28.))
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
                        .size(dp(14.))
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

        let (errors, warnings) = self.notices;
        let badge = (errors + warnings > 0).then(|| {
            let worst = if errors > 0 {
                Tone::Error
            } else {
                Tone::Warning
            };
            let part = |tone: Tone, count: usize| {
                (count > 0).then(|| {
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .children(tone.icon(14., colors))
                        .child(count.to_string())
                })
            };
            div()
                .id("notices")
                .occlude()
                .flex()
                .items_center()
                .gap_1p5()
                .h(dp(26.))
                .px_2()
                .mr_1()
                .rounded(density.radius_md)
                .border_1()
                .border_color(worst.outline(colors))
                .when(self.notices_open, |badge| badge.bg(colors.surface_raised))
                .hover(|badge| badge.bg(colors.surface_raised))
                .typeset(density.small)
                .text_color(colors.text_strong)
                .children(part(Tone::Error, errors))
                .children(part(Tone::Warning, warnings))
                .when_some(self.on_notices, |badge, handler| {
                    badge.on_click(move |_, window, cx| handler((), window, cx))
                })
        });

        let controls = self.controls.then(|| {
            div()
                .flex()
                .items_center()
                .gap_0p5()
                .child(
                    div()
                        .w(HAIRLINE)
                        .h(dp(16.))
                        .mx_1p5()
                        .bg(colors.border_subtle),
                )
                .child(window_control(
                    "minimize",
                    IconName::WindowMinimize,
                    WindowControlArea::Min,
                ))
                .child(window_control(
                    "maximize",
                    IconName::WindowMaximize,
                    WindowControlArea::Max,
                ))
                .child(window_control(
                    "close",
                    IconName::WindowClose,
                    WindowControlArea::Close,
                ))
        });

        div()
            .flex()
            .flex_none()
            .items_center()
            .h(density.row_height + dp(12.))
            .pl_2()
            .pr_1()
            .when(cfg!(target_os = "macos"), |bar| {
                bar.pl(TRAFFIC_LIGHTS_WIDTH)
            })
            .border_b_1()
            .border_color(colors.border_subtle)
            .bg(colors.surface_panel)
            .font_family(theme.fonts.ui.clone())
            .child(
                drag_area("drag-left", window, cx)
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .children(location),
            )
            .children(palette)
            .child(
                drag_area("drag-right", window, cx)
                    .flex()
                    .items_center()
                    .justify_end()
                    .flex_1()
                    .min_w_0()
                    .children(badge)
                    .children(controls),
            )
    }
}

/// Space that moves the window when dragged and maximizes it when double-clicked. The move
/// starts on the first mouse move after a press, so a double-click still arrives. Windows moves
/// the window itself, through the control area.
fn drag_area(id: &'static str, window: &mut Window, cx: &mut App) -> Stateful<Div> {
    let pressed: Entity<bool> = window.use_keyed_state(
        ElementId::Name(format!("{id}-pressed").into()),
        cx,
        |_, _| false,
    );
    div()
        .id(id)
        .h_full()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, {
            let pressed = pressed.clone();
            move |_, _, cx| {
                if !cfg!(target_os = "windows") {
                    pressed.write(cx, true);
                }
            }
        })
        .on_mouse_up(MouseButton::Left, {
            let pressed = pressed.clone();
            move |_, _, cx| pressed.write(cx, false)
        })
        .on_mouse_move(move |event, window, cx| {
            if *pressed.read(cx) {
                pressed.write(cx, false);
                if event.pressed_button == Some(MouseButton::Left) {
                    window.start_window_move();
                }
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

/// Windows answers these itself through the control area, for snap layouts. Elsewhere the click
/// does it.
fn window_control(id: &'static str, icon: IconName, area: WindowControlArea) -> impl IntoElement {
    div().occlude().window_control_area(area).child(
        IconButton::new(id, icon)
            .size(dp(28.))
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
    vec![
        mock::Mock::component("title_bar", "gallery", || {
            let bar = |title_bar: TitleBar| div().w(dp(960.)).child(title_bar).into_any_element();
            let pensive = TitleBarProject {
                initial: "P".into(),
                name: "pensive".into(),
                branch: Some("feat/command-palette".into()),
            };
            crate::Gallery::new("TitleBar")
                .section(
                    "Project open",
                    [(
                        "With window controls, as on Linux and Windows",
                        bar(TitleBar::new(Some(pensive))
                            .controls(true)
                            .on_palette(|_, _| {})),
                    )],
                )
                .section(
                    "Settings open",
                    [(
                        "Settings in place of the branch",
                        bar(TitleBar::new(Some(TitleBarProject {
                            initial: "P".into(),
                            name: "pensive".into(),
                            branch: Some("main".into()),
                        }))
                        .controls(true)
                        .settings_open(true)
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
                .section(
                    "Notices held",
                    [
                        (
                            "Errors and warnings",
                            bar(TitleBar::new(None).notices(2, 1, false)),
                        ),
                        (
                            "Warnings only, list open",
                            bar(TitleBar::new(None).notices(0, 3, true)),
                        ),
                    ],
                )
                .into_any_element()
        })
        .size(1088., 640.),
    ]
}

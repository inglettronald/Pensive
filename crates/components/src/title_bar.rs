//! The title bar Pensive draws: project › branch, the palette trigger, the gear and the window
//! controls, with the empty space dragging the window.

use gpui::{
    AnyElement, App, Div, ElementId, Entity, MouseButton, Pixels, Point, SharedString, Stateful,
    Window, WindowControlArea, div, point, prelude::*, px,
};
use theme::{ActiveTheme, Typeset};

use crate::{Breadcrumb, Callback, Icon, IconButton, IconName, Kbd, callback};

/// Room left of the crumbs for the macOS window buttons.
const TRAFFIC_LIGHTS_WIDTH: Pixels = px(76.);

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
    controls: bool,
    on_project: Option<Callback<()>>,
    on_branch: Option<Callback<()>>,
    on_palette: Option<Callback<()>>,
}

impl TitleBar {
    /// Where the macOS window buttons sit, inside the title bar's left end.
    pub const TRAFFIC_LIGHTS: Point<Pixels> = point(px(14.), px(13.));

    /// With no project, the bar is empty apart from the gear and the window controls, as on the
    /// landing screen, where the palette is the page.
    pub fn new(project: Option<TitleBarProject>) -> Self {
        Self {
            project,
            location: None,
            controls: false,
            on_project: None,
            on_branch: None,
            on_palette: None,
        }
    }

    /// Shown in place of project › branch, on screens that aren't a project.
    pub fn location(mut self, location: impl IntoElement) -> Self {
        self.location = Some(location.into_any_element());
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
                .occlude()
                .flex()
                .items_center()
                .gap_2()
                .h(px(28.))
                .px_1p5()
                .rounded(density.radius_md)
                .hover(|crumb| crumb.bg(colors.surface_raised))
        };

        let location = self.project.map(|project| {
            let name = crumb("project")
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
            let branch = project.branch.map(|branch| {
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
                std::iter::once(name.into_any_element())
                    .chain(branch.map(IntoElement::into_any_element)),
            )
            .chevron_size(px(14.))
            .into_any_element()
        });
        let location = self.location.or(location);

        let palette = self.on_palette.map(|handler| {
            div()
                .id("palette")
                .occlude()
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
                    // Settings opens from here once its window exists (PEN-11).
                    .child(
                        div()
                            .occlude()
                            .child(IconButton::new("settings", IconName::Settings).size(px(28.))),
                    )
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
            let bar = |title_bar: TitleBar| div().w(px(960.)).child(title_bar).into_any_element();
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
                    "Landing",
                    [(
                        "No project: only the gear and the controls",
                        bar(TitleBar::new(None).controls(true)),
                    )],
                )
                .into_any_element()
        })
        .size(size(px(1088.), px(400.))),
    ]
}

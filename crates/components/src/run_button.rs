//! The title bar's Run button: a split button that runs the chosen configuration, with a chevron
//! that picks another. While it runs, stop and restart take play's and the chevron's places. After
//! a failed run, a log segment appears and the outline turns red.

use std::rc::Rc;

use gpui::{AnyElement, App, ElementId, SharedString, Window, deferred, div, prelude::*};
use theme::{ActiveTheme, Typeset, dp};

use crate::{
    Callback, FadeText, Icon, IconName, Tone,
    menu::{Leading, menu, menu_divider, menu_row, menu_section},
};

/// Where the last run got to.
#[derive(Clone, Debug, PartialEq)]
pub enum RunState {
    Idle,
    /// Running for `elapsed`, such as 0:42.
    Running {
        elapsed: SharedString,
    },
    Failed,
}

/// What a click on the Run button asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunAction {
    Run,
    Stop,
    Restart,
    OpenLog,
    /// Picks the configuration at this index.
    Choose(usize),
    EditConfigs,
}

/// A configuration the picker lists.
pub struct RunChoice {
    pub name: SharedString,
    pub command: SharedString,
}

#[derive(IntoElement)]
pub struct RunButton {
    choices: Vec<RunChoice>,
    selected: Option<usize>,
    state: RunState,
    open: Option<bool>,
    hovered: Option<usize>,
    notice: Option<AnyElement>,
    on_action: Option<Callback<RunAction>>,
}

impl RunButton {
    /// Named after the selected choice, or Run when there's none.
    pub fn new(choices: Vec<RunChoice>, selected: Option<usize>) -> Self {
        Self {
            choices,
            selected,
            state: RunState::Idle,
            open: None,
            hovered: None,
            notice: None,
            on_action: None,
        }
    }

    pub fn state(mut self, state: RunState) -> Self {
        self.state = state;
        self
    }

    /// Shows the picker open with the row at `hovered` filled, for a mock. The chevron opens it
    /// otherwise. The row after the choices is Edit run configurations.
    pub fn open(mut self, hovered: Option<usize>) -> Self {
        self.open = Some(true);
        self.hovered = hovered;
        self
    }

    /// Shown under the button, such as a notice about clicking it.
    pub fn notice(mut self, notice: Option<AnyElement>) -> Self {
        self.notice = notice;
        self
    }

    pub fn on_action(
        mut self,
        handler: impl Fn(RunAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for RunButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let opened = window.use_keyed_state(ElementId::Name("run-open".into()), cx, |_, _| false);
        let running = matches!(self.state, RunState::Running { .. });
        let open = !running && self.open.unwrap_or(*opened.read(cx));
        let name = self
            .selected
            .and_then(|index| self.choices.get(index))
            .map_or(SharedString::from("Run"), |choice| choice.name.clone());
        let act = |action: RunAction, on_action: &Option<Callback<RunAction>>| {
            let on_action = on_action.clone();
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                if let Some(handler) = &on_action {
                    handler(action, window, cx)
                }
            }
        };
        let menu = open.then(|| {
            let mut rows: Vec<AnyElement> = vec![menu_section("Saved", cx)];
            if self.choices.is_empty() {
                rows.push(
                    div()
                        .px_2()
                        .h(cx.density().row_height)
                        .flex()
                        .items_center()
                        .text_color(cx.theme().colors.text_faint)
                        .child("No saved configurations")
                        .into_any_element(),
                );
            }
            let count = self.choices.len();
            for (index, choice) in self.choices.into_iter().enumerate() {
                let hovered = self.hovered == Some(index);
                let leading = if self.selected == Some(index) {
                    Leading::Check
                } else {
                    Leading::Blank
                };
                let opened = opened.clone();
                let choose = act(RunAction::Choose(index), &self.on_action);
                rows.push(
                    menu_row(index, leading, hovered, cx)
                        .child(div().flex_none().child(choice.name))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_family(cx.theme().fonts.code.clone())
                                .typeset(cx.density().small)
                                .text_color(cx.theme().colors.text_faint)
                                .child(FadeText::new(choice.command).background(if hovered {
                                    |colors| colors.surface_selected
                                } else {
                                    |colors| colors.surface_raised
                                })),
                        )
                        .on_click(move |event, window, cx| {
                            opened.write(cx, false);
                            choose(event, window, cx)
                        })
                        .into_any_element(),
                );
            }
            rows.push(menu_divider(cx));
            let opened_edit = opened.clone();
            let edit = act(RunAction::EditConfigs, &self.on_action);
            rows.push(
                menu_row(
                    "edit",
                    Leading::Icon(IconName::FileCog),
                    self.hovered == Some(count),
                    cx,
                )
                .child("Edit run configurations")
                .on_click(move |event, window, cx| {
                    opened_edit.write(cx, false);
                    edit(event, window, cx)
                })
                .into_any_element(),
            );
            let close = opened.clone();
            menu(rows, move |cx| close.write(cx, false), cx)
        });
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let failed = self.state == RunState::Failed;
        let edge = if failed {
            Tone::Error.outline(colors)
        } else {
            colors.border
        };
        let segment = |id: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .w(dp(24.))
                .border_l_1()
                .border_color(edge)
                .hover(|segment| segment.bg(colors.surface_selected))
                .when(id != "log", |segment| segment.rounded_r(density.radius_md))
        };
        let go = div()
            .id("go")
            .debug_selector(|| "run".into())
            .flex()
            .items_center()
            .gap_1p5()
            .pl_2()
            .pr_2p5()
            .rounded_l(density.radius_md)
            .hover(|go| go.bg(colors.surface_selected))
            .child(match &self.state {
                RunState::Running { .. } => Icon::new(IconName::Square)
                    .size(dp(12.))
                    .color(colors.diff_removed),
                _ => Icon::new(IconName::Play)
                    .size(dp(14.))
                    .color(colors.diff_added),
            })
            .child(name)
            .when_some(
                match &self.state {
                    RunState::Running { elapsed } => Some(elapsed.clone()),
                    _ => None,
                },
                |go, elapsed| {
                    go.child(
                        div()
                            .font_family(theme.fonts.code.clone())
                            .text_color(colors.text_faint)
                            .child(elapsed),
                    )
                },
            )
            .on_click(act(
                if running {
                    RunAction::Stop
                } else {
                    RunAction::Run
                },
                &self.on_action,
            ));
        let log = failed.then(|| {
            segment("log")
                .child(
                    Icon::new(IconName::ScrollText)
                        .size(dp(13.))
                        .color(colors.diff_removed),
                )
                .on_click(act(RunAction::OpenLog, &self.on_action))
        });
        let pick = if running {
            segment("restart")
                .child(Icon::new(IconName::RotateCcw).size(dp(12.)))
                .on_click(act(RunAction::Restart, &self.on_action))
        } else {
            segment("pick")
                .when(open, |pick| pick.bg(colors.surface_selected))
                .child(
                    Icon::new(IconName::ChevronDown)
                        .size(dp(12.))
                        .color(if open { colors.text } else { colors.text_faint }),
                )
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    opened.update(cx, |open, cx| {
                        *open = !*open;
                        cx.notify();
                    });
                })
        };
        div()
            .id("run")
            .relative()
            .flex()
            .flex_none()
            .h(dp(28.))
            .rounded(density.radius_md)
            .border_1()
            .border_color(edge)
            .bg(colors.surface_raised)
            .typeset(density.small)
            .text_color(colors.text)
            .whitespace_nowrap()
            .child(go)
            .children(log)
            .child(pick)
            .children(menu)
            .children(self.notice.map(|notice| {
                div()
                    .absolute()
                    .top_full()
                    .right_0()
                    .mt_1p5()
                    .child(deferred(notice).priority(1))
            }))
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use crate::TitleBar;

    fn bar(run: RunButton) -> AnyElement {
        div()
            .w(dp(640.))
            .child(TitleBar::new(None).controls(true).run(run))
            .into_any_element()
    }

    fn choices() -> Vec<RunChoice> {
        [
            ("pensive", "cargo run"),
            (
                "demo",
                "cargo run -p pensive --features demo -- --size 1280x800",
            ),
            ("mocks", "cargo mocks"),
        ]
        .map(|(name, command)| RunChoice {
            name: name.into(),
            command: command.into(),
        })
        .into()
    }
    vec![
        mock::Mock::component("run_button", "gallery", || {
            crate::Gallery::new("RunButton")
                .section(
                    "States",
                    [
                        ("Idle", RunState::Idle),
                        (
                            "Running",
                            RunState::Running {
                                elapsed: "0:42".into(),
                            },
                        ),
                        ("Failed", RunState::Failed),
                    ]
                    .map(|(label, state)| {
                        (label, bar(RunButton::new(choices(), Some(0)).state(state)))
                    }),
                )
                .section(
                    "Nothing saved",
                    [("", bar(RunButton::new(Vec::new(), None)))],
                )
                .into_any_element()
        })
        .size(800., 560.),
        mock::Mock::component("run_button", "picker", || {
            crate::Gallery::new("RunButton")
                .section(
                    "Picker open, with demo hovered",
                    [("", bar(RunButton::new(choices(), Some(0)).open(Some(1))))],
                )
                .into_any_element()
        })
        .size(800., 400.),
    ]
}

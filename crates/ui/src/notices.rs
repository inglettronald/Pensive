//! How notices look: as a tooltip on the control they're about, as toasts in the window's
//! corner, and as the list the badge opens.

use std::time::{Duration, SystemTime};

use components::{NoticeList, NoticeRow, Tag, Toast, Tone, Tooltip};
use gpui::{App, Context, Entity, Subscription, Task, Window, div, prelude::*};
use model::{Notice, Notices, Origin, Severity};
use settings::{AnySetting, Choice, Control, Page, Setting, Settings};
use theme::{ActiveTheme, dp};

use crate::{age::age, view_callback, view_callback_with};

pub(crate) fn tone(severity: Severity) -> Tone {
    match severity {
        Severity::Success => Tone::Success,
        Severity::Warning => Tone::Warning,
        Severity::Error => Tone::Error,
    }
}

/// Fixture is blue, so a gap in the sample data never reads as a bug in the interface.
pub(crate) fn tag(origin: Origin) -> Tag {
    let tag = Tag::new(origin.label());
    if origin == Origin::Fixture {
        tag.accent()
    } else {
        tag
    }
}

pub(crate) fn tooltip(notice: &Notice) -> Tooltip {
    let tooltip = Tooltip::new(notice.title.clone())
        .tone(tone(notice.severity))
        .tag(tag(notice.origin));
    match &notice.detail {
        Some(detail) => tooltip.detail(detail.clone()),
        None => tooltip,
    }
}

/// How long a toast shows before it folds into the badge.
pub(crate) static TOAST_SECONDS: Setting<u32> = Setting {
    key: "notices.toast_seconds",
    name: "Toast duration",
    description: "How long a notice shows in the corner before it folds into the badge",
    default: 3,
    control: Control::Select(|_| {
        [2, 3, 5, 10]
            .map(|seconds| Choice::new(seconds, format!("{seconds} seconds")))
            .into()
    }),
};

pub(crate) fn declare_settings(cx: &mut App) {
    let settings: [&'static dyn AnySetting; 1] = [&TOAST_SECONDS];
    Settings::declare(Page::new("Notices", "Toasts"), settings, cx);
}

/// A toast on screen, and how much of its time is left.
struct Shown {
    id: u64,
    notice: Notice,
    left: Duration,
    /// While it drains: when it started to, and the task that folds it once `left` runs out.
    draining: Option<(SystemTime, Task<()>)>,
}

/// The toasts in a window's corner, and the list of held notices that the badge opens.
pub(crate) struct NoticeOverlay {
    notices: Entity<Notices>,
    toasts: Vec<Shown>,
    list_open: bool,
    _subscriptions: [Subscription; 2],
}

impl NoticeOverlay {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let notices = Notices::global(cx);
        let observe = cx.observe(&notices, |this, _, cx| this.sync(cx));
        // A window that closes with a toast under the pointer never sees the pointer leave.
        let release = cx.on_release(|this, cx| {
            let paused = this.toasts.iter().filter(|shown| shown.draining.is_none());
            let ids: Vec<u64> = paused.map(|shown| shown.id).collect();
            this.notices.update(cx, |notices, cx| {
                for id in ids {
                    notices.let_go(id, cx);
                }
            });
        });
        let mut overlay = Self {
            notices,
            toasts: Vec::new(),
            list_open: false,
            _subscriptions: [observe, release],
        };
        overlay.sync(cx);
        overlay
    }

    /// How many errors and warnings the badge shows, and whether their list is open.
    pub fn badge(&self, cx: &App) -> (usize, usize, bool) {
        let (errors, warnings) = self.notices.read(cx).counts();
        (errors, warnings, self.list_open)
    }

    pub fn toggle_list(&mut self, cx: &mut Context<Self>) {
        self.list_open = !self.list_open;
        cx.notify();
    }

    /// Shows the toasts the model has that aren't on screen yet, and drops the ones it folded.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let waiting = self.notices.read(cx).toasts().to_vec();
        self.toasts
            .retain(|shown| waiting.iter().any(|(id, _)| *id == shown.id));
        let total = Duration::from_secs(TOAST_SECONDS.get(cx).into());
        for (id, notice) in waiting {
            if !self.toasts.iter().any(|shown| shown.id == id) {
                self.toasts.push(Shown {
                    id,
                    notice,
                    left: total,
                    draining: None,
                });
                self.drain(id, cx);
            }
        }
        let (errors, warnings) = self.notices.read(cx).counts();
        if errors + warnings == 0 {
            self.list_open = false;
        }
        cx.notify();
    }

    fn drain(&mut self, id: u64, cx: &mut Context<Self>) {
        let now = self.notices.read(cx).now();
        let Some(shown) = self.toasts.iter_mut().find(|shown| shown.id == id) else {
            return;
        };
        let left = shown.left;
        let notices = self.notices.clone();
        let fold = cx.spawn(async move |_, cx| {
            cx.background_executor().timer(left).await;
            notices.update(cx, |notices, cx| notices.expire(id, cx));
        });
        shown.draining = Some((now, fold));
    }

    /// Holds a toast while the pointer is on it, in every window, and lets it drain again when
    /// the pointer leaves.
    fn hover(&mut self, id: u64, hovered: bool, cx: &mut Context<Self>) {
        let Some(shown) = self.toasts.iter_mut().find(|shown| shown.id == id) else {
            return;
        };
        // Draining means not held, so a repeated hover event changes nothing.
        match (hovered, shown.draining.take()) {
            (true, Some((since, _))) => {
                let now = self.notices.read(cx).now();
                shown.left = shown
                    .left
                    .saturating_sub(now.duration_since(since).unwrap_or_default());
                self.notices.update(cx, |notices, _| notices.pause(id));
            }
            (false, None) => {
                self.notices.update(cx, |notices, _| notices.resume(id));
                self.drain(id, cx);
            }
            (_, draining) => shown.draining = draining,
        }
        cx.notify();
    }

    fn toast(&self, shown: &Shown, total: Duration, cx: &mut Context<Self>) -> Toast {
        let id = shown.id;
        let notice = &shown.notice;
        let toast = Toast::new(
            ("toast", id as usize),
            tone(notice.severity),
            notice.title.clone(),
        )
        .tag(tag(notice.origin))
        .on_close(view_callback(cx, move |this: &mut Self, _, cx| {
            this.notices.update(cx, |notices, cx| notices.fold(id, cx))
        }))
        .on_hover(view_callback_with(
            cx,
            move |this: &mut Self, hovered, _, cx| this.hover(id, hovered, cx),
        ));
        let toast = match &notice.detail {
            Some(detail) => toast.detail(detail.clone()),
            None => toast,
        };
        // Only an error folds into the badge, so only an error shows the time left until it does.
        if notice.severity != Severity::Error {
            return toast;
        }
        let left = shown.left.as_secs_f32() / total.as_secs_f32().max(f32::EPSILON);
        toast.timer(left.min(1.), shown.draining.as_ref().map(|_| shown.left))
    }

    fn list(&self, cx: &mut Context<Self>) -> NoticeList {
        let notices = self.notices.read(cx);
        let now = notices.now();
        let rows: Vec<NoticeRow> = notices
            .held()
            .into_iter()
            .map(|held| {
                let id = held.id;
                let notice = &held.notice;
                let row = NoticeRow::new(
                    ("held", id as usize),
                    tone(notice.severity),
                    notice.title.clone(),
                )
                .tag(tag(notice.origin))
                .count(held.count)
                .age(age(now, held.at));
                match &notice.detail {
                    Some(detail) => row.detail(detail.clone()),
                    None => row,
                }
                .on_dismiss(view_callback(cx, move |this: &mut Self, _, cx| {
                    this.notices
                        .update(cx, |notices, cx| notices.dismiss(id, cx))
                }))
            })
            .collect();
        NoticeList::new(rows).on_clear(view_callback(cx, |this: &mut Self, _, cx| {
            this.notices.update(cx, |notices, cx| notices.clear(cx))
        }))
    }
}

/// Toasts drop in under the title bar at the top right. The list opens under the badge, and
/// hides the toasts while it's open.
impl Render for NoticeOverlay {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_bar = cx.density().row_height + dp(12.);
        let total = Duration::from_secs(TOAST_SECONDS.get(cx).into());
        let corner = div()
            .absolute()
            .right_3()
            .flex()
            .flex_col()
            .items_end()
            .gap_2();
        if self.list_open {
            return corner.top(title_bar + dp(4.)).child(self.list(cx));
        }
        let toasts: Vec<Toast> = self
            .toasts
            .iter()
            .map(|shown| self.toast(shown, total, cx))
            .collect();
        corner.top(title_bar + dp(12.)).children(toasts)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use gpui::AnyView;
    use mock::Mock;
    use model::fixtures::PENSIVE;

    use crate::mock_window;

    /// The pensive project's window after the samples below, with their toasts folded into the
    /// badge if `fold`, and the list open if `open`.
    fn window(fold: bool, open: bool, window: &mut Window, cx: &mut App) -> AnyView {
        Notices::global(cx).update(cx, |notices, cx| {
            samples(notices, cx);
            if fold {
                let toasts: Vec<u64> = notices.toasts().iter().map(|(id, _)| *id).collect();
                for id in toasts {
                    notices.fold(id, cx);
                }
            }
        });
        let pensive = mock_window::pensive(Some(PENSIVE), window, cx);
        if open {
            let overlay = pensive.read(cx).notices();
            overlay.update(cx, NoticeOverlay::toggle_list);
        }
        pensive.into()
    }

    /// Four minutes ago, a settings warning three times and a settings error. A minute ago, a
    /// worktree the sample data doesn't have.
    fn samples(notices: &mut Notices, cx: &mut Context<Notices>) {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let notice = |severity, origin, title: &str, detail: &str| Notice {
            severity,
            origin,
            title: title.to_string().into(),
            detail: Some(detail.to_string().into()),
        };
        notices.as_of = Some(start);
        for _ in 0..3 {
            notices.report(
                notice(
                    Severity::Warning,
                    Origin::Config,
                    "Setting ignored",
                    "`palette.blur` isn't a setting",
                ),
                cx,
            );
        }
        notices.report(
            notice(
                Severity::Error,
                Origin::Config,
                "Couldn't read settings.toml",
                "line 14: expected a boolean",
            ),
            cx,
        );
        notices.as_of = Some(start + Duration::from_secs(180));
        notices.report(
            notice(
                Severity::Error,
                Origin::Fixture,
                "Couldn't read changes",
                "~/code/harbor-wt/tooltip-clip is not in a sample repository",
            ),
            cx,
        );
        notices.as_of = Some(start + Duration::from_secs(240));
    }

    vec![
        Mock::new("notices", "toasting", |w, cx| window(false, false, w, cx)),
        Mock::new("notices", "folded", |w, cx| window(true, false, w, cx)),
        Mock::new("notices", "list_open", |w, cx| window(true, true, w, cx)),
    ]
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn an_error_toasts_until_its_time_runs_out_and_stays_held(cx: &mut TestAppContext) {
        let overlay = cx.new(NoticeOverlay::new);
        let notices = cx.update(Notices::global);
        notices.update(cx, |notices, cx| {
            notices.report(
                Notice {
                    severity: Severity::Error,
                    origin: Origin::Git,
                    title: "Fetch failed".into(),
                    detail: None,
                },
                cx,
            )
        });
        cx.run_until_parked();
        let id = overlay.read_with(cx, |overlay, _| overlay.toasts[0].id);

        // Held while the pointer is on it, however long that is.
        overlay.update(cx, |overlay, cx| overlay.hover(id, true, cx));
        cx.executor().advance_clock(Duration::from_secs(10));
        cx.run_until_parked();
        overlay.read_with(cx, |overlay, _| assert_eq!(overlay.toasts.len(), 1));

        overlay.update(cx, |overlay, cx| overlay.hover(id, false, cx));
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        overlay.read_with(cx, |overlay, _| assert!(overlay.toasts.is_empty()));
        notices.read_with(cx, |notices, _| assert_eq!(notices.counts(), (1, 0)));
    }

    #[gpui::test]
    fn a_toast_under_the_pointer_in_one_window_stays_in_every_window(cx: &mut TestAppContext) {
        let first = cx.new(NoticeOverlay::new);
        let second = cx.new(NoticeOverlay::new);
        let notices = cx.update(Notices::global);
        notices.update(cx, |notices, cx| {
            notices.report(
                Notice {
                    severity: Severity::Error,
                    origin: Origin::Git,
                    title: "Fetch failed".into(),
                    detail: None,
                },
                cx,
            )
        });
        cx.run_until_parked();
        let id = first.read_with(cx, |overlay, _| overlay.toasts[0].id);
        let toasting = |cx: &mut TestAppContext| {
            let shown = |overlay: &Entity<NoticeOverlay>, cx: &mut TestAppContext| {
                overlay.read_with(cx, |overlay, _| overlay.toasts.len())
            };
            (shown(&first, cx), shown(&second, cx))
        };

        // The second window's time runs out, but the first window holds the toast.
        first.update(cx, |overlay, cx| overlay.hover(id, true, cx));
        cx.executor().advance_clock(Duration::from_secs(10));
        cx.run_until_parked();
        assert_eq!(toasting(cx), (1, 1));

        first.update(cx, |overlay, cx| overlay.hover(id, false, cx));
        cx.executor().advance_clock(Duration::from_secs(10));
        cx.run_until_parked();
        assert_eq!(toasting(cx), (0, 0));
    }

    #[gpui::test]
    fn closing_a_window_lets_go_of_the_toast_it_held(cx: &mut TestAppContext) {
        let closing = cx.new(NoticeOverlay::new);
        let staying = cx.new(NoticeOverlay::new);
        let notices = cx.update(Notices::global);
        notices.update(cx, |notices, cx| {
            notices.report(
                Notice {
                    severity: Severity::Error,
                    origin: Origin::Git,
                    title: "Fetch failed".into(),
                    detail: None,
                },
                cx,
            )
        });
        cx.run_until_parked();
        let id = closing.read_with(cx, |overlay, _| overlay.toasts[0].id);
        closing.update(cx, |overlay, cx| overlay.hover(id, true, cx));
        drop(closing);
        cx.executor().advance_clock(Duration::from_secs(10));
        cx.run_until_parked();
        staying.read_with(cx, |overlay, _| assert!(overlay.toasts.is_empty()));
    }
}

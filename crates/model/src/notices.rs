//! The app's notices: warnings and errors held until dismissed, and the errors and successes
//! waiting to show as toasts.

use std::{cmp::Reverse, time::SystemTime};

use gpui::{App, AppContext, Context, Entity, Global};

use crate::{Notice, Severity, clock};

/// A warning or error the user hasn't dismissed. A repeat counts here instead of adding a row.
#[derive(Clone, Debug)]
pub struct Held {
    pub id: u64,
    pub notice: Notice,
    pub count: usize,
    /// When it last happened.
    pub at: SystemTime,
}

#[derive(Default)]
pub struct Notices {
    next_id: u64,
    held: Vec<Held>,
    toasts: Vec<(u64, Notice)>,
    /// The time ages are measured from, fixed in fixtures so mocks don't change. The clock
    /// otherwise.
    pub as_of: Option<SystemTime>,
}

struct GlobalNotices(Entity<Notices>);

impl Global for GlobalNotices {}

impl Notices {
    /// The app's notices, made on first use.
    pub fn global(cx: &mut App) -> Entity<Self> {
        if let Some(notices) = cx.try_global::<GlobalNotices>() {
            return notices.0.clone();
        }
        let notices = cx.new(|_| Self::default());
        Self::set_global(notices.clone(), cx);
        notices
    }

    /// Makes `notices` the app's, such as a fresh store for a mock so its samples stay out of the
    /// app's own.
    pub fn set_global(notices: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalNotices(notices));
    }

    /// Reports `notice` to the app's notices.
    pub fn post(notice: Notice, cx: &mut App) {
        Self::global(cx).update(cx, |notices, cx| notices.report(notice, cx));
    }

    /// Successes toast and fade. Warnings only go to the held list, so a noisy source can't take
    /// over the screen. Errors do both.
    pub fn report(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let at = self.now();
        if notice.severity != Severity::Success {
            match self.held.iter_mut().find(|held| held.notice == notice) {
                Some(held) => {
                    held.count += 1;
                    held.at = at;
                }
                None => self.held.push(Held {
                    id,
                    notice: notice.clone(),
                    count: 1,
                    at,
                }),
            }
        }
        if notice.severity != Severity::Warning {
            self.toasts.push((id, notice));
        }
        cx.notify();
    }

    pub fn now(&self) -> SystemTime {
        self.as_of.unwrap_or_else(clock::now)
    }

    /// Errors first, then warnings, newest first within each.
    pub fn held(&self) -> Vec<&Held> {
        let mut held: Vec<&Held> = self.held.iter().collect();
        held.sort_by_key(|held| Reverse((held.notice.severity, held.at)));
        held
    }

    /// How many errors and how many warnings are held.
    pub fn counts(&self) -> (usize, usize) {
        let errors = self
            .held
            .iter()
            .filter(|held| held.notice.severity == Severity::Error)
            .count();
        (errors, self.held.len() - errors)
    }

    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        self.held.retain(|held| held.id != id);
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.held.clear();
        cx.notify();
    }

    /// The notices still waiting to toast, oldest first, by id.
    pub fn toasts(&self) -> &[(u64, Notice)] {
        &self.toasts
    }

    /// Done toasting: an error stays held, a success is gone.
    pub fn fold(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|(toast, _)| *toast != id);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;
    use crate::Origin;

    fn notice(severity: Severity, title: &'static str) -> Notice {
        Notice {
            severity,
            origin: Origin::Config,
            title: title.into(),
            detail: None,
        }
    }

    #[gpui::test]
    fn each_severity_goes_where_it_belongs(cx: &mut TestAppContext) {
        let notices = cx.new(|_| Notices::default());
        notices.update(cx, |notices, cx| {
            notices.report(notice(Severity::Success, "Cloned"), cx);
            notices.report(notice(Severity::Warning, "Unknown setting"), cx);
            notices.report(notice(Severity::Error, "Couldn't read"), cx);
            let toasts: Vec<_> = notices
                .toasts()
                .iter()
                .map(|(_, n)| n.title.clone())
                .collect();
            assert_eq!(toasts, ["Cloned", "Couldn't read"]);
            let held: Vec<_> = notices
                .held()
                .iter()
                .map(|h| h.notice.title.clone())
                .collect();
            assert_eq!(held, ["Couldn't read", "Unknown setting"]);
            assert_eq!(notices.counts(), (1, 1));
        });
    }

    #[gpui::test]
    fn a_repeat_counts_instead_of_adding_a_row(cx: &mut TestAppContext) {
        let notices = cx.new(|_| Notices::default());
        notices.update(cx, |notices, cx| {
            for _ in 0..3 {
                notices.report(notice(Severity::Warning, "Unknown setting"), cx);
            }
            let held = notices.held();
            assert_eq!(held.len(), 1);
            assert_eq!(held[0].count, 3);
        });
    }
}

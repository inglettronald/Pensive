//! Keeps the settings and theme globals in step with the config files, and saves the settings
//! changed in the app.

use std::sync::Arc;

use gpui::{App, AppContext, UpdateGlobal};
use model::{ConfigBackend, ConfigEvent, ConfigFiles, Notice, Notices, Origin, Severity};
use settings::Settings;
use theme::UserThemes;

/// Loads the config files through `backend` and keeps them loaded. Call it after every crate has
/// declared its settings.
pub fn init(backend: Arc<dyn ConfigBackend>, cx: &mut App) {
    let files = cx.new(|_| ConfigFiles::new(backend));
    cx.subscribe(&files, |files, event, cx| match event {
        ConfigEvent::SettingsRead => {
            let source = files.read(cx).settings.clone();
            // The settings observer builds the theme.
            let problems = Settings::update_global(cx, |settings, _| settings.load(&source));
            warn("Setting ignored", problems, cx);
        }
        ConfigEvent::ThemesRead => {
            let themes = files.read(cx).themes.clone();
            let problems = cx.default_global::<UserThemes>().load(themes);
            warn("Theme problem", problems, cx);
            apply_theme(cx);
        }
        ConfigEvent::Problem(problem) => {
            // Most read "What failed: why". The part before the colon is the title.
            let (title, detail) = match problem.split_once(": ") {
                Some((title, detail)) => (title.to_string(), Some(detail.to_string().into())),
                None => (problem.clone(), None),
            };
            report(
                Notice {
                    severity: Severity::Error,
                    origin: Origin::Config,
                    title: title.into(),
                    detail,
                },
                cx,
            );
        }
    })
    .detach();
    cx.observe_global::<Settings>({
        let files = files.clone();
        move |cx| {
            let source = cx.global::<Settings>().source().to_string();
            files.update(cx, |files, cx| files.save_settings(source, cx));
            apply_theme(cx);
        }
    })
    .detach();
    files.update(cx, |files, cx| files.load(cx));
}

/// Installs the theme and density the settings ask for, and reports what went wrong.
fn apply_theme(cx: &mut App) {
    let problems = theme::apply(cx);
    warn("Theme problem", problems, cx);
    cx.refresh_windows();
}

/// One warning per problem, each with `title`. The same problem on a later load counts as a
/// repeat.
fn warn(title: &'static str, problems: impl IntoIterator<Item = String>, cx: &mut App) {
    for problem in problems {
        report(
            Notice {
                severity: Severity::Warning,
                origin: Origin::Config,
                title: title.into(),
                detail: Some(problem.into()),
            },
            cx,
        );
    }
}

fn report(notice: Notice, cx: &mut App) {
    Notices::global(cx).update(cx, |notices, cx| notices.report(notice, cx));
}

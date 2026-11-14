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
        let notice = Notice {
            severity: Severity::Warning,
            origin: Origin::Config,
            title: title.into(),
            detail: Some(problem.into()),
        };
        Notices::post(notice, cx);
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;
    use model::fixtures::FakeConfig;

    use super::*;

    /// The app's globals, kept in step with `config` as the app keeps them.
    fn synced(config: &Arc<FakeConfig>, cx: &mut TestAppContext) {
        cx.update(|cx| {
            theme::init(cx);
            ui::init(cx);
            init(config.clone(), cx);
        });
        cx.run_until_parked();
    }

    #[gpui::test]
    fn a_setting_changed_in_the_app_is_saved_to_the_file(cx: &mut TestAppContext) {
        let config = Arc::new(FakeConfig::new("# my settings\n", &[]));
        synced(&config, cx);
        cx.update(|cx| {
            let density = cx.global::<Settings>().find("density").unwrap();
            density.step(1, cx);
        });
        cx.run_until_parked();
        let saved = config.settings();
        assert!(saved.contains("# my settings\n"), "{saved}");
        assert!(saved.contains("density = "), "{saved}");
    }

    #[gpui::test]
    fn a_setting_the_file_gets_wrong_is_ignored_with_a_warning(cx: &mut TestAppContext) {
        let config = Arc::new(FakeConfig::new("density = 3\n", &[]));
        synced(&config, cx);
        let notices = cx.update(Notices::global);
        notices.read_with(cx, |notices, _| {
            let held = notices.held();
            assert_eq!(held.len(), 1);
            assert_eq!(held[0].notice.title, "Setting ignored");
        });
        assert_eq!(config.settings(), "density = 3\n");
    }
}

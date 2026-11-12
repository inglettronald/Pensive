//! Keeps the settings and theme globals in step with the config folder's files, and saves the
//! settings changed in the app.

use std::sync::Arc;

use config::ConfigFolder;
use gpui::{App, AppContext, UpdateGlobal};
use model::{ConfigEvent, ConfigFiles};
use settings::Settings;
use theme::UserThemes;

/// Loads the config folder and keeps it loaded. Call it after every crate has declared its
/// settings.
pub fn init(cx: &mut App) {
    let Some(folder) = ConfigFolder::find() else {
        eprintln!("Couldn't find a home folder, so every setting uses its default");
        return;
    };
    let files = cx.new(|_| ConfigFiles::new(Arc::new(folder)));
    cx.subscribe(&files, |files, event, cx| match event {
        ConfigEvent::SettingsRead => {
            let source = files.read(cx).settings.clone();
            // The settings observer builds the theme.
            let problems = Settings::update_global(cx, |settings, _| settings.load(&source));
            print(
                problems
                    .into_iter()
                    .map(|problem| format!("settings.toml: {problem}")),
            );
        }
        ConfigEvent::ThemesRead => {
            let themes = files.read(cx).themes.clone();
            print(cx.default_global::<UserThemes>().load(themes));
            apply_theme(cx);
        }
        ConfigEvent::Problem(problem) => eprintln!("{problem}"),
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

/// Installs the theme and density the settings ask for, and prints what went wrong.
fn apply_theme(cx: &mut App) {
    print(theme::apply(cx));
    cx.refresh_windows();
}

fn print(problems: impl IntoIterator<Item = String>) {
    for problem in problems {
        eprintln!("{problem}");
    }
}

//! The config folder: loads settings and themes from it, reloads them when the files change, and
//! writes changes made in the app back to `settings.toml`.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use etcetera::BaseStrategy;
use futures::{StreamExt, channel::mpsc};
use gpui::{App, Global, UpdateGlobal};
use notify::{RecursiveMode, Watcher};
use settings::Settings;
use theme::UserThemes;

struct Config {
    dir: PathBuf,
    /// `settings.toml` as last read or written, to tell Pensive's own writes from the user's.
    on_disk: String,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl Global for Config {}

impl Config {
    fn settings_file(&self) -> PathBuf {
        self.dir.join("settings.toml")
    }

    fn themes_dir(&self) -> PathBuf {
        self.dir.join("themes")
    }
}

/// Loads the config folder and keeps it loaded. Call it after every crate has declared its
/// settings.
pub fn init(cx: &mut App) {
    let Some(dir) = config_dir() else {
        eprintln!("Couldn't find a home folder, so every setting uses its default");
        return;
    };
    let mut config = Config {
        dir,
        on_disk: String::new(),
        _watcher: None,
    };
    if let Err(error) = fs::create_dir_all(config.themes_dir()) {
        eprintln!("Couldn't create {}: {error}", config.themes_dir().display());
    }
    config._watcher = watch(&config, cx);
    cx.set_global(config);
    reload(cx);
    cx.observe_global::<Settings>(|cx| {
        write_changes(cx);
        theme::apply(cx);
        cx.refresh_windows();
    })
    .detach();
}

/// `~/.config/pensive` on Linux and macOS, `%APPDATA%\Pensive` on Windows.
fn config_dir() -> Option<PathBuf> {
    let name = if cfg!(windows) { "Pensive" } else { "pensive" };
    Some(
        etcetera::choose_base_strategy()
            .ok()?
            .config_dir()
            .join(name),
    )
}

/// Watches the folder, the themes in it, and the real folder of a symlinked `settings.toml`.
/// Editors often save by replacing the file, so this watches folders rather than files.
fn watch(config: &Config, cx: &mut App) -> Option<notify::RecommendedWatcher> {
    let (changed, mut changes) = mpsc::unbounded();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok_and(|event| !event.kind.is_access()) {
            changed.unbounded_send(()).ok();
        }
    })
    .inspect_err(|error| eprintln!("Settings won't reload while Pensive runs: {error}"))
    .ok()?;
    let linked = fs::canonicalize(config.settings_file())
        .ok()
        .and_then(|file| Some(file.parent()?.to_path_buf()))
        .filter(|dir| *dir != config.dir);
    for dir in [Some(config.dir.clone()), Some(config.themes_dir()), linked]
        .into_iter()
        .flatten()
    {
        if let Err(error) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
            eprintln!("Changes in {} won't reload: {error}", dir.display());
        }
    }
    cx.spawn(async move |cx| {
        while changes.next().await.is_some() {
            // One save can be several events. Wait for the rest, then reload once.
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            while changes.try_recv().is_ok() {}
            cx.update(reload);
        }
    })
    .detach();
    Some(watcher)
}

/// Reads the themes and settings from disk, applies them, and prints what's wrong with them.
fn reload(cx: &mut App) {
    let config = cx.global::<Config>();
    let (themes, mut problems) = UserThemes::load(read_themes(&config.themes_dir()));
    let settings_file = config.settings_file();
    cx.set_global(themes);

    let source = match fs::read_to_string(&settings_file) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            problems.push(format!("Couldn't read settings.toml: {error}"));
            cx.global::<Config>().on_disk.clone()
        }
    };
    if source != cx.global::<Config>().on_disk || cx.try_global::<Settings>().is_none() {
        cx.global_mut::<Config>().on_disk = source.clone();
        let settings_problems = Settings::update_global(cx, |settings, _| settings.load(&source));
        problems.extend(
            settings_problems
                .into_iter()
                .map(|problem| format!("settings.toml: {problem}")),
        );
    }
    problems.extend(theme::apply(cx));
    cx.refresh_windows();
    for problem in problems {
        eprintln!("{problem}");
    }
}

/// Each `.toml` file in `dir`, as its name without the extension and its text.
fn read_themes(dir: &Path) -> Vec<(String, String)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut themes: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().into_owned();
            Some((name, fs::read_to_string(&path).ok()?))
        })
        .collect();
    themes.sort();
    themes
}

/// Writes `settings.toml` when the app changed a setting.
fn write_changes(cx: &mut App) {
    let Some(config) = cx.try_global::<Config>() else {
        return;
    };
    let source = cx.global::<Settings>().source();
    if source == config.on_disk {
        return;
    }
    let source = source.to_string();
    match write_through_links(&config.settings_file(), &source) {
        Ok(()) => cx.global_mut::<Config>().on_disk = source,
        Err(error) => eprintln!("Couldn't save settings.toml: {error}"),
    }
}

/// Replaces the file at `path` in one step, so a crash never leaves half a file. When `path` is
/// a symlink, this replaces what it points to, so the link survives.
fn write_through_links(path: &Path, text: &str) -> io::Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let temporary = target.with_extension("toml.saving");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, &target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pensive-{name}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn saving_through_a_symlink_keeps_the_link() {
        let dir = scratch("symlink");
        let dotfiles = dir.join("dotfiles.toml");
        let link = dir.join("settings.toml");
        fs::write(&dotfiles, "density = \"calm\"\n").unwrap();
        std::os::unix::fs::symlink(&dotfiles, &link).unwrap();

        write_through_links(&link, "density = \"dense\"\n").unwrap();

        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(
            fs::read_to_string(&dotfiles).unwrap(),
            "density = \"dense\"\n"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn themes_are_the_toml_files_by_name() {
        let dir = scratch("themes");
        fs::write(dir.join("nord.toml"), "[colors]").unwrap();
        fs::write(dir.join("notes.md"), "").unwrap();

        assert_eq!(
            read_themes(&dir),
            [("nord".to_string(), "[colors]".to_string())]
        );
        fs::remove_dir_all(dir).ok();
    }
}

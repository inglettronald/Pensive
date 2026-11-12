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

/// Loads the config folder and keeps it loaded. Call it after every crate has declared its
/// settings.
pub fn init(cx: &mut App) {
    let Some(dir) = config_dir() else {
        eprintln!("Couldn't find a home folder, so every setting uses its default");
        return;
    };
    let watcher = watch(&dir, cx);
    cx.set_global(Config {
        dir,
        on_disk: String::new(),
        _watcher: watcher,
    });
    reload(cx);
    cx.observe_global::<Settings>(|cx| {
        write_changes(cx);
        apply_theme(cx);
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

fn settings_file(dir: &Path) -> PathBuf {
    dir.join("settings.toml")
}

fn themes_dir(dir: &Path) -> PathBuf {
    dir.join("themes")
}

/// Watches the folder, the themes in it, and the real folder of a symlinked `settings.toml`.
/// Editors often save by replacing the file, so this watches folders rather than files. A folder
/// that doesn't exist yet isn't watched until the next launch.
fn watch(dir: &Path, cx: &mut App) -> Option<notify::RecommendedWatcher> {
    let (changed, mut changes) = mpsc::unbounded();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok_and(|event| !event.kind.is_access()) {
            changed.unbounded_send(()).ok();
        }
    })
    .inspect_err(|error| eprintln!("Settings won't reload while Pensive runs: {error}"))
    .ok()?;
    let linked = link_target(&settings_file(dir))
        .parent()
        .map(Path::to_path_buf)
        .filter(|linked| linked != dir);
    for folder in [Some(dir.to_path_buf()), Some(themes_dir(dir)), linked]
        .into_iter()
        .flatten()
        .filter(|folder| folder.is_dir())
    {
        if let Err(error) = watcher.watch(&folder, RecursiveMode::NonRecursive) {
            eprintln!("Changes in {} won't reload: {error}", folder.display());
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

/// Reads the themes and settings from disk and prints what's wrong with them. The theme is built
/// once: by the settings observer when the settings changed, otherwise here.
fn reload(cx: &mut App) {
    let dir = cx.global::<Config>().dir.clone();
    let themes = read_themes(&themes_dir(&dir));
    let problems = cx.default_global::<UserThemes>().load(themes);
    print(problems);

    let source = match read_settings(&dir) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("Couldn't read settings.toml: {error}");
            return apply_theme(cx);
        }
    };
    if source == cx.global::<Config>().on_disk {
        return apply_theme(cx);
    }
    load_settings(source, cx);
}

/// The settings file's text. A missing file reads as empty, which means every default.
fn read_settings(dir: &Path) -> io::Result<String> {
    match fs::read_to_string(settings_file(dir)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        result => result,
    }
}

/// Loads `source` as the settings file. The settings observer then builds the theme.
fn load_settings(source: String, cx: &mut App) {
    cx.global_mut::<Config>().on_disk = source.clone();
    let problems = Settings::update_global(cx, |settings, _| settings.load(&source));
    print(
        problems
            .into_iter()
            .map(|problem| format!("settings.toml: {problem}")),
    );
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

/// Writes `settings.toml` when the app changed a setting. If the file changed on disk since it
/// was last read, the user's edit wins: it's loaded and the app's change is dropped.
fn write_changes(cx: &mut App) {
    let Some(config) = cx.try_global::<Config>() else {
        return;
    };
    let source = cx.global::<Settings>().source();
    if source == config.on_disk {
        return;
    }
    let disk = match read_settings(&config.dir) {
        Ok(disk) => disk,
        Err(error) => return eprintln!("Couldn't save settings.toml: {error}"),
    };
    if disk != config.on_disk {
        eprintln!("settings.toml changed on disk, so the change made in Pensive wasn't saved");
        return load_settings(disk, cx);
    }
    let source = source.to_string();
    match write_through_links(&settings_file(&config.dir), &source) {
        Ok(()) => cx.global_mut::<Config>().on_disk = source,
        Err(error) => eprintln!("Couldn't save settings.toml: {error}"),
    }
}

/// Replaces the file at `path` in one step, so a crash never leaves half a file. When `path` is
/// a symlink, even one whose target doesn't exist yet, this replaces the target, so the link
/// survives.
fn write_through_links(path: &Path, text: &str) -> io::Result<()> {
    let target = link_target(path);
    if let Some(folder) = target.parent() {
        fs::create_dir_all(folder)?;
    }
    let temporary = target.with_extension("toml.saving");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, &target)
}

/// Where `path` ends up after following its symlinks. Unlike `fs::canonicalize`, the end doesn't
/// have to exist.
fn link_target(path: &Path) -> PathBuf {
    let mut path = path.to_path_buf();
    // A loop of links stops here and fails when it's written.
    for _ in 0..32 {
        let Ok(target) = fs::read_link(&path) else {
            break;
        };
        path = match path.parent() {
            Some(folder) => folder.join(target),
            None => target,
        };
    }
    path
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

    #[cfg(unix)]
    #[test]
    fn saving_through_a_dangling_symlink_creates_its_target() {
        let dir = scratch("dangling");
        let dotfiles = dir.join("dotfiles").join("pensive.toml");
        let link = dir.join("settings.toml");
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

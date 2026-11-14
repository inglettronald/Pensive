//! A `ConfigBackend` for the config folder on disk.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::Result;
use etcetera::BaseStrategy;
use model::ConfigBackend;
use notify::{RecursiveMode, Watcher};

#[derive(Debug)]
pub struct ConfigFolder {
    dir: PathBuf,
}

impl ConfigFolder {
    /// `~/.config/pensive` on Linux and macOS, `%APPDATA%\Pensive` on Windows. `None` when
    /// there's no home folder.
    pub fn find() -> Option<Self> {
        let name = if cfg!(windows) { "Pensive" } else { "pensive" };
        let base = etcetera::choose_base_strategy().ok()?;
        Some(Self::new(base.config_dir().join(name)))
    }

    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn settings_file(&self) -> PathBuf {
        self.dir.join("settings.toml")
    }

    fn themes_dir(&self) -> PathBuf {
        self.dir.join("themes")
    }
}

impl ConfigBackend for ConfigFolder {
    fn read_settings(&self) -> Result<String> {
        match fs::read_to_string(self.settings_file()) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            result => Ok(result?),
        }
    }

    /// Replaces the file in one step, so a crash never leaves half a file. When it's a symlink,
    /// even one whose target doesn't exist yet, this replaces the target, so the link survives.
    fn write_settings(&self, text: &str) -> Result<()> {
        let target = link_target(&self.settings_file());
        if let Some(folder) = target.parent() {
            fs::create_dir_all(folder)?;
        }
        let temporary = target.with_extension("toml.saving");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, &target)?;
        Ok(())
    }

    /// Unreadable files are left out.
    fn read_themes(&self) -> Result<Vec<(String, String)>> {
        let entries = match fs::read_dir(self.themes_dir()) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            entries => entries?,
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
        Ok(themes)
    }

    /// Watches the folder, the themes in it, and the real folder of a symlinked `settings.toml`.
    /// Editors often save by replacing the file, so this watches folders rather than files. A
    /// folder that doesn't exist yet isn't watched until the next launch.
    fn watch(&self, changed: Box<dyn Fn() + Send + Sync>) -> Result<Box<dyn Send>> {
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.is_ok_and(|event| !event.kind.is_access()) {
                    changed();
                }
            })?;
        let linked = link_target(&self.settings_file())
            .parent()
            .map(Path::to_path_buf)
            .filter(|linked| *linked != self.dir);
        for folder in [Some(self.dir.clone()), Some(self.themes_dir()), linked]
            .into_iter()
            .flatten()
            .filter(|folder| folder.is_dir())
        {
            watcher.watch(&folder, RecursiveMode::NonRecursive)?;
        }
        Ok(Box::new(watcher))
    }
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

    #[test]
    fn keeps_the_config_contract() {
        let dir = tempfile::tempdir().unwrap();
        let not_created_yet = dir.path().join("pensive");
        model::fixtures::contract::config(&ConfigFolder::new(not_created_yet));
    }

    #[test]
    fn a_missing_settings_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let folder = ConfigFolder::new(dir.path().join("pensive"));
        assert_eq!(folder.read_settings().unwrap(), "");
        assert!(folder.read_themes().unwrap().is_empty());
    }

    #[test]
    fn saving_creates_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = ConfigFolder::new(dir.path().join("pensive"));
        folder.write_settings("density = \"dense\"\n").unwrap();
        assert_eq!(folder.read_settings().unwrap(), "density = \"dense\"\n");
    }

    #[cfg(unix)]
    #[test]
    fn saving_through_a_symlink_keeps_the_link() {
        let dir = tempfile::tempdir().unwrap();
        let dotfiles = dir.path().join("dotfiles.toml");
        let link = dir.path().join("settings.toml");
        fs::write(&dotfiles, "density = \"calm\"\n").unwrap();
        std::os::unix::fs::symlink(&dotfiles, &link).unwrap();

        ConfigFolder::new(dir.path().into())
            .write_settings("density = \"dense\"\n")
            .unwrap();

        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(
            fs::read_to_string(&dotfiles).unwrap(),
            "density = \"dense\"\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn saving_through_a_dangling_symlink_creates_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let dotfiles = dir.path().join("dotfiles").join("pensive.toml");
        let link = dir.path().join("settings.toml");
        std::os::unix::fs::symlink(&dotfiles, &link).unwrap();

        ConfigFolder::new(dir.path().into())
            .write_settings("density = \"dense\"\n")
            .unwrap();

        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(
            fs::read_to_string(&dotfiles).unwrap(),
            "density = \"dense\"\n"
        );
    }

    #[test]
    fn themes_are_the_toml_files_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let themes = dir.path().join("themes");
        fs::create_dir(&themes).unwrap();
        fs::write(themes.join("nord.toml"), "[colors]").unwrap();
        fs::write(themes.join("notes.md"), "").unwrap();

        assert_eq!(
            ConfigFolder::new(dir.path().into()).read_themes().unwrap(),
            [("nord".to_string(), "[colors]".to_string())]
        );
    }
}

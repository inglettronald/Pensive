//! The Git and config backends Pensive starts with, which differ between the desktop and the
//! browser.

use std::{path::PathBuf, sync::Arc};

use model::{ConfigBackend, Projects};

/// What the app opens with.
pub struct Startup {
    pub projects: Projects,
    /// Repositories to open on top of the ones `projects` already has.
    pub paths: Vec<PathBuf>,
    /// Where settings and themes are read and saved. Without one, every setting keeps its default.
    pub config: Option<Arc<dyn ConfigBackend>>,
}

/// The repositories named on the command line, read with the `git` command, and the config
/// folder in the home folder. A demo build opens the sample projects instead.
#[cfg(not(target_family = "wasm"))]
pub fn startup() -> Startup {
    let config = config::ConfigFolder::find();
    if config.is_none() {
        eprintln!("Couldn't find a home folder, so every setting uses its default");
    }
    let (projects, paths) = projects();
    Startup {
        projects,
        paths,
        config: config.map(|folder| Arc::new(folder) as Arc<dyn ConfigBackend>),
    }
}

/// Until Pensive remembers projects, they come from the command line.
#[cfg(not(any(target_family = "wasm", feature = "demo")))]
fn projects() -> (Projects, Vec<PathBuf>) {
    let paths = std::env::args_os().skip(1).map(PathBuf::from).collect();
    (Projects::new(Arc::new(git::GitCli)), paths)
}

#[cfg(all(not(target_family = "wasm"), feature = "demo"))]
fn projects() -> (Projects, Vec<PathBuf>) {
    (model::fixtures::projects(), Vec::new())
}

/// A browser tab has no `git` command or config folder, so it shows the sample projects and
/// keeps settings until the tab closes.
#[cfg(target_family = "wasm")]
pub fn startup() -> Startup {
    Startup {
        projects: model::fixtures::projects(),
        paths: Vec::new(),
        config: Some(Arc::new(model::fixtures::FakeConfig::default())),
    }
}

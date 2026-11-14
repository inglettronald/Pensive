//! The Git and config backends Pensive starts with: the real ones on the desktop, and the sample
//! projects in the browser and in a demo build. docs/architecture.md lists what each one does.

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
/// folder in the home folder.
#[cfg(not(any(target_family = "wasm", feature = "demo")))]
pub fn startup() -> Startup {
    let config = config::ConfigFolder::find();
    if config.is_none() {
        eprintln!("Couldn't find a home folder, so every setting uses its default");
    }
    // Until Pensive remembers projects, they come from the command line.
    let paths = std::env::args_os().skip(1).map(PathBuf::from).collect();
    Startup {
        projects: Projects::new(Arc::new(git::GitCli)),
        paths,
        config: config.map(|folder| Arc::new(folder) as Arc<dyn ConfigBackend>),
    }
}

/// The sample projects. A browser tab has no `git` command or config folder, and a demo shouldn't
/// read or change the real config, which mocks would otherwise save their settings into.
#[cfg(any(target_family = "wasm", feature = "demo"))]
pub fn startup() -> Startup {
    Startup {
        projects: model::fixtures::projects(),
        paths: Vec::new(),
        config: Some(sample_config()),
    }
}

/// Settings in the tab's session storage, so a reload keeps them and a new tab starts from the
/// defaults.
#[cfg(target_family = "wasm")]
fn sample_config() -> Arc<dyn ConfigBackend> {
    match crate::session_storage::SessionStorageConfig::open() {
        Some(storage) => Arc::new(storage),
        None => {
            log::warn!("The browser denied session storage, so settings last until a reload");
            Arc::new(model::fixtures::FakeConfig::default())
        }
    }
}

/// Settings in memory, so every `cargo demo` starts from the defaults.
#[cfg(all(feature = "demo", not(target_family = "wasm")))]
fn sample_config() -> Arc<dyn ConfigBackend> {
    Arc::new(model::fixtures::FakeConfig::default())
}

//! The Git and config backends Pensive starts with: the real ones on the desktop, and the sample
//! ones in the browser and in a demo build.

use std::{path::PathBuf, sync::Arc};

use model::{ConfigBackend, Notice, Projects};
#[cfg(not(any(target_family = "wasm", feature = "demo")))]
use model::{GitProblem, Platform};

/// What the app opens with.
pub struct Startup {
    pub projects: Projects,
    /// Repositories to open on top of the ones `projects` already has.
    pub paths: Vec<PathBuf>,
    /// Where settings and themes are read and saved. Without one, every setting keeps its default.
    pub config: Option<Arc<dyn ConfigBackend>>,
    /// Looks for problems with this machine, such as a missing Git, and describes each. It
    /// blocks, so the app runs it off the UI thread.
    pub check: Box<dyn FnOnce() -> Vec<Notice> + Send>,
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
    let git = Arc::new(git::GitCli::from_env());
    Startup {
        projects: Projects::new(git.clone()),
        paths,
        config: config.map(|folder| Arc::new(folder) as Arc<dyn ConfigBackend>),
        check: Box::new(move || problems(git.installed())),
    }
}

/// An error for a Git Pensive can't use, with this platform's instructions.
#[cfg(not(any(target_family = "wasm", feature = "demo")))]
fn problems(found: Result<git::Git, GitProblem>) -> Vec<Notice> {
    let problem = found.err();
    problem
        .map(|problem| problem.notice(Platform::CURRENT))
        .into_iter()
        .collect()
}

/// The sample projects, with settings kept in memory until the app closes. A browser tab has no
/// `git` command or config folder, and a demo shouldn't read or change the real config, which
/// mocks would otherwise save their settings into.
#[cfg(any(target_family = "wasm", feature = "demo"))]
pub fn startup() -> Startup {
    Startup {
        projects: model::fixtures::projects(),
        paths: Vec::new(),
        config: Some(Arc::new(model::fixtures::FakeConfig::default())),
        check: Box::new(Vec::new),
    }
}

#[cfg(all(test, not(any(target_family = "wasm", feature = "demo"))))]
mod tests {
    use model::{GitVersion, Origin, Severity};

    use super::*;

    #[test]
    fn a_git_new_enough_has_nothing_to_report() {
        let git = git::Git {
            path: "/usr/bin/git".into(),
            version: GitVersion::MINIMUM,
        };
        assert!(problems(Ok(git)).is_empty());
    }

    #[test]
    fn a_missing_git_is_one_error_from_git() {
        let notices = problems(Err(GitProblem::Missing));
        let [notice] = notices.as_slice() else {
            panic!("{notices:?}");
        };
        assert_eq!(notice.title, "Couldn't find Git");
        assert_eq!(
            (notice.severity, notice.origin),
            (Severity::Error, Origin::Git)
        );
    }
}

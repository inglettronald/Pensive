//! Why the Git on this machine can't run Pensive's commands, and what the user can do about it.

use std::{fmt, path::PathBuf};

/// A Git release, compared by its numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    /// The oldest Git with every command and flag the git backend runs. The newest of them,
    /// `git worktree list -z` and `git ls-tree --format`, arrived in 2.36.
    pub const MINIMUM: Self = Self::new(2, 36, 0);

    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

/// As Git's release notes write it, without a patch of zero: 2.36, 2.34.1.
impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if self.patch != 0 {
            write!(f, ".{}", self.patch)?;
        }
        Ok(())
    }
}

/// Which install instructions to give.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    /// Also the BSDs, which git-scm.com's Linux page covers.
    Linux,
}

impl Platform {
    /// The one Pensive was built for.
    pub const CURRENT: Self = if cfg!(target_os = "macos") {
        Self::MacOs
    } else if cfg!(windows) {
        Self::Windows
    } else {
        Self::Linux
    };
}

/// Why Pensive can't use the Git it looked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitProblem {
    /// No `git` in any of the places Pensive looks.
    Missing,
    /// A `git` whose `git version` failed, such as macOS's stand-in before the command line tools
    /// are installed.
    Broken {
        path: PathBuf,
        reason: String,
    },
    TooOld {
        path: PathBuf,
        version: GitVersion,
    },
}

impl GitProblem {
    pub fn title(&self) -> String {
        match self {
            GitProblem::Missing => "Couldn't find Git".into(),
            GitProblem::Broken { .. } => "Git won't run".into(),
            GitProblem::TooOld { version, .. } => format!("Git {version} is too old"),
        }
    }

    /// What was found, then what to do about it on `platform`.
    pub fn detail(&self, platform: Platform) -> String {
        let minimum = GitVersion::MINIMUM;
        let fix = self.fix(platform);
        match self {
            GitProblem::Missing => format!("Pensive needs Git {minimum} or newer. {fix}"),
            GitProblem::Broken { path, reason } => format!(
                "`{} version` didn't work: {}. {fix}",
                path.display(),
                first_line(reason)
            ),
            GitProblem::TooOld { path, version } => format!(
                "Pensive needs Git {minimum} or newer, and {} is {version}. {fix}",
                path.display()
            ),
        }
    }

    /// The next step on `platform`, from git-scm.com's install pages.
    fn fix(&self, platform: Platform) -> &'static str {
        let (install, update) = match platform {
            Platform::MacOs => (
                "Run `xcode-select --install` for Apple's Git, or `brew install git`.",
                "Run `brew install git`, or update the command line tools.",
            ),
            Platform::Windows => (
                "Get Git for Windows at https://git-scm.com/install/windows, or run \
                 `winget install --id Git.Git -e --source winget`.",
                "Get the latest at https://git-scm.com/install/windows, or run \
                 `git update-git-for-windows`.",
            ),
            Platform::Linux => (
                "Install it with your package manager, such as `sudo apt install git`. See \
                 https://git-scm.com/install/linux for other systems.",
                "On Ubuntu, run `sudo add-apt-repository ppa:git-core/ppa`, then \
                 `sudo apt update && sudo apt install git`. See https://git-scm.com/install/linux \
                 for other systems.",
            ),
        };
        match self {
            GitProblem::TooOld { .. } => update,
            GitProblem::Missing | GitProblem::Broken { .. } => install,
        }
    }
}

/// The first line of `reason` with text on it, cut to fit in a notice and without a closing
/// period, since the detail adds its own.
fn first_line(reason: &str) -> String {
    const LONGEST: usize = 120;
    let line = reason
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .trim_end_matches('.');
    match line.char_indices().nth(LONGEST) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

/// The title and detail, with the instructions for the platform Pensive was built for.
impl fmt::Display for GitProblem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}. {}", self.title(), self.detail(Platform::CURRENT))
    }
}

impl std::error::Error for GitProblem {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_and_drop_a_zero_patch() {
        assert!(GitVersion::new(2, 9, 5) < GitVersion::MINIMUM);
        assert!(GitVersion::new(2, 36, 1) > GitVersion::MINIMUM);
        assert_eq!(GitVersion::MINIMUM.to_string(), "2.36");
        assert_eq!(GitVersion::new(2, 34, 1).to_string(), "2.34.1");
    }

    #[test]
    fn a_git_too_old_names_both_versions_where_it_is_and_the_commands() {
        let problem = GitProblem::TooOld {
            path: "/usr/bin/git".into(),
            version: GitVersion::new(2, 34, 1),
        };
        assert_eq!(problem.title(), "Git 2.34.1 is too old");
        assert_eq!(
            problem.detail(Platform::Linux),
            "Pensive needs Git 2.36 or newer, and /usr/bin/git is 2.34.1. On Ubuntu, run \
             `sudo add-apt-repository ppa:git-core/ppa`, then `sudo apt update && sudo apt \
             install git`. See https://git-scm.com/install/linux for other systems."
        );
    }

    #[test]
    fn a_broken_git_quotes_the_first_line_of_its_error_cut_short() {
        let broken = |reason: &str| GitProblem::Broken {
            path: "/usr/bin/git".into(),
            reason: reason.into(),
        };
        let detail =
            broken("\nxcrun: error: invalid active developer path.\nmore").detail(Platform::MacOs);
        assert!(
            detail.starts_with(
                "`/usr/bin/git version` didn't work: xcrun: error: invalid active developer \
                 path. Run"
            ),
            "{detail}"
        );
        let detail = broken(&"x".repeat(500)).detail(Platform::MacOs);
        assert!(
            detail.contains(&format!("{}…. Run", "x".repeat(120))),
            "{detail}"
        );
    }
}

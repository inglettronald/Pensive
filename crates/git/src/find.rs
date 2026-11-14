//! Finds the Git a [`GitCli`] runs: the first on the `PATH`, or in a folder Git usually installs
//! to, that's new enough for every command in this crate.

use std::{
    collections::HashSet,
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

use model::{GitProblem, GitVersion};

use crate::{GitCli, command};

/// How long a problem stands before Pensive looks for Git again. Opening a repository runs about
/// ten commands in under a second, so they share one look, and macOS's install prompt can't reopen
/// with every command. Installing Git takes longer than this, so nobody waits on it.
const RECHECK: Duration = Duration::from_secs(10);

/// A Git new enough for Pensive's commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Git {
    pub path: PathBuf,
    pub version: GitVersion,
}

impl GitCli {
    /// Looks for Git on this process's `PATH`, then in the folders Git usually installs to.
    pub fn from_env() -> Self {
        Self::looking_in(candidates(&env::var_os("PATH").unwrap_or_default()))
    }

    /// Looks for Git only at `candidates`, in order.
    pub(crate) fn looking_in(candidates: Vec<PathBuf>) -> Self {
        Self {
            finder: Finder::new(candidates),
        }
    }

    /// The Git every call runs, or why there isn't one. It blocks while it looks.
    pub fn installed(&self) -> Result<Git, GitProblem> {
        self.finder.git()
    }
}

/// Where to look for Git, and what the last look found.
#[derive(Debug)]
pub(crate) struct Finder {
    candidates: Vec<PathBuf>,
    recheck: Duration,
    last: Mutex<Option<(Result<Git, GitProblem>, Instant)>>,
}

impl Finder {
    fn new(candidates: Vec<PathBuf>) -> Self {
        Self {
            candidates,
            recheck: RECHECK,
            last: Mutex::default(),
        }
    }

    /// The Git found, kept until [`Self::forget`]. A problem is kept for [`RECHECK`]. Calls that
    /// arrive while it looks wait for its answer instead of looking too.
    pub(crate) fn git(&self) -> Result<Git, GitProblem> {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        if matches!(&*last, Some((Err(_), at)) if at.elapsed() >= self.recheck) {
            *last = None;
        }
        let (found, _) = last.get_or_insert_with(|| (find(&self.candidates), Instant::now()));
        found.clone()
    }

    /// Makes the next call look again, such as after the Git it found went missing.
    pub(crate) fn forget(&self) {
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// Where Git might be, in the order to try: each folder on `path`, then where installers put Git
/// when an app's `PATH` may not include it. A relative folder is skipped, since it would run
/// whatever `git` is in the folder Pensive started from, and a folder listed twice is tried once.
fn candidates(path: &OsStr) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    env::split_paths(path)
        .filter(|folder| folder.is_absolute())
        .chain(install_folders())
        .map(|folder| git_in(&folder))
        .filter(|candidate| seen.insert(candidate.clone()))
        .collect()
}

fn git_in(folder: &Path) -> PathBuf {
    folder.join(format!("git{}", env::consts::EXE_SUFFIX))
}

/// An app opened from the Dock gets launchd's `PATH`, which has none of these.
#[cfg(target_os = "macos")]
fn install_folders() -> Vec<PathBuf> {
    ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"]
        .map(PathBuf::from)
        .into()
}

/// Git for Windows installs for every user or for one.
#[cfg(windows)]
fn install_folders() -> Vec<PathBuf> {
    [
        ("ProgramFiles", r"Git\cmd"),
        ("LOCALAPPDATA", r"Programs\Git\cmd"),
    ]
    .into_iter()
    .filter_map(|(base, folder)| Some(PathBuf::from(env::var_os(base)?).join(folder)))
    .collect()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn install_folders() -> Vec<PathBuf> {
    Vec::new()
}

fn find(candidates: &[PathBuf]) -> Result<Git, GitProblem> {
    first_usable(
        candidates
            .iter()
            .filter(|path| runnable(path))
            .map(|path| check(path.clone())),
    )
}

/// The first Git that's new enough. Failing that, the first one's problem, since that's the Git
/// the user's terminal runs. It stops at the first that's new enough, so later ones never run.
fn first_usable(
    checked: impl IntoIterator<Item = Result<Git, GitProblem>>,
) -> Result<Git, GitProblem> {
    let mut first_problem = None;
    for found in checked {
        match found {
            Ok(git) => return Ok(git),
            Err(problem) => {
                first_problem.get_or_insert(problem);
            }
        }
    }
    Err(first_problem.unwrap_or(GitProblem::Missing))
}

/// A file that could be a program: on Unix one with an execute bit, on Windows any file.
fn runnable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    metadata.is_file()
}

/// Runs `git version` on the file at `path`.
fn check(path: PathBuf) -> Result<Git, GitProblem> {
    let output = match command::clean(&path).arg("version").output() {
        Ok(output) => output,
        Err(error) => return Err(broken(path, error.to_string())),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = match (stderr.trim(), output.status.code()) {
            ("", Some(code)) => format!("it exited with code {code}"),
            ("", None) => "it was stopped".to_owned(),
            (stderr, _) => stderr.to_owned(),
        };
        return Err(broken(path, reason));
    }
    judge(path, &String::from_utf8_lossy(&output.stdout))
}

/// Whether the Git at `path`, which printed `printed` for `git version`, is new enough.
fn judge(path: PathBuf, printed: &str) -> Result<Git, GitProblem> {
    let Some(version) = parse_version(printed) else {
        return Err(broken(path, format!("it printed `{}`", printed.trim())));
    };
    if version < GitVersion::MINIMUM {
        return Err(GitProblem::TooOld { path, version });
    }
    Ok(Git { path, version })
}

fn broken(path: PathBuf, reason: String) -> GitProblem {
    GitProblem::Broken { path, reason }
}

/// Reads `git version 2.43.0`, and the builds that add to it, such as `2.39.5 (Apple Git-154)` and
/// `2.47.1.windows.2`. A missing patch number reads as zero.
fn parse_version(printed: &str) -> Option<GitVersion> {
    let numbers = printed.trim().strip_prefix("git version ")?;
    let mut numbers = numbers.split(['.', ' ']).map(str::parse::<u32>);
    let major = numbers.next()?.ok()?;
    let minor = numbers.next()?.ok()?;
    let patch = numbers.next().and_then(Result::ok).unwrap_or(0);
    Some(GitVersion::new(major, minor, patch))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use model::GitBackend as _;

    use super::*;

    /// A `git` in `folder` that fails to report its version. On Unix it's a link to `false`, which
    /// starts and exits with an error. On Windows it's a file that isn't a program.
    fn broken_git(folder: &Path) -> PathBuf {
        let path = git_in(folder);
        #[cfg(unix)]
        std::os::unix::fs::symlink("/usr/bin/false", &path).unwrap();
        #[cfg(windows)]
        fs::write(&path, "not a program").unwrap();
        path
    }

    fn real_git() -> Git {
        GitCli::from_env().installed().unwrap()
    }

    fn too_old(path: &str) -> GitProblem {
        GitProblem::TooOld {
            path: path.into(),
            version: GitVersion::new(2, 34, 1),
        }
    }

    #[test]
    fn reads_the_version_from_each_platforms_build() {
        let read = |printed| parse_version(printed).map(|version| version.to_string());
        assert_eq!(read("git version 2.43.0\n").as_deref(), Some("2.43"));
        assert_eq!(
            read("git version 2.39.5 (Apple Git-154)\n").as_deref(),
            Some("2.39.5")
        );
        assert_eq!(
            read("git version 2.47.1.windows.2\n").as_deref(),
            Some("2.47.1")
        );
        assert_eq!(read("git version 2.48.0.rc1").as_deref(), Some("2.48"));
        assert_eq!(read("git version 2.50").as_deref(), Some("2.50"));
        assert_eq!(read("hub version 2.14.2"), None);
        assert_eq!(read("git version"), None);
    }

    #[test]
    fn a_git_older_than_the_minimum_is_too_old() {
        let path = "/usr/bin/git";
        assert_eq!(
            judge(path.into(), "git version 2.34.1\n"),
            Err(too_old(path))
        );
        assert!(judge(path.into(), "git version 2.36.0\n").is_ok());
        assert!(matches!(
            judge(path.into(), "not git"),
            Err(GitProblem::Broken { .. })
        ));
    }

    #[test]
    fn only_absolute_folders_on_the_path_are_searched_and_each_once() {
        let folder = env::temp_dir();
        let relative = ["", ".", "bin"].map(PathBuf::from);
        let path = env::join_paths(relative.into_iter().chain([folder.clone(), folder.clone()]));
        let found = candidates(&path.unwrap());
        assert_eq!(found[0], git_in(&folder));
        assert!(found.iter().all(|candidate| candidate.is_absolute()));
        let listed = found
            .iter()
            .filter(|candidate| **candidate == git_in(&folder));
        assert_eq!(listed.count(), 1);
        assert!(
            candidates(OsStr::new(""))
                .iter()
                .all(|candidate| candidate.is_absolute())
        );
    }

    #[test]
    fn the_first_git_new_enough_wins_and_otherwise_the_first_problem() {
        let broken = || broken(PathBuf::from("/opt/git"), "it exited with code 1".into());
        assert_eq!(
            first_usable([Err(too_old("/usr/bin/git")), Err(broken())]),
            Err(too_old("/usr/bin/git"))
        );
        let git = real_git();
        assert_eq!(first_usable([Err(broken()), Ok(git.clone())]), Ok(git));
        assert_eq!(first_usable([]), Err(GitProblem::Missing));
    }

    #[test]
    fn a_file_that_isnt_a_program_is_skipped_on_unix_and_broken_on_windows() {
        let folder = tempfile::tempdir().unwrap();
        let fake = git_in(folder.path());
        fs::write(&fake, "not a program").unwrap();
        let found = find(std::slice::from_ref(&fake)).unwrap_err();
        if cfg!(unix) {
            assert_eq!(found, GitProblem::Missing);
        } else {
            assert!(matches!(found, GitProblem::Broken { path, .. } if path == fake));
        }
    }

    #[test]
    fn every_call_fails_with_the_problem_when_git_is_missing_or_broken() {
        let folder = tempfile::tempdir().unwrap();
        let missing = GitCli::looking_in(vec![git_in(folder.path())]);
        let error = missing.repository(folder.path()).unwrap_err();
        assert_eq!(error.downcast_ref(), Some(&GitProblem::Missing));

        let broken = GitCli::looking_in(vec![broken_git(folder.path())]);
        let error = broken.changes(folder.path()).unwrap_err();
        assert!(
            matches!(error.downcast_ref(), Some(GitProblem::Broken { .. })),
            "{error:#}"
        );
    }

    /// A link to the real Git puts one where there was none.
    #[cfg(unix)]
    #[test]
    fn a_problem_stands_until_git_is_looked_for_again() {
        use std::os::unix::fs::symlink;

        let folder = tempfile::tempdir().unwrap();
        let path = git_in(folder.path());
        let finder = Finder::new(vec![path.clone()]);
        assert_eq!(finder.git(), Err(GitProblem::Missing));
        symlink(real_git().path, &path).unwrap();
        assert_eq!(finder.git(), Err(GitProblem::Missing));
        finder.forget();
        assert_eq!(finder.git().unwrap().path, path);

        let later = folder.path().join("later");
        let impatient = Finder {
            recheck: Duration::ZERO,
            ..Finder::new(vec![git_in(&later)])
        };
        assert_eq!(impatient.git(), Err(GitProblem::Missing));
        fs::create_dir(&later).unwrap();
        symlink(&path, git_in(&later)).unwrap();
        assert!(impatient.git().is_ok());
    }
}

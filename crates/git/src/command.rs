//! How Pensive starts `git`: the one it found, in a clean environment, with no prompts and no
//! console window.

use std::{
    ffi::OsStr,
    io::{self, Write as _},
    path::Path,
    process::{Child, Command, Stdio},
};

use anyhow::{Context as _, Result, bail};
use model::GitProblem;

use crate::GitCli;

/// The variables that point Git at one repository, as `git rev-parse --local-env-vars` lists
/// them. Inherited from a Git hook or a shell, they would override `-C`.
const LOCAL_ENV_VARS: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

/// The Git at `git`, set up as every command runs it.
pub(crate) fn clean(git: &Path) -> Command {
    let mut command = Command::new(git);
    command
        // Git must never wait for a password on a terminal nobody can see.
        .env("GIT_TERMINAL_PROMPT", "0")
        // Skipping optional locks keeps reads from failing the user's own Git commands.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null());
    for var in LOCAL_ENV_VARS {
        command.env_remove(var);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // Without it, every call flashes a console window.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

impl GitCli {
    /// `git -C dir`, ready for arguments, or why there's no Git to run.
    pub(crate) fn command(&self, dir: &Path) -> Result<Command, GitProblem> {
        let mut command = clean(&self.finder.git()?.path);
        command.arg("-C").arg(dir);
        Ok(command)
    }

    /// Runs `git -C dir args` and returns what it printed, or fails with its error output.
    pub(crate) fn git(&self, dir: &Path, args: &[impl AsRef<OsStr>]) -> Result<Vec<u8>> {
        self.git_with_input(dir, args, None)
    }

    /// [`Self::git`], writing `input` to its standard input when there is some. Git's output is
    /// read after the input is written, so it suits commands that read their input before printing
    /// much.
    pub(crate) fn git_with_input(
        &self,
        dir: &Path,
        args: &[impl AsRef<OsStr>],
        input: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        let mut command = self.command(dir)?;
        command
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if input.is_some() {
            command.stdin(Stdio::piped());
        }
        let child = match command.spawn() {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // Git was uninstalled since it was found. This call says why, unless another Git
                // turned up, which the next call runs.
                self.finder.forget();
                self.finder.git()?;
                return Err(error).context("couldn't start Git");
            }
            started => started.context("couldn't start Git")?,
        };
        finish(child, dir, args, input)
    }
}

/// Writes `input` to the started `child`, waits for it, and returns what it printed.
fn finish(
    mut child: Child,
    dir: &Path,
    args: &[impl AsRef<OsStr>],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    // Dropping stdin closes it, which ends the input.
    let written = match (input, child.stdin.take()) {
        (Some(input), Some(mut stdin)) => stdin.write_all(input),
        _ => Ok(()),
    };
    let output = child
        .wait_with_output()
        .context("couldn't read what Git printed")?;
    if !output.status.success() {
        let args: Vec<_> = args
            .iter()
            .map(|arg| arg.as_ref().to_string_lossy())
            .collect();
        bail!(
            "`git {}` failed in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    // Checked after Git's own error, which says more when Git stopped reading early.
    written.context("couldn't write to `git`")?;
    Ok(output.stdout)
}

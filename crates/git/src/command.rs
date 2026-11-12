//! How Pensive starts `git`: in a clean environment, with no prompts and no console window.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context as _, Result, bail};

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

/// `git -C dir`, ready for arguments.
pub(crate) fn command(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
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

/// Runs `git -C dir args` and returns what it printed, or fails with its error output.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = command(dir)
        .args(args)
        .output()
        .context("couldn't run `git`. Is Git installed?")?;
    if !output.status.success() {
        bail!(
            "`git {}` failed in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

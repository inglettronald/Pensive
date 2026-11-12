//! Runs `git` the way every call from Pensive should, and returns what it printed.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context as _, Result, bail};

/// Runs `git -C dir args` and returns its standard output.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(args)
        // Git must never wait for a password on a terminal nobody can see.
        .env("GIT_TERMINAL_PROMPT", "0")
        // Reads skip the locks that would make the user's own Git commands fail meanwhile.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // Without it, every call flashes a console window.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
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

# Finding Git

Decided in PEN-29. Covers the oldest Git Pensive runs, how it finds that Git on each platform, what the user sees when it's missing or too old, and whether Pensive should bundle one. Facts were checked on the web in November 2026.

Pensive looks for Git the first time it needs it and runs that file by its full path from then on. A missing Git, one that won't run, or one older than 2.36 becomes an error that names the fix for the user's platform, shown as a notice when the app starts. Pensive doesn't bundle Git. Once it has installers, the Windows one should offer to install Git for Windows rather than carry a copy.

## The oldest Git: 2.36

These are the commands and flags in `crates/git` that set the floor, with the release that added each, from Git's [release notes](https://github.com/git/git/tree/master/Documentation/RelNotes):

| Command or flag | What Pensive uses it for | Added in |
|---|---|---|
| `git worktree list --porcelain -z` | Listing worktrees, with paths that aren't UTF-8 | 2.36 |
| `git ls-tree --format` | Finding a committed file's blob | 2.36 |
| `--pathspec-from-file`, `--pathspec-file-nul` | Staging and unstaging any number of paths | 2.25 |
| `--end-of-options` | Looking up commit times by name | 2.24 |
| `%(upstream:remotename)` in `for-each-ref` | Finding a branch's upstream | 2.16 |
| `GIT_OPTIONAL_LOCKS=0` | Reads that never block the user's own Git | 2.15 |
| `git status --porcelain=v2` | Listing uncommitted files with renames | 2.11 |
| `git cat-file --filters` | A committed file as checkout writes it | 2.11 |
| `git log --no-show-signature` | Keeping `log.showSignature` out of parsed output | 2.10 |
| `git -C <dir>` | Every command | 1.8.5 |

The rest, such as `rev-list --count`, `symbolic-ref --short` and `diff --numstat -z`, were all in Git by 2.18. Merge preview will raise the floor to 2.38 for `git merge-tree --write-tree`.

Who 2.36 leaves out:

| Platform | Its Git | New enough |
|---|---|---|
| Ubuntu 20.04 | [2.25.1](https://launchpad.net/ubuntu/focal/+source/git) | No |
| Ubuntu 22.04 | [2.34.1](https://packages.ubuntu.com/search?keywords=git&searchon=names&exact=1&suite=all&section=all) | No |
| Ubuntu 24.04 | 2.43.0 | Yes |
| Debian 11 | [2.30.2](https://packages.debian.org/search?keywords=git&searchon=names&exact=1&suite=all&section=all) | No |
| Debian 12 | 2.39.5 | Yes |
| RHEL 8.10 | 2.43.7, from [RHSA-2025:11534](https://access.redhat.com/errata/RHSA-2025:11534) | Yes |
| macOS, command line tools 26 | 2.50.1 | Yes |
| Git for Windows | [2.56.1](https://git-scm.com/install/windows) | Yes |

Debian 11's long-term support [ended in August 2026](https://www.debian.org/News/2026/20260831), so Ubuntu 22.04 is the one that matters. Dropping to 2.34 would mean parsing `ls-tree`'s default output, which is easy, and giving up `worktree list -z`, which isn't. Before 2.36, `--porcelain` didn't quote paths with unusual bytes, so a worktree whose path holds a newline would split into two entries. Pensive keeps 2.36, and the Linux fix points Ubuntu users at the git-core PPA.

## Where Pensive looks

1. Each folder on the `PATH`, in order. Relative entries such as `.` are skipped, since they'd run whatever `git` sits in the folder Pensive started from. A folder listed twice is tried once.
2. Then folders where Git lands but that aren't always on an app's `PATH`. On macOS that's `/opt/homebrew/bin`, `/usr/local/bin` and `/opt/local/bin`, for Homebrew on Apple silicon, Homebrew on Intel and MacPorts. On Windows it's `%ProgramFiles%\Git\cmd` and `%LOCALAPPDATA%\Programs\Git\cmd`, where Git for Windows installs for all users or for one. Linux has none.

Pensive runs `git version` on each file it finds that could be a program (on Unix, one with an execute bit) and keeps the first that's new enough. If none is, it reports the first one's problem, since that's the Git the user's terminal runs.

It keeps the Git it found until that file disappears. It keeps a problem for 10 seconds, then looks again. Opening a repository runs about ten commands in under a second, so they share one look instead of each running `git version` on every candidate. Commands that arrive while it looks wait for its answer. Installing Git takes longer than 10 seconds, so after installing it the user can open a repository again without restarting.

Every command then runs that file by its full path, so the Git whose version was checked is the Git that runs.

### macOS apps don't get the shell's `PATH`

An app opened from the Dock or Finder gets launchd's `PATH`, `/usr/bin:/bin:/usr/sbin:/sbin`, and never sees the folders a shell profile adds. The extra folders above let Pensive find a Homebrew Git, but Git then starts its own helpers from that same short `PATH`: `git-lfs`, credential helpers such as Git Credential Manager, `gpg` for signing. Those would still go missing.

Zed handles this by starting the user's login shell when it wasn't launched from a terminal and copying that shell's environment into its own (`load_login_shell_environment` in Zed's `util` crate, called from `crates/zed/src/main.rs` at v1.27.0). Pensive should do the same, in a follow-up ticket, since it changes the environment of every program Pensive starts and not only how it finds Git.

`/usr/bin/git` on macOS is a stand-in. With Xcode or its command line tools installed it runs Apple's Git. Without them, [running it prompts](https://developer.apple.com/documentation/xcode/installing-the-command-line-tools) the user to install the tools and fails. Pensive's `git version` can therefore open that prompt, at most once every 10 seconds while the tools are missing. The prompt is the fix the notice suggests anyway.

## What the user sees

When the app starts, it looks for Git off the UI thread and shows a problem as an error notice from Git. Like every error, it toasts for the toast duration setting (3 seconds by default), then folds into the badge's list. Every Git call after that fails with the same text, so a repository that won't open says why instead of showing a failed command.

| Problem | Title | Detail |
|---|---|---|
| No Git found | Couldn't find Git | Pensive needs Git 2.36 or newer. Then the platform's fix |
| A Git that won't run, such as macOS's stand-in without the tools | Git won't run | `<path> version` didn't work: the first line of its error output, cut at 120 characters. Then the fix |
| Too old | Git 2.34.1 is too old | Pensive needs Git 2.36 or newer, and `<path>` is 2.34.1. Then the fix |

The fix for each platform, from [git-scm.com](https://git-scm.com/install/linux)'s install pages:

| Platform | Missing, or won't run | Too old |
|---|---|---|
| macOS | Run `xcode-select --install` for Apple's Git, or `brew install git` | Run `brew install git`, or update the command line tools |
| Windows | Get Git for Windows at https://git-scm.com/install/windows, or run `winget install --id Git.Git -e --source winget` | Get the latest at https://git-scm.com/install/windows, or run `git update-git-for-windows` |
| Linux and the BSDs | Install it with your package manager, such as `sudo apt install git`. See https://git-scm.com/install/linux for other systems | On Ubuntu, run `sudo add-apt-repository ppa:git-core/ppa`, then `sudo apt update && sudo apt install git`. See https://git-scm.com/install/linux for other systems |

### A proposal that needs a design

A notice's detail is plain text, so the user can't click the link or copy the command, and the toast folds away after a few seconds over an app that can't open anything. A notice undersells the problem. A better experience is a screen in place of the project view: the version found, the command with a copy button, the link, and a button to look again. That needs a design and is not built.

## Bundling

Zed and GitHub Desktop both bundle Git:

- **Zed** on macOS ships Git from GitHub's [dugite-native](https://github.com/desktop/dugite-native) inside the app. At v1.27.0, `script/bundle-mac` pins dugite-native v2.43.3, thirteen minor releases behind Git 2.56.1. Zed runs the Git on the project's `PATH` first and the bundled one only when there's none. Since [March 2025](https://github.com/zed-industries/zed/pull/25900) push, pull and fetch always use the system Git, because the bundled copy lacks what HTTPS needs.
- **GitHub Desktop** ships dugite-native on macOS and Windows and runs it instead of the system's Git.

A bundled Git is only as current as the app's last release. Git for Windows had two high-severity advisories in 2026, both CVSS 7.4 and both leaking the user's NTLM hash to a hostile server: [GHSA-9j5h-h4m7-85hx](https://github.com/git-for-windows/git/security/advisories/GHSA-9j5h-h4m7-85hx) through `git clone`, fixed in 2.53.0.windows.3 and published in April, and [GHSA-xrpg-8j9v-v282](https://github.com/git-for-windows/git/security/advisories/GHSA-xrpg-8j9v-v282) through server-advertised bundle URIs, fixed in 2.53.0.windows.4, 2.54.0.windows.3 and 2.55.0.windows.4 in August. Each would have needed a Pensive release, and every user would have had to update Pensive. A system Git gets the same fix from the package manager or from Git for Windows' own updater. Pensive doesn't fetch or clone yet, but it will, and that's where both advisories hit.

The recommendation, for when installers exist:

- **Linux.** Don't bundle. Every distribution packages Git, and the PPA covers Ubuntu 22.04.
- **macOS.** Don't bundle. `/usr/bin/git` already offers Apple's install prompt, and the command line tools ship a Git well past 2.36.
- **Windows.** Don't bundle. It's the one platform with no Git by default, so the installer should look for one and offer to install Git for Windows, through winget or its own installer. Git for Windows then updates itself.

If bundling comes back, for a portable build say, the bundled Git goes last in the list of places Pensive looks, so a system Git that's new enough still wins and the user's credential helpers, SSH setup and LFS keep working. It needs auto-update first, and a Pensive release for every Git security release. Git for Windows' MinGit, a minimal build for other apps to carry, would be the one to ship.

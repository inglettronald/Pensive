# Git backend

How Pensive talks to Git. Decided in PEN-6, with facts checked in November 2026.

## Recommendation

Run the `git` command line for everything, and link no Git library. Pensive asks Git for refs, trees and file contents in machine-readable formats, and computes diffs itself.

Look at gix again as a fast path for reads only when a profile shows the CLI is the bottleneck, and only once gix can open reftable repositories.

## What Pensive needs

The command palette needs, per project:

- Worktrees, each with its checked-out branch and its last change
- Branches with the time of their last commit
- Opening a folder, cloning from a URL, creating and switching branches, merging

These fill `Worktree` and `Branch` in `model::Projects`. The palette also lists fetch, pull, push, commit and stash, so the backend needs the network and the user's credentials.

Pensive is review-first, so most later work is reads: status, diffs between any two trees, blame, history, and previewing a merge before running it.

## Options

### git2 (libgit2)

Rust bindings to libgit2, a C library. git2 0.21.0 (May 2026) bundles libgit2 1.9.7.

- libgit2 1.9 can't read repositories that store refs as reftable. Git 3.0 will make reftable the default for new repositories. libgit2 merged reftable support in May 2026 for its 2.0 release, which reached its first release candidate in October. git2 hasn't moved to it.
- SSH goes through libssh2, which doesn't read `~/.ssh/config`. Pensive would write the credential callback that chooses between helpers, agents and key files.
- Builds C with `cc`. HTTPS needs OpenSSL on Linux.
- Calls block, and a `Repository` can't be shared between threads. Only network calls can be cancelled, through their progress callbacks.
- libgit2 shipped three security releases in the past year, two of them for repository paths its SSH transports didn't escape.

### gix (gitoxide)

A Git implementation in pure Rust. gix 0.90.0 (October 2026), released about every three weeks, with breaking changes as 0.x allows.

- Strong at reads: status, tree diffs with rename tracking, blame and tree merges.
- No push. The tracking issue was [closed as not planned](https://github.com/GitoxideLabs/gitoxide/issues/306) in July 2026.
- Checkout, switch and reset of a worktree "still need plumbing", per its [status page](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md). The same page says its blame isn't yet competitive with Git's on speed.
- Can't open reftable repositories. gix-ref 0.68 only explains the failure better.
- Builds as plain Rust on every platform.

gix alone can't switch a branch, merge into a worktree or push, so it can't back the palette today.

### The `git` CLI

Spawn `git` and parse its output.

- Opens every repository the user's Git can, with reftable, sparse checkout, partial clone, LFS, hooks and commit signing working as the user configured them.
- Credentials come from the user's own setup: credential helpers, `~/.ssh/config` and SSH agents. Pensive sets `GIT_TERMINAL_PROMPT=0` so Git never waits on a terminal, and points `GIT_ASKPASS` and `SSH_ASKPASS` at itself to ask for passwords in the app.
- Each call spawns a process, which costs milliseconds, more on Windows. A long-running `git cat-file --batch` streams any number of objects through one process.
- Output is parsed, so every call uses a stable format: `--porcelain`, `-z` or `--format`.
- The user must have Git installed.
- A child process is asynchronous already: a task on GPUI's background executor awaits its output, and cancelling means killing the child.

Zed, which we pin at v1.27.0, [removed libgit2](https://github.com/zed-industries/zed/pull/53453) in June 2026 and now runs the CLI for everything. Their reasons were reftable support and dropping about 30,000 lines of vendored C. GitKraken started the [same move](https://gitkraken.com/blog/gitkraken-client-migrating-from-libgit2-to-git-executable) in 2023 because libgit2 trails Git's features. Zed's `git` crate is GPL-3.0, so we can learn from it but not copy it into MPL-2.0 Pensive.

### Hybrid

The CLI for writes and the network, with gix for hot reads. That means two code paths and two sets of bugs, and gix still needs a CLI fallback for reftable repositories. Not worth it before a profile says so.

## Comparison

| | git2 | gix | `git` CLI |
|---|---|---|---|
| Worktrees | List, add, prune | List, add | Everything |
| Merge into a worktree | Yes | Trees only | Yes |
| Clone and fetch | Yes | Yes | Yes |
| Push | Yes | No | Yes |
| SSH and credentials | Pensive's callback, no `~/.ssh/config` | Helpers, the `ssh` program | The user's setup |
| Reftable repositories | Not until libgit2 2.0 | No | Yes |
| Off the UI thread | Blocking, one handle per thread | Blocking, shareable | Async child processes |
| Build | C compiler, OpenSSL on Linux | Rust only | Nothing, but Git must be installed |
| License | MIT or Apache-2.0, libgit2 GPLv2 with linking exception | MIT or Apache-2.0 | GPLv2, run as a separate program |
| Maturity | Stable, slow to follow Git | 0.x, frequent breaking changes | The reference |

## How it fits

- `model` defines the `GitBackend` trait. Its Fake answers from fixtures, as the glossary plans.
- The CLI backend lives in its own crate. Only `pensive` depends on it, as the architecture requires.
- Every call runs on GPUI's background executor, and its result refreshes `Projects`. Views never wait on Git.
- The minimum Git version is the oldest that has every command Pensive runs, and it rises when a feature needs more. Today that's 2.36, for `git worktree list -z`. Merge preview will raise it to 2.38, for `git merge-tree --write-tree`, which computes a merge without touching the worktree.
- Pensive requires Git for now and looks it up in one place. PEN-29 covers a missing or outdated Git, and whether to bundle one as a fallback.
- Hosting services such as GitHub get their own backend, decided in PEN-30. It holds pull requests, comments and CI status. Git still does every diff, blame and merge, on the fetched pull request.

## Diffs

Git says which files changed, and Pensive diffs their contents.

- `git diff --name-status -M -z` lists changed files with renames. `git check-attr` reports which ones `.gitattributes` marks as binary.
- For each text file, the backend returns both versions, and Pensive diffs them in process with a crate such as imara-diff, which Zed and gix both use.

Parsing `git diff` would match the terminal exactly and honor the user's diff settings, but costs more for a review client:

- Highlighting needs the whole file, since a highlighter can't color a hunk without the code around it. Once Pensive holds both files, Git's hunks repeat what it already has.
- Word highlights need a second `git diff --word-diff` run. Moved lines only come from `--color-moved`, which exists only in colored output.
- Showing more context needs another call.

The cost of our own diff: hunks can differ slightly from what `git diff` prints, and `textconv` drivers don't apply. imara-diff has Git's histogram algorithm, so differences should be rare. Staging part of a file still works: Pensive writes a patch from its own diff and pipes it to `git apply --cached`.

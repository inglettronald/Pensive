# Git backend

Decided in PEN-6. Covers how Pensive talks to Git, how it will diff files, the oldest Git it supports, and where hosting services fit. Facts were checked on the web in November 2026.

Pensive runs the `git` command line for everything and links no Git library. Git hands over refs, file contents and merge results in machine-readable formats. Pensive will diff file contents itself.

## Why not a library

**git2**, the Rust bindings to libgit2, can't open repositories that store refs as reftable, which Git 3.0 will make the default for new repositories. libgit2 merged reftable support in May 2026 for its 2.0 release, which reached its first release candidate in October. git2 0.21 still bundles libgit2 1.9. libgit2's SSH goes through libssh2, which ignores `~/.ssh/config`. libgit2 also shipped three security releases in the past year.

**gix**, from gitoxide, is a pure Rust rewrite of Git. It reads quickly, but it can't push: the tracking issue was [closed as not planned](https://github.com/GitoxideLabs/gitoxide/issues/306) in July 2026. Its [status page](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md) says checkout, switch and reset of a worktree still need work. It can't open reftable repositories either.

**The CLI** opens every repository the user's Git can, and credential helpers, `~/.ssh/config`, hooks, signing and LFS work as the user set them up. Zed, which we pin at v1.27.0, [removed libgit2](https://github.com/zed-industries/zed/pull/53453) in June 2026 for reftable support and to drop about 30,000 lines of vendored C. GitKraken started the [same move](https://gitkraken.com/blog/gitkraken-client-migrating-from-libgit2-to-git-executable) in 2023. Zed's `git` crate is GPL-3.0, so we learn from it but don't copy it.

The CLI's costs:

- Each call spawns a process, which costs milliseconds, more on Windows. Once Pensive reads many objects at a time, for diffs and blame, one long-running `git cat-file --batch` will stream them all.
- Output is parsed, so every call asks for a stable format: `--porcelain`, `-z` or `--format`.
- The user must have Git installed. [find-git.md](find-git.md) covers a missing or outdated Git, and why Pensive doesn't bundle one.

A hybrid, with gix for hot reads, means two code paths, and gix would still need the CLI for reftable repositories. Look at it again only when a profile shows the CLI is the bottleneck, and only once gix opens reftable repositories.

## Diffs

Git will say which files changed, and Pensive will diff their contents.

- `git diff --name-status -M -z` lists changed files with renames.
- A file is binary if `git check-attr` says `.gitattributes` marks it so, or if its first 8,000 bytes hold a NUL byte, the same test Git uses.
- For each text file, the backend will return both versions, and Pensive will diff them in process with a crate such as imara-diff, which Zed and gix both use.

Both versions go through Git's filters first. Without them, LFS pointers, `core.autocrlf` line endings and clean and smudge filters would show as whole-file changes. Both versions are compared as checkout writes them: the committed side comes from `git cat-file --filters`, and the worktree side is read as it is. That's the form the editor saves, so a diff never shows filter output as a change. Staging part of a file will need the clean form instead, since `git apply --cached` takes a patch against the index.

Parsing `git diff` would match the terminal exactly and honor the user's diff settings, but it costs more for a review client:

- Highlighting needs the whole file, since a highlighter can't color a hunk without the code around it. Once Pensive holds both files, Git's hunks repeat what it already has.
- Word highlights need a second `git diff --word-diff` run. Moved lines only come from `--color-moved`, which exists only in colored output.
- Showing more context needs another call.

The cost of our own diff: hunks can differ slightly from what `git diff` prints, and `textconv` drivers don't apply. imara-diff has Git's histogram algorithm, so differences should be rare. Staging part of a file will still work: Pensive writes a patch from its own diff of the clean content and pipes it to `git apply --cached`.

## Minimum Git version

The oldest version that has every command Pensive runs, raised when a feature needs more. Today that's 2.36, for `git worktree list -z` and `git ls-tree --format`. [find-git.md](find-git.md) lists each command with the release that added it. Merge preview will raise it to 2.38, for `git merge-tree --write-tree`, which computes a merge without touching the worktree.

## Hosting services

GitHub and other hosting services will get their own backend, to be decided in PEN-30. It will hold pull requests, comments and CI status. Git will still do every diff, blame and merge, on the fetched pull request.

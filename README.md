# Pensive

A code editor and Git client for people who spend their day reading code. Written in Rust with [GPUI](https://www.gpui.rs).

Pensive is in early development, and there's nothing to download yet.

## Why

Most of my day goes to reading code. I review a branch, check what an agent changed in its worktree, or trace a function back through its history. The tools I have each get part of that right.

IntelliJ comes closest. Its index understands the code, it maps old commits onto the current checkout, and its Git tools are the best I've used. It's also slow. Indexing is meant to run in the background, but the indexing is heavy - with no tuning options - and somehow stalls the main thread. Switch branches or worktrees, and you're starting all over.

Zed is fast and never makes you wait. Its Git support is thin, though, and code navigation stops the moment you open an old commit. History turns into plain text.

Agent apps show their changes as a flat diff. From there you can't jump to a definition, find a function's callers or see how a file got this way.

## What Pensive is for

Working with agents means working in parallel, across several worktrees, branches and often projects, switching between them all day. Pensive is built so that every switch is instant. Opening a file, jumping to a commit and moving to another worktree never wait on an indexer.

When you get there, the context is already on screen: what changed, the history of the line you're on, where a function is defined and who calls it. That holds on any commit, not only the one checked out. The slow work happens in the background, and the interface never waits for it.

Pensive also recognizes that indexing isn't binary, and strives to give users more configuration on what their LSP or custom indexer is actually doing. This means less memory, prioritized indexing, and the experience you choose.

## How it feels

The base is monochrome, and color shows up only when it means something, like an added line or a failed run. Git actions sit next to the changes they act on instead of in a menu. One palette, on Ctrl K, reaches every project, branch, command and setting. Any choice that comes down to taste is a setting, and each one ships with a default worth keeping.

## Download

Not available yet.

## Building from source

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Documentation

- [Architecture](docs/architecture.md): how the code is organized
- [Design](docs/design.md): how Pensive looks and why
- [Glossary](docs/glossary.md): what terms like view, component and mock mean here

## License

[MPL-2.0](LICENSE)

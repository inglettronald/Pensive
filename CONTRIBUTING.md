# Contributing to Pensive

## Setting up your machine

You need four things: Rust, a few system libraries, Git 2.36 or newer, and the source code. After the first build, `cargo run` is all you need.

On Debian or Ubuntu, `script/bootstrap` does steps 1 and 2 for you.

### 1. Install Rust

Follow the instructions at [rustup.rs](https://rustup.rs). This installs `rustup`, which manages Rust versions for you. You don't need to pick a version: the first time you build Pensive, rustup reads `rust-toolchain.toml` and downloads the exact version this project uses.

### 2. Install system libraries

Pensive draws its interface with [GPUI](https://www.gpui.rs), which talks to your operating system's windowing, font and graphics libraries. Rust can't download those, so your package manager installs them.

**Debian / Ubuntu**

```sh
sudo apt install build-essential libfontconfig-dev libxkbcommon-x11-dev mesa-vulkan-drivers
```

| Package | Why |
|---|---|
| `build-essential` | C compiler and linker. Rust doesn't include one on Linux. It uses the system's to link the final program, and a few dependencies compile small C pieces |
| `libfontconfig-dev` | Finds the fonts installed on your system, linked into Pensive at build time |
| `libxkbcommon-x11-dev` | Keyboard layout handling, linked into Pensive at build time. It also installs `libxcb1-dev`, the X11 connection library GPUI links against |
| `mesa-vulkan-drivers` | Vulkan drivers, including a software one, so mocks render on machines without a GPU, such as CI |

Pensive also loads the Wayland or X11 and Vulkan libraries when it starts. Any desktop Linux install already has them.

**macOS**

Install Xcode from the App Store, open it once to finish setup, then run `xcode-select --install`.

### 3. Build and run

```sh
git clone https://github.com/inglettronald/Pensive
cd Pensive
cargo run
```

The first build takes a few minutes while it downloads and compiles dependencies. Later builds only recompile what changed.

`cargo run <path>...` opens the repositories at those paths. Until Pensive remembers projects, that's the only way to open one.

`cargo run mocked` starts the app with sample projects instead, so you can click around views without real repositories. It only works in debug builds.

## Where everything comes from

| Piece | What it is | Installed by |
|---|---|---|
| rustup | Installs and switches Rust versions | You, once (step 1) |
| Rust toolchain: `rustc`, `cargo`, `rustfmt`, `clippy` | Compiler, build tool, formatter, linter | rustup, automatically, at the version in `rust-toolchain.toml` |
| Rust libraries such as `gpui` | Code Pensive depends on | cargo, automatically, from [crates.io](https://crates.io) or the Zed Git repo at the versions in `Cargo.lock` |
| System libraries | Your OS's desktop and graphics stack | Your package manager (step 2) |
| Git, 2.36 or newer | Runs every Git command Pensive makes, so it must be on your `PATH` | You, or your package manager. macOS includes it with Xcode |

## Inspecting dependencies in a debugger

Debug builds keep only line tables for dependencies such as gpui. Backtraces show the file and line of every frame, and a debugger can still break on a line in gpui and step through it. What it can't show is gpui's local variables and types. Once tests are built, this saves about 3 GB in `target/`.

To inspect gpui's variables in a debugger, build with full debug info for every dependency:

```sh
cargo build --profile debug-deps
```

Then point your debugger at `target/debug-deps/pensive` (`pensive.exe` on Windows). The profile builds into its own directory, so switching back to a plain `cargo build` doesn't rebuild anything.

## Before opening a pull request

CI rejects formatting differences and any compiler or clippy warning. To run the same checks locally:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

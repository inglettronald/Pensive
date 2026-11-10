# Contributing to Pensive

## Setting up your machine

You need three things: Rust, a few system libraries, and the source code. After the first build, `cargo run` is all you need.

On Debian or Ubuntu, `script/bootstrap` does steps 1 and 2 for you.

### 1. Install Rust

Follow the instructions at [rustup.rs](https://rustup.rs). This installs `rustup`, which manages Rust versions for you. You don't need to pick a version: the first time you build Pensive, rustup reads `rust-toolchain.toml` and downloads the exact version this project uses.

### 2. Install system libraries

Pensive draws its interface with [GPUI](https://www.gpui.rs), which talks to your operating system's windowing, font and graphics libraries. Rust can't download those, so your package manager installs them.

**Debian / Ubuntu**

```sh
sudo apt install build-essential libfontconfig-dev libxkbcommon-x11-dev
```

| Package | Why |
|---|---|
| `build-essential` | C compiler and linker. Rust doesn't include one on Linux. It uses the system's to link the final program, and a few dependencies compile small C pieces |
| `libfontconfig-dev` | Finds the fonts installed on your system, linked into Pensive at build time |
| `libxkbcommon-x11-dev` | Keyboard layout handling, linked into Pensive at build time. It also installs `libxcb1-dev`, the X11 connection library GPUI links against |

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

## Where everything comes from

| Piece | What it is | Installed by |
|---|---|---|
| rustup | Installs and switches Rust versions | You, once (step 1) |
| Rust toolchain: `rustc`, `cargo`, `rustfmt`, `clippy` | Compiler, build tool, formatter, linter | rustup, automatically, at the version in `rust-toolchain.toml` |
| Rust libraries such as `gpui` | Code Pensive depends on | cargo, automatically, from [crates.io](https://crates.io) or the Zed Git repo at the versions in `Cargo.lock` |
| System libraries | Your OS's desktop and graphics stack | Your package manager (step 2) |

## Before opening a pull request

CI rejects formatting differences and any compiler or clippy warning. To run the same checks locally:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

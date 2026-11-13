# Architecture

Pensive is a Cargo workspace. A crate can only use the crates it lists as dependencies, so the compiler enforces the layering below.

## Crates

| Crate | Job | Depends on |
|---|---|---|
| `pensive` | The app. Opens the window and gives each model its backend, which differ between the desktop and the browser. | `ui`, `model`, `git` and `config` on the desktop, `settings`, `theme`, `assets` |
| `ui` | Views: screens and panels that read models and own their view state. | `components`, `model`, `settings`, `theme` |
| `components` | Stateless building blocks that render from props and the theme. | `theme` |
| `model` | Application state, and the traits that I/O backends implement. | |
| `git` | The Git backend, which runs the `git` command line. | `model` |
| `config` | The config folder backend, which reads, writes and watches the settings and theme files. | `model` |
| `settings` | Declared settings, and the store that holds the settings file's text and the values in it. It does no I/O. | |
| `theme` | Color, type and spacing roles, with user themes and the appearance settings. | `settings` |
| `mock` | Named mock states for views, rendered without a window. | `theme`, `assets`, `model` |
| `assets` | Files bundled into the app, such as icons, served to GPUI by path. | |

Every crate except `git` and `config` also depends on `gpui`.

## Rules

- Views can trigger I/O but never perform it. Git, the filesystem, the clock, the network, environment variables and randomness go through a model. That way any view can render from mock data.
- Each crate declares the settings it reads, and code reads and changes a setting through its declaration.
- Components never see models. They render from the props they're given.
- Backends implement traits defined in `model`. `ui` never depends on a backend crate, so only `pensive` sees both sides.
- A model holds its backend inside a `model::Backend`. Its `call` runs on GPUI's background executor, since backend calls block, and reports a failure to the notices, so no caller can drop the error. The failure is still returned, for a view that also shows it on the control that started the call.
- A new component ships with its gallery. A new view ships with mocks for the states that change the screen, usually empty, loading, error and a typical case.
- Every visual state of a component can be set through its props, so a mock can show it without input.

## Mocks

Each view declares named mocks next to its code: the view built from fixed data. They sit behind a `mocks` feature on `ui` and `components`, so they never ship in the app. `pensive` has the feature too, for the mock browser, and `cargo mocked` turns it on.

`cargo mocks` renders every mock to `target/mocks/<subject>/<mock>.png` without opening a window. `cargo mocks root` renders only the mocks whose `<subject>/<mock>` contains `root`. A subject with more than one mock also gets an atlas at `target/mocks/<subject>.png`, with all of its mocks in one numbered grid. The atlas is only written when the filter matches every mock of the subject, so `cargo mocks root/landing` deletes `root.png` instead of leaving one that's out of date. `cargo test` renders them too, so CI checks that every mock renders on Linux and macOS. GitHub's Windows runner can't render them yet (PEN-24). On Windows, mocks render with wgpu rather than the app's DirectX 11 renderer, so fine details of text can differ from the running app.

A mock fails to render when it asks a fake backend for something the fixtures don't have, and the error names what was missing. Add the sample to `model::fixtures`, or mark a mock that shows such a failure on purpose with `expects_fixture_gaps`.

## GPUI

crates.io stops at gpui 0.2.2, which can't render headless. We pin `gpui` and `gpui_platform` to a Zed release tag in the root `Cargo.toml`. To upgrade, change both tags together.

## The browser build

`pensive` also builds for `wasm32-unknown-unknown`, where `gpui_web` draws the window to a canvas. A browser has no `git` command or config folder, so `git` and `config` are desktop-only dependencies. In their place the app opens the sample projects and keeps settings in memory. It runs single-threaded, because threads in wasm need a nightly toolchain, so `gpui_platform`'s default features are off.

# Architecture

Pensive is a Cargo workspace. A crate can only use the crates it lists as dependencies, so the compiler enforces the layering below.

## Crates

| Crate | Job | Depends on |
|---|---|---|
| `pensive` | The app. Opens the window and gives each model its backend, which differ between the desktop and the browser. | `ui`, `model`, `git` and `config` on the desktop, `settings`, `theme`, `assets` |
| `ui` | Views: screens and panels that read models and own their view state. | `assets`, `components`, `model`, `settings`, `theme` |
| `components` | Stateless building blocks that render from props and the theme. | `theme` |
| `model` | Application state, and the traits that I/O backends implement. | `text`, `language` |
| `text` | Immutable Rope snapshots, source stamps, validated byte coordinates and ordered edit records. No I/O or mutable document owner. | |
| `language` | Language catalog, provider selection and syntax tokens. Java syntax is portable, and desktop builds also include Tree-sitter grammars. | `text` |
| `git` | The Git backend, which runs the `git` command line. | `model` |
| `config` | The config folder backend, which reads, writes and watches the settings and theme files. | `model` |
| `settings` | Declared settings, and the store that holds the settings file's text and the values in it. It does no I/O. | |
| `theme` | Color, type and spacing roles, with user themes and the appearance settings. | `settings` |
| `mock` | Named mock states for views, rendered without a window. | `theme`, `assets` |
| `assets` | Files bundled into the app, such as icons, served to GPUI by path. | |

Every crate except `git`, `config`, `text` and `language` also depends on `gpui`.

`model::Document` alone owns mutable file text, undo, saving/conflicts and diff scheduling.
It publishes cheap `text::TextSnapshot` values from its existing Rope, with separate source
identities and revisions for the base and editable sides. The base also retains its existing
flat string for borrowed removed-line display; its immutable Rope is built when base contents
arrive, never on a snapshot request. Background diff publication checks both source stamps
and its request generation. Edits, grouped undo and redo, and disk reloads each publish
one revision with ordered changes in UTF-8 byte coordinates.

`GitBackend::versions` retains both original paths and resolved commit origins in
`FileVersions`; its fake obeys the same contract. `Document::source_snapshot` pairs each
side's existing text snapshot with that context and a selection from the shared language
registry. `Projects::set_languages` updates existing and future documents without changing
their text stamps. Explicit path, filename and extension associations take precedence
over bundled recognition. Language results also depend on source context and registry
configuration.

The editor submits visible source ranges to `Document` after the list lays out. Each
viewport has its own query budget and published spans. Queries run on GPUI's background
executor and share one parser per side. Providers reuse a completed full-file parse when
the source, provider selection and edit history match. The document notifies editors
when results arrive and releases a viewport's work when its editor detaches.

The editor composes each visible source row into a `DisplayLine`. Each styled part
either covers source bytes or displays text at a single source offset. Both diff sides
use this representation. Painting, hit testing, selection and input-method geometry use
the same byte mapping. A click inside anchored text resolves to its anchor, and the
caret at that offset sits before the text.

## Rules

- Views can trigger I/O but never perform it. Git, the filesystem, the clock, the network, environment variables and randomness go through a model. That way any view can render from mock data. Clippy enforces this: the root `clippy.toml` disallows the standard library's file, environment, process and clock calls, and `git`, `config` and `pensive` each have their own `clippy.toml` that drops those rules.
- Each crate declares the settings it reads, and code reads and changes a setting through its declaration.
- Components never see models. They render from the props they're given.
- Backends implement traits defined in `model`. `ui` never depends on a backend crate, so only `pensive` sees both sides. Each backend's tests, the fakes' included, run the checks in `model::fixtures::contract`, so a fake keeps the promises its trait makes.
- A model holds its backend inside a `model::Backend`. Its `call` runs on GPUI's background executor, since backend calls block, and reports a failure to the notices, so no caller can drop the error. The failure is still returned, for a view that also shows it on the control that started the call.
- A new component ships with its gallery. A new view ships with mocks for the states that change the screen, usually empty, loading, error and a typical case.
- Every visual state of a component can be set through its props, so a mock can show it without input.
- Sizes are written with `theme::dp`, a pixel at 100% zoom that renders as rems, so the whole interface scales with the window's rem size. GPUI's spacing methods (`gap_1`, `p_3`, ...) are rems already. Clippy disallows `gpui::px` outside `pensive`. The few fixed sizes are `theme::HAIRLINE`, `theme::CARET` and `theme::Shadow`, and window sizes and pixel tests carry an `#[expect]` that says why.

## Mocks

Each view declares named mocks next to its code: the view built from fixed data. They sit behind a `mocks` feature on `ui` and `components`, so they never ship in the app. `pensive` has a `demo` feature that turns them on for the mock browser and opens the sample projects. `cargo demo` runs the app with it, and so does the browser build.

`cargo mocks` renders every mock to `target/mocks/<subject>/<mock>.png` without opening a window. `cargo mocks root` renders only the mocks whose `<subject>/<mock>` contains `root`. A subject with more than one mock also gets an atlas at `target/mocks/<subject>.png`, with all of its mocks in one numbered grid. One too large for the GPU to read back at once continues on `<subject>-2.png` and on. The atlas is only written when the filter matches every mock of the subject, so `cargo mocks root/landing` deletes `root.png` and its pages instead of leaving one that's out of date. Mocks of a whole window render at 1920 × 1080, the size of a maximized window on a 1080p screen. `cargo test` renders them too, so CI checks that every mock renders on Linux and macOS. A mock that fails or panics is reported by its id, and the others still render. `cargo test <name>` renders only the mocks whose id contains `<name>`, and passes if none do, as any other test binary would.

`cargo mocks --save-baseline` also copies each render to `target/mocks-baseline`. While that folder exists, `cargo mocks` lists every mock whose render differs from its copy, and writes a picture of the change to `target/mocks-diff`, with the changed pixels in magenta. Renders on one machine come out identical every run, so any difference is a real change. Baselines stay on your machine: renders differ between GPUs and operating systems. GitHub's Windows runner can't render them yet (PEN-24). On Windows, mocks render with wgpu rather than the app's DirectX 11 renderer, so fine details of text can differ from the running app.

`cargo mocks --matrix FILTER...` renders the window mocks that match at the window sizes people use, at 80% and 200% zoom in both densities, to `target/mocks-matrix/<subject>/<mock>/<cell>.png`. It doesn't run in CI. Mocks made with `mock_window::mock` are in it, and those made with `mock_window::fixed` aren't.

## GPUI

crates.io stops at gpui 0.2.2, which can't render headless. We pin `gpui` and `gpui_platform` to a Zed release tag in the root `Cargo.toml`. To upgrade, change both tags together.

## The browser build

`pensive` also builds for `wasm32-unknown-unknown`, where `gpui_web` draws the window to a canvas. A browser has no `git` command or config folder, so `git` and `config` are desktop-only dependencies. In their place the app opens the sample projects and keeps settings in memory. It runs single-threaded, because threads in wasm need a nightly toolchain, so `gpui_platform`'s default features are off.

The browser build is always a demo build, for trying Pensive and recording it, and never a release artifact. GPUI's test support doesn't build for wasm, so `mock` can't render mocks there, and each card in the mock browser says "Rendering" where the desktop shows a thumbnail.

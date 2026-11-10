# Architecture

Pensive is a Cargo workspace. A crate can only use the crates it lists as dependencies, so the compiler enforces the layering below.

## Crates

| Crate | Job | Depends on |
|---|---|---|
| `pensive` | The app. Opens the window and gives each model its backend. | `ui`, `model`, `theme` |
| `ui` | Views: screens and panels that read models and own their view state. | `components`, `model`, `theme` |
| `components` | Stateless building blocks that render from props and the theme. | `theme` |
| `model` | Application state, and the traits that I/O backends implement. | |
| `theme` | Color, type and spacing roles, with user overrides. | |
| `mock` | Named mock states for views, rendered without a window. | |

Every crate also depends on `gpui`.

## Rules

- Views can trigger I/O but never perform it. Git, the filesystem, the clock, the network, environment variables and randomness go through a model. That way any view can render from mock data.
- Components never see models. They render from the props they're given.
- Backends implement traits defined in `model`. `ui` never depends on a backend crate, so only `pensive` sees both sides.

## GPUI

crates.io stops at gpui 0.2.2, which can't render headless. We pin `gpui` and `gpui_platform` to a Zed release tag in the root `Cargo.toml`. To upgrade, change both tags together.

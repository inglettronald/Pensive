# Agent guide

Pensive is a review-first Git client written in Rust with GPUI.

## Read first

- [docs/glossary.md](docs/glossary.md): what view, component, mock and other terms mean here
- [docs/architecture.md](docs/architecture.md): the crates and the rules between them
- [docs/design.md](docs/design.md): visual decisions
- [CONTRIBUTING.md](CONTRIBUTING.md): setup, and the checks CI runs

## Checking your work

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` must pass. CI rejects any warning.
- To check UI, run `cargo mocks <filter>` and open the PNGs it prints. It renders without a window or display, so use it instead of launching the app.

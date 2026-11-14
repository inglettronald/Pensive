# Agent guide

Pensive is a review-first Git client written in Rust with GPUI.

## Read first

- [docs/glossary.md](docs/glossary.md): what view, component, mock and other terms mean here
- [docs/architecture.md](docs/architecture.md): the crates and the rules between them
- [docs/design.md](docs/design.md): visual decisions
- [CONTRIBUTING.md](CONTRIBUTING.md): setup, and the checks CI runs
- [docs/decisions/](docs/decisions/): one file per decision, with the options weighed and why

## Checking your work

- Run `script/check` before you finish. It runs the checks CI runs and prints one line for each, then the output of any that failed. CI rejects any warning.
- To check UI, run `cargo mocks <filter>` and open the PNGs it prints. It renders without a window or display, so use it instead of launching the app.
- Before changing UI, run `cargo mocks --save-baseline`. Each `cargo mocks` after that lists the mocks whose pixels changed, with a picture of each change in `target/mocks-diff`. Check that only the mocks you meant to change are listed.
- To click through the app, open it in the T3 preview with the `web-preview` skill. `cargo mocks` is still how to check what a view looks like.

## Skills

Repo skills live in `.agents/skills/`, and each one is linked from `.claude/skills/`. When adding a skill, turn off automatic invocation for both harnesses unless asked for one that runs on its own: `disable-model-invocation: true` in the frontmatter, and `policy.allow_implicit_invocation: false` in `agents/openai.yaml`.

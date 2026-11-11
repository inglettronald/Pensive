# Design

How Pensive looks and why. This doc records decisions, not values. Exact colors, fonts and sizes live in the `theme` crate, and `cargo mocks theme` renders all of them. Boards that aren't built yet, such as diffs and project icons, are on the [design canvas](https://claude.ai/artifact/39s8kb64HfEHWMni3pPhTJ).

## Principles

- The base is monochrome: twelve neutral grays with no tint.
- Color only appears when it means something. Three hues describe code changes, and one blue marks interface signals.
- The reader's syntax colors come first. Diff markers must not wash them out.
- Visual choices that come down to taste are settings, and each one ships with an opinionated default.

## Color

Pensive is dark by choice and ships no light theme. Anyone who wants one can build it as a user theme.

Components read roles, never palette steps. Roles are grouped as surfaces, borders, text, controls, diff and accent.

Disabled text and unchanged line numbers fall below the 4.5:1 contrast minimum on purpose. Disabled text is exempt by convention, and quiet line numbers let the colored numbers on changed lines stand out.

### Meaning hues

- Added (green): added lines and words, file status A, success
- Removed (red): removed lines and words, file status D, conflicts, destructive actions
- Modified (amber): file status M, moved lines, warnings

### Accent

One blue for interface signals: info callouts, links and the accent button.

### Themes and overrides

A user theme overrides individual roles and fonts in a TOML file, keyed by the role names in the `theme` crate. Where Pensive reads that file from isn't decided yet.

## Type

A UI font and a code font, both bundled with the app so every machine renders the same: Inter and JetBrains Mono by default. Geist and Geist Mono are bundled too. Users can override either.

## Density

Calm and Dense both ship, as a setting, with Calm as the default. Density changes sizes only. Fonts and colors come from the theme.

## Diffs

Default:

- An edge bar between the line numbers and the code
- One column of new-file line numbers, colored on changed lines
- No gutter tint
- A faint fill on changed lines and a stronger highlight on changed words

Settings: change marker (edge bar or tinted lines), bar width, line numbers (off, new file, both), number color, gutter tint, line fill, changed-word highlight. One shortcut cycles through the line-number modes.

## Navigation

Breadcrumbs separate levels with a chevron, never a slash. They are used for project › branch and for file paths.

## Project icons

The icon is shown whenever a project is open. First match wins:

1. The user's setting for that project
2. The repo's `.pensive/project.toml`
3. A letter tile from the repo name

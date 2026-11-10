# Design

How Pensive looks and why. The [design canvas](https://claude.ai/artifact/39s8kb64HfEHWMni3pPhTJ) is the visual reference. This doc records the decisions behind it. Once the theme crate exists, exact colors and sizes live there, not here.

## Principles

- The base is monochrome: twelve neutral grays with no tint.
- Color only appears when it means something. Three hues describe code changes, and one blue marks interface signals.
- The reader's syntax colors come first. Diff markers must not wash them out.
- Visual choices that come down to taste are settings, and each one ships with an opinionated default.

## Color

### Palette

Gray 1 (darkest) to gray 12 (lightest). Dark mode first. Light mode is derived once the roles are locked.

### Roles

Components read roles, never palette steps. TODO: role table (role, gray step, used for), grouped as:

- Surfaces: app, panel, raised, selected
- Borders: subtle, default, focus
- Text: strong, default, muted, faint, disabled
- Controls: primary and secondary buttons
- Diff: `diff.line_number` sits at gray 8 on purpose. Line numbers are rarely read, so they are exempt from the 4.5:1 contrast rule, like disabled text.

### Meaning hues

- Added (green): added lines and words, file status A, success
- Removed (red): removed lines and words, file status D, conflicts, destructive actions
- Modified (amber): file status M, moved lines, warnings

### Accent

One blue for interface signals: info callouts, links and the accent button.

### Themes and overrides

A user theme extends a base theme and overrides individual roles and fonts. File format TBD.

## Type

A UI font and a code font, both bundled with the app so every machine renders the same. Users can override either in settings.

TODO: type scale (label, small, body, title, heading, code).

## Spacing, radii and density

- Spacing scale: 2, 4, 6, 8, 12, 16, 24, 32 px
- Radii: none, sm, md, lg

TODO: row heights and diff line height for the chosen density.

## Diffs

Default:

- A 4 px edge bar between the line numbers and the code
- One column of new-file line numbers, colored on changed lines
- No gutter tint
- Line fill at 3.5%, changed-word highlight at 16%

Settings: change marker (edge bar or tinted lines), bar width (2, 3, 4, 6 px or a tab in the +/− column), line numbers (off, new file, both), number color, gutter tint, line fill, changed-word highlight. One shortcut cycles through the line-number modes.

## Navigation

Breadcrumbs separate levels with a chevron, never a slash. They are used for project › branch and for file paths.

## Project icons

The icon is shown whenever a project is open. First match wins:

1. The user's setting for that project
2. The repo's `.pensive/project.toml`
3. A letter tile from the repo name
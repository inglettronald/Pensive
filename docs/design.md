# Design

How Pensive looks and why. This doc records decisions, not values. Exact colors, fonts and sizes live in the `theme` crate, and `cargo mocks theme` renders all of them.

To draft a design, run the `design` skill. It draws boards as HTML pages that use the theme's tokens and shows them inline in the chat. Drafts are throwaway: once a design is built, its mocks are the record. The [old design canvas](https://claude.ai/artifact/39s8kb64HfEHWMni3pPhTJ) is deprecated, but it still holds the only drafts of diffs and project icons.

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

A tooltip that reports how an action went leads with an icon and tints its outline toward the hue: green for success, amber for a warning, red for an error. A tag after its title says where a problem came from: UI, Git, Config or Fixture. Fixture is blue, so a gap in the sample data never reads as a bug in the interface. A control with nothing behind it yet answers with an amber notice tagged UI.

### Accent

One blue for interface signals: info callouts, links and the accent button.

### Themes and overrides

A user theme overrides individual roles and fonts in a TOML file, keyed by the role names in the `theme` crate. [Settings storage](decisions/settings-storage.md) covers where theme files live and how one extends another.

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

Navigation sits on the top and left edges only. Nothing docks on the right, so content is never boxed in from both sides.

Projects switch from the title bar or from the command palette. There is no project rail by default, because many people only ever work in one or two projects.

Breadcrumbs separate levels with a chevron, never a slash. They are used for project › branch and for file paths.

## Window

The app owns the projects, Settings and the theme. Each window has its own command palette and shows one project. Switching projects replaces the one shown, and Ctrl Enter in the palette opens one in a new window instead.

A window has a title bar, the sidebar on the left and the main view. There is no status bar.

Pensive draws its own title bar. It holds project › branch as a breadcrumb, the palette trigger in the middle, the settings gear and the window controls. Clicking a crumb opens the palette scoped with `project:` or `branch:`. macOS keeps its own window buttons on the left. So does a Linux compositor that decorates windows itself, and Pensive's title bar then sits under the compositor's.

Overlays such as tooltips draw above every pane, so no pane clips them.

## Sidebar

The sidebar is the current branch as a timeline, newest at the top: uncommitted work, the commits that aren't pushed, then the pushed ones. A HEAD marker sits where committed work starts and an origin marker where pushed work starts. The line is dashed along uncommitted work and fades out below the oldest commit.

Git actions live in the sidebar, next to the changes they act on. Fetch, pull and push sit in its header. Pull and push show an arrow and a count, and hide at zero. The commit box is pinned to the bottom, so it is always on screen. The Commit button is outlined rather than filled, so it doesn't outshine the code, and its Ctrl Enter hint sits inside it.

The Uncommitted node stages or unstages everything, and fills like a file's ring: empty, half or full. An arrow beside it folds the file list.

A commit title fades out at the sidebar's edge instead of ending in an ellipsis. A small chip after it marks a commit with a description. Opening a commit shows its description, then its files. Its node gets a ring, so the selected background only ever marks the file shown in the main view.

## Changed files

Files show as a list, a tree or by symbol, switched by an icon control on the Uncommitted node. The choice applies to an open commit too.

A file's ring takes its status color and fills when it's staged, or half fills when only some of its hunks are. A commit's files are always filled. Clicking a ring stages the whole file, or unstages it once it's fully staged.

Every file row shows its added and removed line counts. The folder isn't on the row. Hovering the file shows its full path in a tooltip, and Ctrl Shift C copies it while the tooltip is open, leaving plain Ctrl C alone. The tooltip then says Copied path for a second, outlined in green.

The tree draws a guide under each folder's chevron and folds a chain of single-child folders into one row. By symbol, each file lists the functions, types and headings its hunks touch, each with an icon and its own line counts.

## Command palette

Ctrl K opens the palette from anywhere. On the landing screen the palette is the whole page, so the same keys work before and after a project is open.

Three tabs sit in the search field: All, Active and Commands. Tab cycles through them. The set is fixed so it stays short and learnable. Plugins extend the palette with keywords instead, like `project:`, `branch:` or `merge:`, which narrow the results to one scope.

Active lists the worktrees and branches that changed recently in any project, newest first. It is where you keep track of agent work.

Rows stay quiet. Fuzzy matches are the only accent color, and the right edge shows only the time since the last change unless a scope has something worth saying in color. Each entry labels its own Enter and Ctrl Enter actions in the footer.

A setting found in the palette carries its control inline, so it can be changed without leaving. Settings itself isn't listed until you search for it. The gear in the footer opens it at the palette's section.

Settings: whether the tabs show, which tab Ctrl K opens on, how far back Active looks, background opacity and scrim strength. Blur behind the palette waits on GPUI support.

## Project icons

The icon is shown whenever a project is open. First match wins:

1. The user's setting for that project
2. The repo's `.pensive/project.toml`
3. A letter tile from the repo name

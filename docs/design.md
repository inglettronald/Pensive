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

## Size

Calm and Dense both ship, as a setting, with Calm as the default. Density changes sizes only. Fonts and colors come from the theme.

Zoom scales everything in every window, from 80 to 200%. Ctrl = and Ctrl - step it, and Ctrl 0 resets it. Code size is its own setting, in pixels at 100% zoom, so zoom still scales it. It follows Density until it's set.

Zoom and code size use a stepper, which the Controls section covers along with how the wheel changes it.

## Diffs

Default:

- An edge bar between the line numbers and the code
- One column of new-file line numbers, colored on changed lines
- No gutter tint
- A faint fill on changed lines and a stronger highlight on changed words

Settings: change marker (edge bar or tinted lines), bar width, line numbers (off, new file, both), number color, gutter tint, line fill, changed-word highlight. One shortcut cycles through the line-number modes.

## Editor

The main view shows the selected file in an editor, in one of three modes picked by an icon switcher in its header. Changes shows each change with three lines around it and folds the rest. File shows the whole file with the diff marked. Plain shows it with no marks. The mode stays as files change. Removed lines show between the lines that replaced them and can't be edited. A commit's files use the same view, read-only.

A fold leads with buttons that show 20 more lines down from the change above, 20 more up from the change below, or all of them, then says how many lines it hides.

Saving is a setting. Auto, the default, saves a second after typing stops and when you leave the file or the window. Manual saves on Ctrl S. Unsaved edits put a dot and Unsaved after the file name, and a long path fades out at its start so the name always shows. If the file changed on disk since Pensive read it, saving stops and says so rather than overwrite it.

## Navigation

Navigation sits on the top and left edges only. Nothing docks on the right, so content is never boxed in from both sides.

A rail of icons runs down the window's left edge in every view. Its top holds the project's panels: Changes, Files, Search and History. The gear sits at its foot and opens Settings.

Projects switch from the title bar or from the command palette. There is no rail of projects by default, because many people only ever work in one or two projects.

Breadcrumbs separate levels with a chevron, never a slash. They are used for project › branch and for file paths.

## Window

The app owns the projects, Settings and the theme. Each window has its own command palette and shows one project. Switching projects replaces the one shown, and Ctrl Enter in the palette opens one in a new window instead.

The first window opens maximized. A window opened from it, with Ctrl Enter or Ctrl-click on the gear, opens windowed on the same display, so it doesn't hide the first.

A window has a title bar, the rail and its panel on the left, and the main view. There is no status bar.

Pensive draws its own title bar. It holds project › branch as a breadcrumb, the palette trigger in the middle, the notice badge and the window controls. Clicking a crumb opens the palette scoped with `project:` or `branch:`. macOS keeps its own window buttons on the left. So does a Linux compositor that decorates windows itself, and Pensive's title bar then sits under the compositor's.

Overlays such as tooltips draw above every pane, so no pane clips them.

## Changes panel

The Changes panel is the current branch as a timeline, newest at the top: uncommitted work, the commits that aren't pushed, then the pushed ones. A HEAD marker sits where committed work starts and an origin marker where pushed work starts. The line is dashed along uncommitted work and fades out below the oldest commit.

Git actions live in the panel, next to the changes they act on. Fetch, pull and push sit in its header. Pull and push show an arrow and a count, and hide at zero. The commit box is pinned to the bottom, so it is always on screen. The Commit button is outlined rather than filled, so it doesn't outshine the code, and its Ctrl Enter hint sits inside it.

The Uncommitted node stages or unstages everything, and fills like a file's ring: empty, half or full. An arrow beside it folds the file list.

A commit title fades out at the panel's edge instead of ending in an ellipsis. A small chip after it marks a commit with a description. Opening a commit shows its description, then its files. Its node gets a ring, so the selected background only ever marks the file shown in the main view.

## Changed files

Files show as a list, a tree or by symbol, switched by an icon control on the Uncommitted node. The choice applies to an open commit too.

A file's ring takes its status color and fills when it's staged, or half fills when only some of its hunks are. A commit's files are always filled. Clicking a ring stages the whole file, or unstages it once it's fully staged.

Every file row shows its added and removed line counts. The folder isn't on the row. Hovering the file shows its full path in a tooltip, and Ctrl Shift C copies it while the tooltip is open, leaving plain Ctrl C alone. The tooltip then says Copied path for a second, outlined in green.

The tree draws a guide under each folder's chevron and folds a chain of single-child folders into one row. By symbol, each file lists the functions, types and headings its hunks touch, each with an icon and its own line counts.

## Notices

A notice about a control shows in a tooltip on that control. One with no control behind it, such as a failed read or a bad settings file, shows as a toast at the top right, under the title bar.

Only errors and the successes of long work toast. Warnings never do, so a noisy source can't take over the screen. A success fades on its own. An error folds into the badge once its time runs out, three seconds by default and a setting. A line along its bottom drains until then, and hovering it holds it open.

The badge sits at the title bar's right end: an icon and a count for the errors held and for the warnings, outlined in the worse of the two. It's gone while nothing is held. Clicking it opens the list, errors first and newest first within each. A repeat counts on its row as ×3 instead of adding one. Each row dismisses on its own, and Clear all empties the list.

## Command palette

Ctrl K opens the palette from anywhere. On the landing screen the palette is the whole page, so the same keys work before and after a project is open.

Three tabs sit in the search field: All, Active and Commands. Tab cycles through them. The set is fixed so it stays short and learnable. Plugins extend the palette with keywords instead, like `project:`, `branch:` or `merge:`, which narrow the results to one scope.

Active lists the worktrees and branches that changed recently in any project, newest first. It is where you keep track of agent work.

Rows stay quiet. Fuzzy matches are the only accent color, and the right edge shows only the time since the last change unless a scope has something worth saying in color. Each entry labels its own Enter and Ctrl Enter actions in the footer.

A setting found in the palette carries its control inline, so it can be changed without leaving. Settings itself isn't listed until you search for it. The gear in the footer opens it at the palette's section.

Settings: whether the tabs show, which tab Ctrl K opens on, how far back Active looks, background opacity and scrim strength. Blur behind the palette waits on GPUI support.

## Settings

The gear, the palette's Settings command or Ctrl , opens Settings in place of the project, in the same window. Ctrl-click on the gear opens it in a new window, the way Ctrl Enter opens a project in one from the palette. Esc or the project crumb goes back.

In Settings, the rail shows the categories in place of the project's panels, and the gear stays at its foot, filled. The current category is filled and marked by a white bar, and hovering an icon names it. Beside the rail, the list shows the current category's pages on a guide line. The list is as wide as a project panel, so opening Settings reads as a change of view rather than a new place.

The page shows one category: its heading, then its pages, each a card of rows. The wheel scrolls through them and stops at the category's end. A page becomes current once its heading passes a line near the top, and the list's highlight slides to it. The heading counts as part of the first page. Clicking the list scrolls smoothly and puts that page at the top, and the page leaves room under its last page so every page can get there. Clicking the rail fades the page over to that category, which rises slightly as it comes in. Once the category's last setting is on screen with room under it, a card leading to the next category docks at the bottom, a gutter below that setting. Every category has a description, which its heading and that card show.

Rows stop at a reading width, so a control stays near its name in a wide window. A changed setting shows its default and a button that resets it. There's no third level of headings: a page with too many settings becomes two pages.

Search matches names, pages and descriptions. Matches in the category being read come first, then every other category's together. The rail dims categories with no match, and clicking a result shows it on its category's page.

A setting with a preview will get an eye button that opens a drawer under its row, one at a time, drawn with the real components.

## Controls

Settings controls are a row tall, so they follow the density.

- A dropdown has a minimum width, so the dropdowns in a card line up on their right edge. Its menu has a check on the current value.
- A segmented control gives every segment the widest label's width. The selection stretches to its new segment: the leading edge leaves first, then the trailing edge follows. Hovering another segment fills it lightly, and hovering the selection brightens it one step. Icon switchers in headers and the palette's tabs are a compact version of the same control.
- A stepper holds a number between minus and plus, and dims the end it can't go past.
- A switch keeps one outline on and off. Only the knob changes, from gray to white.
- Resting on a control with ordered values hands it the mouse wheel after a moment. A line fills under it while it waits, so scrolling past a card never changes a value. Down moves to the next value, and the ends don't wrap.

## Project icons

The icon is shown whenever a project is open. First match wins:

1. The user's setting for that project
2. The repo's `.pensive/project.toml`
3. A letter tile from the repo name

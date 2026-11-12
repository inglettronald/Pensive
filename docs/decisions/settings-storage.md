# Settings storage and theme files

Decided in PEN-15. Covers where Pensive keeps settings, themes and its own state on each platform, what format those files use, and how a user theme builds on the base theme. Crate versions were checked on crates.io in November 2026.

## Four kinds of file

Pensive reads four kinds of file, and they differ in who writes them and whether they travel between machines.

| File | Written by | Holds | Travels |
|---|---|---|---|
| `settings.toml` | The user, by hand or through the palette and settings window | Preferences: theme, density, fonts, palette options | Yes. People keep it in a dotfiles repo |
| `themes/<name>.toml` | The user | Color roles and fonts layered over a base theme | Yes |
| `projects.toml` | Pensive only | Recent projects, when each was last opened, and any icon the user picked for one | No. It's full of paths on this machine |
| `.pensive/project.toml` | A repo's maintainers | The project's icon, which everyone who clones the repo sees | With the repo |

The rule that splits settings from state is that settings never hold a path on this machine. Anything keyed by a project path is state, so it goes in `projects.toml`. That answers PEN-13: the recent projects list is state, not settings.

For PEN-14, a project's icon normally comes from the repo. Maintainers commit it in `.pensive/project.toml`, and everyone who clones the project gets it. A user who wants a different icon for one project on their machine picks one, and that choice is stored next to the project's recent entry in `projects.toml`. It wins over the repo's, as `docs/design.md` orders them.

## Where they live

| | Settings and themes | State |
|---|---|---|
| Linux | `$XDG_CONFIG_HOME/pensive`, usually `~/.config/pensive` | `$XDG_STATE_HOME/pensive`, usually `~/.local/state/pensive` |
| macOS | `~/.config/pensive` | `~/.local/state/pensive` |
| Windows | `%APPDATA%\Pensive` | `%LOCALAPPDATA%\Pensive` |

macOS breaks Apple's convention of `~/Library/Application Support`. That's deliberate. Pensive's users edit config files and sync them with dotfiles, and they already find Zed, Helix and most terminal tools under `~/.config`. Zed v1.27.0 makes the same choice on macOS in its `paths` crate, with settings in `~/.config/zed` and state in `~/.local/state`.

On Windows, settings go in Roaming AppData so they follow a domain profile, and state goes in Local AppData because its paths only make sense on one machine.

The [`etcetera`](https://docs.rs/etcetera) crate (0.11) encodes this policy. Its `choose_base_strategy` picks XDG on Linux and macOS and the Windows folders on Windows, and it honors `XDG_CONFIG_HOME` and `XDG_STATE_HOME`. Its Windows strategy has no state folder, so state uses its cache folder there, which is `%LOCALAPPDATA%`. The alternative, [`dirs`](https://docs.rs/dirs) (7.0), returns `~/Library/Application Support` on macOS, so we'd have to special-case macOS the way Zed does.

## Format

All four files are TOML. The theme crate already parses overrides from TOML (PEN-16), and TOML allows comments, which a hand-edited file needs. Zed uses JSON with comments. That buys it a JSON Schema for autocomplete in editors, but TOML editors read schemas too (Taplo and Tombi both do), so we can add one later if people ask.

Settings files are sparse. Defaults live in code, and the file only holds what the user changed. An empty or missing file means every default.

When the palette or settings window changes a value, Pensive edits that one key with [`toml_edit`](https://docs.rs/toml_edit) (0.25), which keeps the user's comments, ordering and spacing. It writes to a temporary file and renames it over the original, so a crash never leaves half a file. When `settings.toml` is a symlink into a dotfiles repo, Pensive resolves the link first and replaces the target, so the link survives.

### Why state is a file and not a database

Zed keeps recent projects and window layouts in SQLite, in its `db` crate. A database pays off when state is large, queried in pieces, or written by several processes at once. Pensive's state today is a short list of projects, written by one process. A TOML file needs no new dependency, can be read and fixed by hand when debugging, and Pensive rewrites it whole on each change. JSON would work as well, but TOML keeps one format and one parser.

If state grows into something worth querying, such as review progress per file, SQLite through `rusqlite` (0.42) is the next step. It would get its own file beside `projects.toml`, so moving there doesn't touch settings.

## The settings file

```toml
theme = "nord"
density = "dense"

[fonts]
code = "Geist Mono"

[palette]
show_tabs = false
open_on = "active"
active_days = 14
opacity = 90
scrim = 25
```

Keys are snake_case and grouped by feature. A setting's full key is its dotted path, like `palette.show_tabs`.

### Declaring a setting

The palette's `setting:` keyword and the settings window (PEN-11) both list settings, so each one needs metadata: a name, a description, its control and its default. There are three ways to keep that metadata next to the value.

- **A settings struct plus a list of items.** This is Zed's way. `SettingsContent` is a tree of optional fields that serde reads, and `settings_ui` has a separate, hand-written list of over 450 items, each with a title, a description and functions that pick and write its field. Every setting is written twice, in two crates, and nothing checks that the list covers the struct.
- **A derive macro on the struct.** The macro reads doc comments and attributes into metadata. One place per setting, but it needs a proc-macro crate, and the struct grows an attribute language.
- **Each setting is a declared value.** A setting is a value with its key, name, description, control and default, typed by what it holds. The store keeps the parsed file as a table, and code reads a setting through its declaration, which returns the typed value or the default. The registry is the list of declarations, so the palette and settings window list exactly what exists.

We use declared values. Each crate declares the settings it reads, next to the code that reads them: the palette's in `ui`, density and fonts in `theme`. Nothing has to be kept in sync, and an unknown key is one with no declaration.

Each declaration names its control, so two settings that hold the same type can use different ones. Opacity and scrim are both percentages, but opacity is a select and scrim is segmented.

| Control | For | Examples |
|---|---|---|
| Toggle | true or false | `palette.show_tabs` |
| Segmented | A choice from two to four short options | `density`, `palette.open_on`, `palette.scrim` |
| Select | A longer list, or one known only at runtime | `theme`, `fonts.code`, `palette.active_days`, `palette.opacity` |

A select's options can come from a function, so `theme` lists the themes on disk and the fonts list what's bundled. Opacity and the Active window are numbers, but they get a select over fixed steps: 70, 80, 90 and 100 percent, or 1, 3, 7, 14 and 30 days. If the settings window wants a slider or free number entry later, that's a fourth control.

### Palette settings

These are the settings `docs/design.md` asks for. Values are a starting point for the implementation.

| Key | Control | Default |
|---|---|---|
| `palette.show_tabs` | Toggle | On |
| `palette.open_on` | Segmented: All, Active, Commands | All |
| `palette.active_days` | Select | 7 |
| `palette.opacity` | Select: 70, 80, 90, 100% | 100% |
| `palette.scrim` | Segmented: 0, 25, 50, 70% | 50% |

The opacity and scrim steps and defaults come from the approved palette canvas (PEN-27). In code, `CommandPalette` already takes an `opacity` prop that defaults to 1, and the root view draws the scrim at 50%. Neither value is in the `theme` crate.

## Theme files

A user theme is a file in `themes/`, named by its file stem. `settings.toml` picks one with `theme = "nord"`. The built-in base is `pensive-dark`, and built-in names are reserved: a user file with that name is ignored, with a warning.

```toml
extends = "pensive-dark"

[colors]
surface_app = "#2e3440"
accent_fg = "#88c0d0"

[fonts]
ui = "Geist"
```

The `[colors]` and `[fonts]` tables are today's `ThemeOverride`, keyed by the role names in `crates/theme`. Colors take any hex form GPUI parses: `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.

`extends` is optional and defaults to `pensive-dark`. It can name another user theme, so someone can tweak a shared theme in a few lines without copying it. Helix themes work the same way, with `inherits`. A loop of themes extending each other, or an `extends` that names a missing theme, is reported. The chain stops there, and the themes before that point still apply over the base. Naming the base in the file costs one line, and it keeps theme files valid if Pensive ever ships a second base.

The theme in use is built in this order, each layer overriding only the roles it names:

1. The base, `pensive-dark`
2. Each theme in the `extends` chain, furthest first
3. The selected theme
4. `[fonts]` in `settings.toml`

Fonts appear in two places on purpose. A theme author can pair fonts with their colors, and a user can still change the code font from the palette without writing a theme. Density isn't part of a theme. It only changes sizes, and it's a setting.

## Mistakes in the files

Three kinds of mistake, handled the same way in settings and theme files:

| Mistake | What Pensive does |
|---|---|
| The file isn't valid TOML | Keeps the last good version and reports the line and column |
| A known key has a bad value, like `density = "compact"` | Uses that key's default, keeps the rest, and reports the key |
| An unknown key, like `palette.show_tab` | Ignores it, keeps the rest, and reports the key |

Declared settings find unknown keys for free. Theme overrides deserialize through the `Refineable` derive, which ignores unknown keys and can't be told otherwise, so the loader wraps it in [`serde_ignored`](https://docs.rs/serde_ignored) (0.1), which reports each key serde skipped. Cargo's "unused manifest key" warning works this way.

Unknown keys are warnings, not errors, so a settings file shared between two Pensive versions still loads in the older one. Zed does the same: an invalid value falls back to its default and the rest of the file still applies. How a report looks in the app needs its own mockup, with the settings implementation.

## Reloading

Pensive watches the config folder with [`notify`](https://docs.rs/notify) (8.2) and applies changes to settings and the selected theme chain as soon as a file is saved, with no restart.

## The project file

`.pensive/project.toml` comes from whatever repo the user cloned. It describes the project, starting with its icon, and nothing else. It can't change the user's settings, the theme or any behavior. Zed lets `.zed/settings.json` override most settings per project. We leave that out until someone needs it, because a cloned repo then decides how Pensive behaves for you. What else belongs in `.pensive/` is PEN-14's question.

## New dependencies

| Crate | Job | Considered instead |
|---|---|---|
| `etcetera` 0.11 | Finds the config and state folders on each platform | `dirs` 7, which needs a macOS special case to use `~/.config` |
| `toml_edit` 0.25 | Changes one key in `settings.toml` and keeps the rest of the file as written | Rewriting the file with `toml`, which drops comments |
| `serde_ignored` 0.1 | Reports unknown keys in theme files | `deny_unknown_fields`, which `Refineable` can't pass through, and which turns a typo into an error |
| `notify` 8.2 | Watches the config folder for edits | Reloading when the window gains focus, which can't show a theme edit live while Pensive sits beside the editor |

`toml` 1.2 is already a dependency.

## Recommendation

1. Settings and themes live in `~/.config/pensive` on Linux and macOS, honoring `XDG_CONFIG_HOME`, and `%APPDATA%\Pensive` on Windows. Resolve them with `etcetera`.
2. Pensive's own state lives in `~/.local/state/pensive` on Linux and macOS and `%LOCALAPPDATA%\Pensive` on Windows, starting with `projects.toml` for recent projects and the icons a user picks. Settings never hold a machine path. State is a file until it needs queries, then SQLite.
3. Every file is TOML. `settings.toml` only holds what the user changed. Pensive edits it with `toml_edit` so comments survive, and writes through symlinks.
4. Each setting is a declared value with a key, name, description, control and default, declared in the crate that reads it. The palette and settings window list the declarations.
5. A user theme is `themes/<name>.toml` with an optional `extends` (default `pensive-dark`) and today's `[colors]` and `[fonts]` tables. The layers apply as base, `extends` chain, theme, then the fonts in settings.
6. Bad values fall back to their defaults and unknown keys are reported, not fatal. Theme files use `serde_ignored` to find unknown keys.
7. Pensive reloads settings and themes when they change on disk.
8. `.pensive/project.toml` describes a project, including the icon everyone who clones it sees, and never overrides user settings.

Left for the implementation: which crate holds the store and the declaration type (it sits below `theme`, `model` and `ui`, so probably a new `settings` crate in `docs/architecture.md`), and how the file I/O fits the backend traits in `model`.

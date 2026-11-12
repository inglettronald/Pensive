---
name: design
description: Start a design session whose drafts use the theme crate's tokens.
disable-model-invocation: true
---

# Design

Sets up a design session whose boards use the app's own values. Boards are screens and components of the Pensive app, not web pages, drawn as HTML and shown inline in the chat. Drafts are throwaway. Once a design is built, its mocks are the record.

## Setup

1. Read `docs/design.md` for the decisions every draft follows.
2. Read the `theme` crate. It is the only source of colors, fonts and sizes.
3. The session's topic comes with the invocation ($ARGUMENTS in Claude Code). If there is none, ask for one before making anything.
4. Write `target/design/<topic>/tokens.css` from the crate. Git ignores `target/`.
   - `--<field>` for each color role
   - `--font_<field>` for each font
   - a `[data-density="<preset in lower case>"]` block per density preset, with `--<field>` for its sizes and `--<role>_size`, `--<role>_line_height` and `--<role>_weight` for each text role. The default preset's block also matches `:root`.
   - a `.typeset-<role>` class per text role that sets size, line height and weight from those variables

## Drafting

- Draw each board as its own HTML file next to `tokens.css`. It links `tokens.css` and loads the theme's fonts from Google Fonts. One board per file keeps a change to one board from resending the others.
- Boards use only the variables and classes in `tokens.css`. A value the crate doesn't have is a proposal, so label it on the board.
- Draw screens at the window size the `pensive` crate opens. The inline frame is 728px wide, so draw a 1:1 crop of the window around what the board is about, or the whole window with `zoom` and the scale written on the board.
- Use plain blocks for the parts of a screen the board isn't about, not screenshots. An embedded image is resent with every render.
- Draw only what GPUI can render. Solid and linear-gradient fills, borders, box shadows, flex and grid are safe. For anything else, check GPUI's styling source at the tag pinned in the root `Cargo.toml` before calling it unsupported, and label on the board what would need hand-built work.
- Draw motion as a looping animation, with each step's duration and easing written on the board and a button that slows it down. GPUI animates by changing an element's style every frame, so opacity, position, size and color are safe. Only SVGs take a transform, so label any scale or rotation of anything else as hand-built work.

## Showing a board

1. Build the page to send: the board with its `tokens.css` link replaced by a `<style>` holding the file. Both tools take one self-contained page.
2. Check it with `html_preview`. Read the PNG and the console, and fix overlaps, clipping and wrapping before the user sees it.
3. Show it with `html_render`, at the `contentHeight` the preview returned. Leave the page itself without a background. The board's window draws its own surface.

Without these tools, render each board to a PNG with a headless browser, give the paths, and say which boards animate, because a PNG can't show it.

## Building an approved draft

Follow `docs/architecture.md` for where the code goes. Render it with `cargo mocks <filter>` and compare the PNGs to the board at the same size until they match.

1. **Record the decisions.** Move what was approved into `docs/design.md`, as decisions rather than values. Proposals that became values go in code, not the doc.
2. **Build the parts first.** Anything the boards draw in more than one place, such as a keycap or a toggle, becomes its own component with a gallery. Render and match those before building the component that uses them.
3. **Port the boards' data as mocks.** Give each board state a mock with the same name and the same sample data, so every PNG has exactly one board to compare against. Add a Dense mock for anything with rows or a fixed height. A mock is one frame, so give an animated board's resting states their own mocks.
4. **Derive new sizes from the theme.** Write a size the crate lacks in terms of a density field, like `row_height * 2 - 8px`, so Dense follows without a second value. Props that pick a color take a role, never a hex value.
5. **Add icons through `assets`.** Download the Lucide SVG at the version the other icons use into `assets/icons`, list it in the `assets` crate, and add its `IconName`. A test fails if a name has no file.
6. **Compare, then fix.** Open each PNG next to the board's preview. Mocks render at twice their size. For text of two sizes on one line, give both the same line height and center them, because `items_baseline` doesn't line them up in GPUI.
7. **Hand over for review.** Run the checks in `AGENTS.md`, then run `cargo mocks <subject>/` and show the subject's atlas, `target/mocks/<subject>.png`, not a description of it. A subject with one mock has no atlas, so show that mock's PNG. Don't commit. The user reviews the diff.

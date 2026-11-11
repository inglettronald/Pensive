---
name: design
description: Start a design session whose drafts use the theme crate's tokens.
disable-model-invocation: true
---

# Design

Sets up a design session whose boards use the app's own values. Boards are screens and components of the Pensive app, not web pages. Drafts are throwaway. Once a design is built, its mocks are the record.

## Setup

1. Read `docs/design.md` for the decisions every draft follows.
2. Read the `theme` crate. It is the only source of colors, fonts and sizes.
3. The session's topic comes with the invocation ($ARGUMENTS in Claude Code). If there is none, ask for one before making anything.
4. Write `tokens.css` from the crate:
   - `--<field>` for each color role
   - `--font_<field>` for each font
   - a `[data-density="<preset in lower case>"]` block per density preset, with `--<field>` for its sizes and `--<role>_size`, `--<role>_line_height` and `--<role>_weight` for each text role. The default preset's block also matches `:root`.
   - a `.typeset-<role>` class per text role that sets size, line height and weight from those variables
5. Set up where the boards live, using the first option your environment supports:
   - **The claude.ai Design artifact type.** Create a canvas from it, titled after the topic. Use no design system, because the theme crate is the look. Put `tokens.css` at `project/tokens.css`. Copy `assets/Tokens.dc.html` to `project/Tokens.dc.html` and fill its two placeholders, `__FONTS_LINK__` and `__THEME__`. The comment above `__THEME__` gives its shape. Publish, and give the link.
   - **Anything else.** Use `target/design/<topic>/`, which git ignores. Put `tokens.css` there and draw each board as a plain HTML file next to it. `cargo mocks theme` renders the tokens for reference.

## Drafting

- Every board links `tokens.css` and loads the theme's fonts from Google Fonts. On the canvas, follow the Tokens board: the stylesheet in `<head>`, the fonts in `<helmet>`.
- Boards use only the variables and classes in `tokens.css`. A value the crate doesn't have is a proposal, so label it on the board.
- Draw screens at the window size the `pensive` crate opens.
- Draw only what GPUI can render. Solid and linear-gradient fills, borders, box shadows, flex and grid are safe. For anything else, check GPUI's styling source at the tag pinned in the root `Cargo.toml` before calling it unsupported, and label on the board what would need hand-built work.
- Outside the canvas, render each board to a PNG at its size if you have a browser tool, and give the paths.

## Building an approved draft

Follow `docs/architecture.md` for where the code goes. Render it with `cargo mocks <filter>` and compare the PNGs to the board at the same size until they match.

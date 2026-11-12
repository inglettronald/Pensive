---
name: web-preview
description: Open Pensive in the T3 Code preview, built for the browser on sample data. Use when asked to open, show or run Pensive in the preview, a browser tab or a web view.
---

# Web preview

Builds Pensive for the browser with Trunk and opens it in a T3 Code preview tab. The browser has no `git` command or config folder, so it opens the sample projects from `model::fixtures`, and settings changed in it last until the tab closes. Use it to click through views. `cargo mocks` is still how to check what a view looks like.

## Opening it

1. Run `trunk --version`. If Trunk is missing, ask the user to run `cargo install trunk --locked`, and stop. Don't install it yourself. The wasm target comes with the toolchain in `rust-toolchain.toml`.
2. If `curl -sf http://127.0.0.1:8765/ >/dev/null` succeeds, a server is already running. Skip to step 4.
3. From the repo root, start `trunk serve --config crates/pensive/Trunk.toml` as a background command. Wait until the curl in step 2 succeeds, without a foreground `sleep`. A cold build takes several minutes. If the server exits first, show the error from its output and stop.
4. Call `preview_open` with `target: {kind: "environment-port", port: 8765}`. Load it with ToolSearch first if it isn't loaded.
5. Call `preview_snapshot` with `includeImage: true` and look at the image. GPUI draws everything to one canvas, so the accessibility tree is empty and only the image shows the app. A panic or a failed GPU setup shows up in the snapshot's console diagnostics.

Leave the server and the tab open for the user. Trunk rebuilds when a file in `crates/` or `assets/` changes, then reloads the page.

## Driving it

Locators can't reach inside the canvas. Keys go to a hidden textarea that GPUI keeps focused, so drive the app with the keyboard: `preview_act` with `action: "type"` to type into the focused field, and `action: "press"` for keys such as `Enter`. To open a project, type part of its name into the palette and press `Enter`.

Clicks work at `x` and `y` in CSS pixels, which is the snapshot image's size divided by the page's `devicePixelRatio`. Clicking a palette row takes keyboard focus off the palette's search field, and keys stop reaching it. Reload the page with `preview_navigate` to get it back.

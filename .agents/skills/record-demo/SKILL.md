---
name: record-demo
description: Record a flow through Pensive in the T3 preview to a video, starting from a named mock.
disable-model-invocation: true
---

# Record a demo

Records the browser build in a T3 Code preview tab while you drive it through a flow, and saves the video to `target/demos/`. It starts from a named mock, so each take of a flow starts on the same screen.

The invocation ($ARGUMENTS in Claude Code) names the mock and describes the flow, such as `root/project: open the palette with Ctrl K and type "font"`. Mock ids are `<subject>/<mock>`, as `cargo mocks` names its PNGs. It may add a size in CSS pixels as `<width>x<height>`, such as `1440x900`. Without one, use `1280x800`. If the mock or the flow is missing, ask for it before doing anything else.

If any step below fails: stop the recording if it started, close the tab if you opened one, stop the server if you started it, then tell the user what failed.

## Checking the tools

1. Run `command -v trunk ffmpeg ffprobe`. If any is missing, ask the user to install it (`cargo install trunk --locked` for Trunk, their package manager for FFmpeg), and stop. Don't install anything yourself.
2. Load `preview_open`, `preview_recording_start` and `preview_recording_stop` with ToolSearch if they aren't loaded. If the recording tools don't exist in this session, say so and stop.

## Opening the mock

1. Pick a free port with `python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'`. Your own server on its own port means a `trunk serve` already on 8765, maybe from another worktree, can't be the one you record.
2. From the repo root, start `trunk serve --config crates/pensive/Trunk.toml --port <port> --dist "$PWD/target/demos/web"` as a background command. The separate `--dist` keeps it from overwriting the files a `web-preview` server serves. Trunk reads a relative `--dist` from `crates/pensive`, so keep it absolute.
3. Wait for it with `curl -sf --retry 290 --retry-connrefused --retry-delay 2 --retry-max-time 590 http://127.0.0.1:<port>/ >/dev/null`, and give the shell tool a `timeout` of 600000, since a cold build takes several minutes. Then read the server's output. Trunk keeps running after a failed build and serves the last good one, or nothing, so if curl failed or the output has a build error after its last `success` line, show the error and stop.
4. Call `preview_open` with `target: {kind: "environment-port", port: <port>, path: "/?mock=<id>"}` and `reuseExistingTab: false`. Keep the `tabId` it returns and pass it to every preview call below, so you never record or close one of the user's tabs.
5. Call `preview_resize` with `mode: "freeform"` and the size. The tab is the app's window, so its size is the window's.
6. Call `preview_snapshot` with `includeImage: true`. If its console entries have a panic such as `no mock is called`, the id is wrong: stop. Otherwise check that the image shows the mock you asked for, not the landing screen.
7. Call `preview_evaluate` with the contents of `keep-painting.js`, next to this file, without the trailing newline, which the tool rejects. Without it, pauses vanish from the video: the recorder only gets a frame when the page paints, and GPUI doesn't paint while nothing changes. A reload removes it, so run it again after any `preview_navigate`.

## Recording

To pause, call `preview_evaluate` with `new Promise(done => setTimeout(done, <ms>))`.

1. Call `preview_recording_start`, then pause 1000 ms so the video opens on the mock.
2. Drive the flow with `preview_act`, as the "Driving it" section of `.agents/skills/web-preview/SKILL.md` says. Pause about 600 ms after each step so the change can be seen. To show typing, type one or two characters per `preview_act`, since one call types its whole text in a single frame.
3. Pause 1500 ms on the last screen, then call `preview_recording_stop`. It returns the path of an MP4. Its size in pixels is the tab's size times the screen's pixel ratio, so 1280x800 at a ratio of 1.25 gives 1600x1000.

## Saving and checking

1. Copy the MP4 to `target/demos/<id with / as ->-<two or three words for the flow>.mp4`. Git ignores `target/`.
2. Run `.agents/skills/record-demo/contact-sheet.sh <video>`. It writes `<video>-sheet.png`, 12 frames evenly spaced from the first to the last, and prints the video's length.
3. Read the sheet. Its first frame should be the mock, its last the end of the flow, and each step should show in the frames between. If a step is missing or a frame is blank, record again.
4. Close the tab with `t3_preview_close`, and stop the server you started with `pkill -f '[t]runk serve .*--port <port>'`. The brackets keep the pattern from matching the `pkill` command's own shell.
5. Reply with the video's path, its length, and what the sheet shows.

---
name: record-demo
description: Record a flow through Pensive in the T3 preview to a video, starting from a named mock.
disable-model-invocation: true
---

# Record a demo

Records the browser build in a T3 Code preview tab while you drive it through a flow, and saves the video to `target/demos/`. It starts from a named mock, so each take of a flow starts on the same screen.

The invocation ($ARGUMENTS in Claude Code) names the mock and describes the flow, such as `root/project: open the palette with Ctrl K and type "font"`. It may give a size. Without one, use 1280x800. If the mock or the flow is missing, ask for it before recording. Mock ids are `<subject>/<mock>`, as `cargo mocks` names its PNGs.

## Opening the mock

1. Start the server with steps 1 to 3 of `.agents/skills/web-preview/SKILL.md`.
2. Call `preview_open` with `target: {kind: "environment-port", port: 8765, path: "/?mock=<id>"}` and `reuseExistingTab: false`. Keep the `tabId` it returns and pass it to every preview call below, so you never record or close one of the user's tabs.
3. Call `preview_resize` with `mode: "freeform"` and the size. The tab is the app's window, so its size is the window's.
4. Call `preview_snapshot` with `includeImage: true`. Check that the image shows the mock. If its console entries have a panic such as `no mock is called`, tell the user and stop.
5. Call `preview_evaluate` with the contents of `keep-painting.js`, next to this file, without the trailing newline, which the tool rejects. Without it, pauses vanish from the video: the recorder only gets a frame when the page paints, and GPUI doesn't paint while nothing changes.

## Recording

To pause, call `preview_evaluate` with `new Promise(done => setTimeout(done, <ms>))`.

1. Call `preview_recording_start`, then pause 1000 ms so the video opens on the mock.
2. Drive the flow with `preview_act`, as the "Driving it" section of `web-preview` says. Pause about 600 ms after each step so the change can be seen. To show typing, type one or two characters per `preview_act`, since one call types its whole text in a single frame.
3. Pause 1500 ms on the last screen, then call `preview_recording_stop`. It returns the path of an MP4, sized at the tab's size times the screen's pixel ratio.

## Saving and checking

1. Copy the MP4 to `target/demos/<id with / as ->-<two or three words for the flow>.mp4`. Git ignores `target/`.
2. Get the video's length in seconds with `ffprobe -v error -show_entries format=duration -of csv=p=0 <video>`. If `ffprobe` or `ffmpeg` is missing, ask the user to install FFmpeg, and say in your reply that the video wasn't checked.
3. Make a contact sheet of 12 frames spread over the video, where `<rate>` is 12 divided by the length:
   `ffmpeg -v error -y -i <video> -vf "fps=<rate>,scale=640:-1,tile=3x4" -frames:v 1 <video without .mp4>-sheet.png`
   Read the sheet. Its first frame should be the mock, its last the end of the flow, and each step should show in the frames between. If a step is missing or a frame is blank, record again.
4. Close the tab with `t3_preview_close`. Leave the server running.
5. Reply with the video's path, its length, and what the sheet shows.

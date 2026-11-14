# Overlay blur

Decided in PEN-26. Covers whether the command palette, and later other overlays, can blur the window behind them, what each way of doing it costs, and what Pensive does instead. GPUI paths are in Zed's repo at v1.27.0, the tag we pin, under `crates/`. Upstream pull requests were checked on GitHub in November 2026.

Pensive keeps opacity and scrim as the only backdrop settings. GPUI can't blur part of a window, and adding that means maintaining a fork of GPUI's renderers. On Pensive's dark gray interface, blur would change little that the opacity setting doesn't already do.

## How GPUI draws a frame

Each frame, elements paint into a `Scene`: flat lists of primitives, one list per kind (`gpui/src/scene.rs:41-53`). There are eight kinds: quads, shadows, paths, underlines, three kinds of sprite (glyphs and images), and surfaces (`scene.rs:222-231`). Each primitive gets an order number from a spatial tree, so a primitive that overlaps an earlier one sorts after it (`scene.rs:87-100`, `gpui/src/bounds_tree.rs:13-19`). The renderer walks the lists in that order as batches (`scene.rs:172`, `scene.rs:477-496`) and draws them into the window's frame texture in one pass. It clears the frame first and redraws the whole window every time it presents (`gpui_wgpu/src/wgpu_renderer.rs:1503-1515`).

A blur behind an element, like CSS `backdrop-filter`, needs something that pass never does: stop partway through the frame, read back what's drawn under the element so far, blur it, and draw it back inside the element's rounded rect before the element's own fill.

## What GPUI has

**No blur inside a window.** No primitive reads what's already drawn. `Window`'s paint methods cover quads, shadows, paths, underlines, glyphs, SVGs, images and surfaces (`gpui/src/window.rs:4579-5201`). A fill is a solid color, a linear gradient, a slash pattern or a checkerboard (`gpui/src/color.rs:745-750`). The only blur in the shaders is the one on box shadows, which computes a blurred rectangle's edge from a formula and never samples pixels (`gpui_wgpu/src/shaders.wgsl:985-1021`, `gpui_apple/src/shaders.metal:476-540`, `gpui_windows/src/shaders.hlsl:894-930`).

**No way for Pensive to add one.** The primitive list is a closed enum in `gpui`, and each platform's window builds its own renderer, so an app can't add a primitive or a render pass. Surfaces look like a way in, but they carry macOS video frames and the wgpu renderer skips them (`wgpu_renderer.rs:1606-1608`).

**Blur behind the whole window.** `WindowBackgroundAppearance::Blurred` asks the OS compositor to blur what's behind the window, and GPUI's own comment says "Not always supported" (`gpui/src/platform.rs:2894-2898`).

| Platform | How | Where |
|---|---|---|
| macOS | An `NSVisualEffectView` behind the Metal layer, blending what's behind the window | `gpui_macos/src/window.rs:1849-1868`, `3836-3852` |
| Windows | `SetWindowCompositionAttribute` with the acrylic accent, an undocumented `user32` export looked up at runtime | `gpui_windows/src/window.rs:929-931`, `1640-1670` |
| Wayland | KDE's `org_kde_kwin_blur`, only when the compositor offers it | `gpui_linux/src/linux/wayland/window.rs:2322-2335`, `wayland/client.rs:306` |
| X11 | None. The value only turns on transparency | `gpui_linux/src/linux/x11/window.rs:296-299`, `1625-1629` |
| Browser | None | |

GNOME's compositor doesn't offer `org_kde_kwin_blur`, so on GNOME `Blurred` is plain transparency.

Zed uses only the window-level blur. A theme picks the window's appearance, and `theme_is_transparent` tells Zed's panels to drop their own fill so the desktop shows through (`ui/src/styles/appearance.rs:13-21`). Nothing in Zed blurs inside a window.

## Upstream

People have asked for this and tried to add it. None of it landed.

| | What | Outcome |
|---|---|---|
| [Discussion #47429](https://github.com/zed-industries/zed/discussions/47429), January 2026 | "Backdrop Blur for GPUI", offering a fork's patch | No reply from Zed staff |
| [#54512](https://github.com/zed-industries/zed/pull/54512), April 2026 | Blur and "liquid glass" primitives for wgpu and Metal, a stub for DirectX, +13,054 lines | Author closed it to keep it in their fork |
| [#59026](https://github.com/zed-industries/zed/pull/59026), June 2026 | A backdrop blur primitive, DirectX only, +1,569 lines | Closed for having no staff buy-in and only one backend |
| [#63773](https://github.com/zed-industries/zed/pull/63773) and its stack, September 2026 | Effect layers for CSS `filter` and `backdrop-filter`, all three renderers, +2,066 lines for the core part alone | Closed by a Zed staff member the same day |

Zed's contributing guide asks for staff buy-in before a feature PR, and no Zed staff member has given it on any of these, so waiting for upstream has no date. Forks carry versions of it: #54512's author moved theirs to a fork, and #63773 is the same change as one in the `remorses/zed` fork. One fork is on crates.io, [`gpui-box`](https://crates.io/crates/gpui-box), with a `Window::paint_backdrop_blur`. Both its releases came out on one day in August 2026, before v1.27.0, and it hasn't changed since. Using it would mean swapping all of GPUI for a third party's snapshot, so it doesn't count as an upstream path.

## Options

### 1. A backdrop blur pass in GPUI

What it takes:

- In `gpui`, a backdrop primitive with bounds, corner radii, clip and radius, its own batch kind, a `Window::paint_backdrop_blur`, and a `backdrop_blur` style that a `div` paints before its fill. The primitive has to take its order with its bounds grown by three times the blur radius. The spatial tree then sorts it after everything the blur samples, including content just outside the element.
- In each renderer, at a backdrop batch: end the pass, copy the region under it, blur it at reduced size, start a new pass that keeps what's drawn, and draw the blurred copy through the rounded rect. Paths already split the pass this way to draw into a side texture (`wgpu_renderer.rs:1532-1566`, `gpui_apple/src/metal_renderer.rs:807-830`, `gpui_windows/src/directx_renderer.rs:372-375`), so the structure exists.
- Shaders in three languages: WGSL (plus the WebGL variant in `gpui_wgpu/src/shaders_webgl.wgsl`), Metal and HLSL.

The hard part differs per renderer, because none of them can read the frame texture as it's set up today.

| Renderer | Used for | The catch | Risk |
|---|---|---|---|
| wgpu | Linux, the browser, every mock | The swapchain texture only allows drawing into it (`wgpu_renderer.rs:453-455`). Not every surface allows copying from it either, so frames with a backdrop would draw into an offscreen texture and copy that to the screen at the end. The headless target mocks use can already be copied (`wgpu_renderer.rs:2370`) | Low. One code path, and mocks check it |
| Metal | macOS | The drawable is framebuffer-only. GPUI turns that off only in debug test builds, because it costs presentation (`metal_renderer.rs:207-211`). Release builds would need the offscreen texture too | Medium. Needs a Mac to check |
| DirectX 11 | Windows | The swapchain buffers are render targets, not shader inputs (`directx_renderer.rs:1313`, `1342`). Copying out is easy: GPUI already copies the back buffer for screenshots (`directx_renderer.rs:431-457`) | Medium. Needs a Windows machine, and no mock covers this renderer (PEN-24) |

Mocks would show the blur on Linux and Windows, which render mocks with wgpu (`crates/mock/src/lib.rs:152-159`), and on macOS, where the headless Metal renderer shares the window's draw code (`metal_renderer.rs:636`, `772`).

Size and upkeep. The closed upstream attempts run from 1,500 lines for one backend to over 2,000 for the core alone. I'd expect 2,500 to 4,000 lines for all three renderers: roughly 3 to 5 days for the core and wgpu, and 2 to 3 days each for Metal and DirectX, plus testing on real Macs and Windows machines. Nobody upstream would review or maintain it, so it means a fork: all four of our Zed dependencies point at it, and every GPUI upgrade rebases the patch. The files it touches change often. In the six months to November 2026, `scene.rs` had 12 upstream commits, `wgpu_renderer.rs` 14, `metal_renderer.rs` 16 and `directx_renderer.rs` 12.

Performance, estimated from pixel counts and not measured. The palette is 576 dp wide (`crates/components/src/command_palette/mod.rs:329`) and about 400 dp tall with results. At 2x scale with a 16 dp blur, the region to copy is about 1,350 by 1,000 pixels. Blurring at reduced size adds about a third more work, which totals about one full-window copy at 1080p. That should cost well under a millisecond on a laptop GPU. It runs on every frame GPUI presents while the palette is open. Frames come from typing and hover, and from the toast's draining line, which requests a new frame on every frame while a toast counts down (`crates/ui/src/notices.rs:272-274`). The renderer can't cache the blur, because it gets a fresh scene each frame with no record of what changed under the palette. One palette can afford this. Blur on every tooltip and menu would multiply it.

### 2. The palette as its own window

Open the palette in a separate transparent window with `Blurred`, and let the OS compositor blur the main window underneath it.

- Blur only works where `Blurred` works: macOS, Windows and KDE. GNOME, X11 and the browser get plain transparency.
- Wayland doesn't let an app place its own top-level windows. GPUI's anchored popups can be placed relative to a parent, but only on Wayland. macOS, Windows and X11 reject them (`gpui_macos/src/platform.rs:750-754`, `gpui_windows/src/window.rs:443-447`, `gpui_linux/src/linux/x11/window.rs:477-481`).
- Keyboard focus moves to another window, so Pensive's title bar shows the main window as inactive while the palette is open. The palette also has to follow the main window when it moves or resizes, and the scrim (in the main window) and the palette (in its own) fade in separately.
- Mocks can't show it. The headless renderer draws one window into an opaque target (`wgpu_renderer.rs:2323-2326`), and the blur only happens later, in the OS compositor.

This option rewrites how the palette behaves as a window to get blur on half the platforms, with no mocks of it.

### 3. Fake glass without blur

What real blur shows is the color and tone of what's behind, with none of its detail. Nothing GPUI can draw does both without reading pixels.

- Higher opacity hides detail, but it hides color with it. At 90% the code behind still shows as faint text (see the prototype below).
- A grain texture, a bundled noise image painted over the fill at low alpha, since GPUI has no noise fill (`color.rs:745-750`). It adds texture but keeps the edges of the text behind, so the faint text stays.
- A tinted fill with a gradient sheen (`color.rs:865`) and a lighter top border reads as a material, with the same faint text through it.

I didn't render the last two. They follow from the first: anything drawn over the fill without reading the pixels under it leaves their edges in place.

These are cheap: a few hours in `components`, and they show in mocks everywhere. But none of them reaches what blur does to the content behind.

### 4. Opacity and scrim only

What ships today (PEN-31). Opacity defaults to 100% and scrim to 50%, so by default nothing shows through the palette at all.

## Prototype

To see whether blur would be worth it on Pensive's own interface, I simulated it without changing GPUI. On a throwaway local branch, I rendered the `root/palette` mock at 70% opacity and 25% scrim, then again over an opaque black scrim and an opaque white one. Those two renders give the palette's color and coverage at every pixel. I composited them with ffmpeg over a Gaussian-blurred copy of the window as the blur would draw it. To check the method, compositing over the unblurred window reproduces GPUI's own render at 52.7 dB PSNR, which is within 8-bit rounding.

The result:

- At 70% opacity with no blur, the code in the editor shows through clearly and competes with the palette's rows.
- At 90% with no blur, the code is still legible as faint text.
- At 70% with an 8 dp or 16 dp blur, the text behind is gone, and what's left is an almost even dark gray with faint green and red where diff lines sit. Both radii look nearly the same, because the window is mostly gray.

So blur would stop text showing through at lower opacity, and little else. Opacity already does that at 100%, the default. Beyond that, blur adds a faint color wash from the diff, which isn't worth a renderer fork.

## Recommendation

1. Don't build blur now. Pensive doesn't fork GPUI for one overlay effect, and option 2 is ruled out.
2. Keep opacity and scrim as the palette's backdrop settings, with 100% opacity as the default.
3. Don't add a fake glass treatment. None of the cheap versions hides the text behind, which is the one thing blur would do here.
4. Revisit if Zed accepts backdrop blur upstream, or if Pensive forks GPUI for another reason. Then build option 1 in the wgpu renderer first, since that covers Linux, the browser and every mock, and add Metal and DirectX after.

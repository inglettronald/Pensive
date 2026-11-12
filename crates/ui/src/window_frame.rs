//! The frame around a window whose decorations Pensive draws: a shadow, rounded corners and
//! edges that resize the window. Adapted from GPUI's `examples/window_shadow.rs` (Apache-2.0, in
//! this crate's `LICENSE-APACHE`) at the Zed tag pinned in the root `Cargo.toml`.
//!
//! With server decorations, as on macOS, Windows and in mocks, it adds nothing.

use gpui::{
    AnyElement, App, Bounds, BoxShadow, CursorStyle, Decorations, HitboxBehavior, MouseButton,
    Pixels, Point, ResizeEdge, Size, Tiling, Window, canvas, div, hsla, point, prelude::*, px,
    transparent_black,
};
use theme::ActiveTheme;

/// How far outside the window the shadow and the resize edges reach.
const SHADOW: Pixels = px(10.);
const ROUNDING: Pixels = px(10.);

/// Whether Pensive draws minimize, maximize and close in the title bar. macOS keeps its own,
/// and so does a Linux compositor that decorates the window itself.
pub(crate) fn draws_window_controls(window: &Window) -> bool {
    if cfg!(target_os = "macos") {
        false
    } else if cfg!(target_os = "windows") {
        true
    } else {
        matches!(window.window_decorations(), Decorations::Client { .. })
    }
}

/// Wraps `content` in the frame when the window draws its own decorations.
pub(crate) fn window_frame(content: AnyElement, window: &mut Window, cx: &App) -> AnyElement {
    let Decorations::Client { tiling } = window.window_decorations() else {
        return content;
    };
    // A maximized window fills the screen like one tiled on every side: no frame to grab.
    let tiling = if window.is_maximized() {
        Tiling::tiled()
    } else {
        tiling
    };
    window.set_client_inset(SHADOW);
    let border = cx.theme().colors.border;

    div()
        .id("window-frame")
        .size_full()
        .bg(transparent_black())
        .child(
            canvas(
                |_, window, _| {
                    window.insert_hitbox(
                        Bounds::new(
                            point(px(0.), px(0.)),
                            window.window_bounds().get_bounds().size,
                        ),
                        HitboxBehavior::Normal,
                    )
                },
                move |_, hitbox, window, _| {
                    let size = window.window_bounds().get_bounds().size;
                    let Some(edge) = resize_edge(window.mouse_position(), size, tiling) else {
                        return;
                    };
                    window.set_cursor_style(cursor(edge), &hitbox);
                },
            )
            .size_full()
            .absolute(),
        )
        .when(!(tiling.top || tiling.left), |frame| {
            frame.rounded_tl(ROUNDING)
        })
        .when(!(tiling.top || tiling.right), |frame| {
            frame.rounded_tr(ROUNDING)
        })
        .when(!tiling.top, |frame| frame.pt(SHADOW))
        .when(!tiling.bottom, |frame| frame.pb(SHADOW))
        .when(!tiling.left, |frame| frame.pl(SHADOW))
        .when(!tiling.right, |frame| frame.pr(SHADOW))
        .on_mouse_move(|_, window, _| window.refresh())
        .on_mouse_down(MouseButton::Left, move |event, window, _| {
            let size = window.window_bounds().get_bounds().size;
            if let Some(edge) = resize_edge(event.position, size, tiling) {
                window.start_window_resize(edge);
            }
        })
        .child(
            div()
                .size_full()
                .cursor(CursorStyle::Arrow)
                .overflow_hidden()
                .border_color(border)
                .when(!(tiling.top || tiling.left), |inner| {
                    inner.rounded_tl(ROUNDING)
                })
                .when(!(tiling.top || tiling.right), |inner| {
                    inner.rounded_tr(ROUNDING)
                })
                .when(!tiling.top, |inner| inner.border_t_1())
                .when(!tiling.bottom, |inner| inner.border_b_1())
                .when(!tiling.left, |inner| inner.border_l_1())
                .when(!tiling.right, |inner| inner.border_r_1())
                .when(!tiling.is_tiled(), |inner| {
                    inner.shadow(vec![
                        BoxShadow::new(px(0.), px(0.), hsla(0., 0., 0., 0.4))
                            .blur_radius(SHADOW / 2.),
                    ])
                })
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .child(content),
        )
        .into_any_element()
}

fn cursor(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

/// The edge under `position`, if any. A tiled edge has no frame around it, so it never resizes.
fn resize_edge(position: Point<Pixels>, size: Size<Pixels>, tiling: Tiling) -> Option<ResizeEdge> {
    let top = !tiling.top && position.y < SHADOW;
    let bottom = !tiling.bottom && position.y > size.height - SHADOW;
    let left = !tiling.left && position.x < SHADOW;
    let right = !tiling.right && position.x > size.width - SHADOW;
    Some(match (top, bottom, left, right) {
        (true, _, true, _) => ResizeEdge::TopLeft,
        (true, _, _, true) => ResizeEdge::TopRight,
        (true, ..) => ResizeEdge::Top,
        (_, true, true, _) => ResizeEdge::BottomLeft,
        (_, true, _, true) => ResizeEdge::BottomRight,
        (_, true, ..) => ResizeEdge::Bottom,
        (.., true, _) => ResizeEdge::Left,
        (.., true) => ResizeEdge::Right,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use gpui::{point, px, size};

    use super::*;

    #[test]
    fn a_tiled_edge_never_resizes() {
        let window = size(px(800.), px(600.));
        let corner = point(px(2.), px(2.));
        assert_eq!(
            resize_edge(corner, window, Tiling::default()),
            Some(ResizeEdge::TopLeft)
        );
        assert_eq!(resize_edge(corner, window, Tiling::tiled()), None);
        let left_tiled = Tiling {
            left: true,
            ..Tiling::default()
        };
        assert_eq!(
            resize_edge(corner, window, left_tiled),
            Some(ResizeEdge::Top)
        );
    }
}

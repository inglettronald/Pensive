//! A control's last value, kept across frames, so it can animate from it when the value changes.

use gpui::{App, ElementId, Hsla, Window};

use crate::child_id;

/// The value an animation leaves and the one it reaches. Each change gets a new `generation`,
/// which keys the animation so a new change restarts it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transition<T> {
    pub from: T,
    pub to: T,
    pub generation: usize,
}

impl<T: Copy + PartialEq> Transition<T> {
    /// Whether there's a change to animate, rather than a value that has always been `to`.
    pub fn moved(&self) -> bool {
        self.generation > 0 && self.from != self.to
    }

    /// An id for the animation of this change, unique among the control's animations.
    pub fn animation_id(&self, id: &ElementId, part: &'static str) -> ElementId {
        child_id(id, format!("{part}-{}", self.generation))
    }
}

/// The transition that leads to `value` for the control with `id`. A value that changes again
/// mid-animation starts the next one from where the last was headed.
pub(crate) fn transition<T: Copy + PartialEq + 'static>(
    id: &ElementId,
    value: T,
    window: &mut Window,
    cx: &mut App,
) -> Transition<T> {
    let key = child_id(id, "transition");
    let state = window.use_keyed_state(key, cx, |_, _| Transition {
        from: value,
        to: value,
        generation: 0,
    });
    state.update(cx, |state, _| {
        if state.to != value {
            *state = Transition {
                from: state.to,
                to: value,
                generation: state.generation + 1,
            };
        }
        *state
    })
}

/// The color `delta` of the way from `from` to `to`.
pub(crate) fn mix(from: Hsla, to: Hsla, delta: f32) -> Hsla {
    let between = |a: f32, b: f32| a + (b - a) * delta;
    Hsla {
        h: between(from.h, to.h),
        s: between(from.s, to.s),
        l: between(from.l, to.l),
        a: between(from.a, to.a),
    }
}

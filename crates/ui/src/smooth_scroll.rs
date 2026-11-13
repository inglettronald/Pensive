//! Glides a scroll handle to a new offset with an ease-out, so a click or a wheel notch moves the
//! page instead of snapping it. A new target mid-glide starts from where the page is, so notches
//! in a row add up.

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{Context, Pixels, ScrollHandle, Task, point, px};

/// Each glide moves the page once per frame.
const FRAME: Duration = Duration::from_millis(16);

pub(crate) struct SmoothScroll {
    handle: ScrollHandle,
    /// Where the running glide ends, if one is running.
    target: Rc<Cell<Option<Pixels>>>,
    glide: Option<Task<()>>,
}

impl SmoothScroll {
    pub fn new() -> Self {
        Self {
            handle: ScrollHandle::new(),
            target: Rc::default(),
            glide: None,
        }
    }

    /// The handle to track and read. Moving the page through it skips the glide.
    pub fn handle(&self) -> &ScrollHandle {
        &self.handle
    }

    /// Glides to vertical offset `to`, kept within the content, over `duration`.
    pub fn glide_to<V: 'static>(&mut self, to: Pixels, duration: Duration, cx: &mut Context<V>) {
        let to = self.clamp(to);
        let from = self.handle.offset().y;
        self.target.set(Some(to));
        let (handle, target) = (self.handle.clone(), self.target.clone());
        let frames = (duration.as_millis() / FRAME.as_millis()).max(1) as u32;
        self.glide = Some(cx.spawn(async move |view, cx| {
            for frame in 1..=frames {
                cx.background_executor().timer(FRAME).await;
                let done = frame as f32 / frames as f32;
                let eased = 1. - (1. - done).powi(3);
                handle.set_offset(point(handle.offset().x, from + (to - from) * eased));
                if view.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
            target.set(None);
        }));
    }

    /// Glides `delta` further than the running glide is headed, or than the page is.
    pub fn glide_by<V: 'static>(&mut self, delta: Pixels, duration: Duration, cx: &mut Context<V>) {
        let from = self.target.get().unwrap_or_else(|| self.handle.offset().y);
        self.glide_to(from + delta, duration, cx);
    }

    /// Moves straight to vertical offset `to`, kept within the content, ending any glide.
    pub fn jump_to(&mut self, to: Pixels) {
        self.glide = None;
        self.target.set(None);
        let to = self.clamp(to);
        self.handle.set_offset(point(self.handle.offset().x, to));
    }

    /// Offsets run from zero at the top down to minus the most the content can scroll.
    fn clamp(&self, offset: Pixels) -> Pixels {
        offset.clamp(-self.handle.max_offset().y, px(0.))
    }
}

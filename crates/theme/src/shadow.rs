//! The drop shadows under surfaces that float over the window. Like the window's own shadow, they
//! stay the same at any zoom.

use gpui::{BoxShadow, Pixels, hsla, px};

#[derive(Clone, Copy, Debug)]
pub enum Shadow {
    /// Tooltips.
    Small,
    /// Toasts and the notice list.
    Medium,
    /// The command palette over a project.
    Large,
}

impl Shadow {
    /// The shadow as `Styled::shadow` takes it.
    #[expect(
        clippy::disallowed_methods,
        reason = "shadows stay the same at any zoom"
    )]
    pub fn layers(self) -> Vec<BoxShadow> {
        let (offset_y, blur, opacity) = match self {
            Self::Small => (6., 16., 0.5),
            Self::Medium => (8., 24., 0.55),
            Self::Large => (16., 48., 0.6),
        };
        vec![
            BoxShadow::new(Pixels::ZERO, px(offset_y), hsla(0., 0., 0., opacity))
                .blur_radius(px(blur)),
        ]
    }
}

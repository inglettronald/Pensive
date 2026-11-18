//! What a frame painted, in window pixels, read from GPUI's scene for the layout checks.

use gpui::{Bounds, ScaledPixels, Scene};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn is_empty(&self) -> bool {
        self.width() <= 0. || self.height() <= 0.
    }

    pub fn intersect(&self, other: &Self) -> Self {
        Self::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        )
    }

    pub fn union(&self, other: &Self) -> Self {
        Self::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }

    pub fn contains(&self, other: &Self) -> bool {
        self.left <= other.left
            && self.top <= other.top
            && self.right >= other.right
            && self.bottom >= other.bottom
    }

    fn from_scene(bounds: &Bounds<ScaledPixels>, scale: f32) -> Self {
        let (left, top) = (bounds.origin.x.0 / scale, bounds.origin.y.0 / scale);
        Self::new(
            left,
            top,
            left + bounds.size.width.0 / scale,
            top + bounds.size.height.0 / scale,
        )
    }
}

/// A glyph or an icon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    pub bounds: Rect,
    /// What it's clipped to.
    pub clip: Rect,
    /// Of two things that overlap, the higher order painted later.
    pub order: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    /// A color, with how opaque it is from 0 to 1. A box with only a border has 0.
    Solid(f32),
    Gradient,
}

/// A background or a border.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintedBox {
    pub bounds: Rect,
    pub clip: Rect,
    pub order: u32,
    pub fill: Fill,
}

impl PaintedBox {
    pub fn is_opaque(&self) -> bool {
        matches!(self.fill, Fill::Solid(alpha) if alpha >= 0.99)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Painted {
    pub width: f32,
    pub height: f32,
    pub ink: Vec<Ink>,
    pub boxes: Vec<PaintedBox>,
}

impl Painted {
    pub(crate) fn from_scene(scene: &Scene, width: f32, height: f32, scale: f32) -> Self {
        let rect = |bounds| Rect::from_scene(bounds, scale);
        let monochrome = scene
            .monochrome_sprites
            .iter()
            .map(|sprite| (&sprite.bounds, &sprite.content_mask, sprite.order));
        let subpixel = scene
            .subpixel_sprites
            .iter()
            .map(|sprite| (&sprite.bounds, &sprite.content_mask, sprite.order));
        let polychrome = scene
            .polychrome_sprites
            .iter()
            .map(|sprite| (&sprite.bounds, &sprite.content_mask, sprite.order));
        let ink = monochrome
            .chain(subpixel)
            .chain(polychrome)
            .map(|(bounds, mask, order)| Ink {
                bounds: rect(bounds),
                clip: rect(&mask.bounds),
                order,
            })
            .collect();
        let boxes = scene
            .quads
            .iter()
            .map(|quad| PaintedBox {
                bounds: rect(&quad.bounds),
                clip: rect(&quad.content_mask.bounds),
                order: quad.order,
                fill: match quad.background.as_solid() {
                    Some(color) => Fill::Solid(color.a),
                    None if quad.background.is_transparent() => Fill::Solid(0.),
                    None => Fill::Gradient,
                },
            })
            .collect();
        Self {
            width,
            height,
            ink,
            boxes,
        }
    }

    pub fn window(&self) -> Rect {
        Rect::new(0., 0., self.width, self.height)
    }
}

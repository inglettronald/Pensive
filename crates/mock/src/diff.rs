//! Compares a mock's render with an earlier one, pixel by pixel.

use image::{Rgba, RgbaImage};

/// How a render differs from the one before it.
pub struct Diff {
    /// How many pixels differ.
    pub changed: usize,
    /// The smallest box around them, as `(x, y, width, height)` in the render's pixels.
    pub bounds: (u32, u32, u32, u32),
    /// The new render, dimmed, with the changed pixels in magenta.
    pub image: RgbaImage,
}

const MARK: Rgba<u8> = Rgba([255, 0, 255, 255]);

/// What changed from `before` to `after`, or `None` when they're identical. Renders of different
/// sizes count as changed everywhere.
pub fn diff(before: &RgbaImage, after: &RgbaImage) -> Option<Diff> {
    let mut image = RgbaImage::from_fn(after.width(), after.height(), |x, y| {
        let Rgba([r, g, b, _]) = *after.get_pixel(x, y);
        Rgba([r / 3, g / 3, b / 3, 255])
    });
    let same_size = before.dimensions() == after.dimensions();
    let mut changed = 0;
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, pixel) in after.enumerate_pixels() {
        if same_size && before.get_pixel(x, y) == pixel {
            continue;
        }
        changed += 1;
        image.put_pixel(x, y, MARK);
        (left, top) = (left.min(x), top.min(y));
        (right, bottom) = (right.max(x), bottom.max(y));
    }
    let bounds = if changed == 0 {
        (0, 0, 0, 0)
    } else {
        (left, top, right + 1 - left, bottom + 1 - top)
    };
    (changed > 0 || !same_size).then_some(Diff {
        changed,
        bounds,
        image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_renders_have_no_diff() {
        let image = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        assert!(diff(&image, &image.clone()).is_none());
    }

    #[test]
    fn a_changed_pixel_is_counted_boxed_and_marked() {
        let before = RgbaImage::from_pixel(8, 8, Rgba([90, 90, 90, 255]));
        let mut after = before.clone();
        after.put_pixel(2, 3, Rgba([0, 0, 0, 255]));
        after.put_pixel(5, 6, Rgba([0, 0, 0, 255]));
        let diff = diff(&before, &after).unwrap();
        assert_eq!(diff.changed, 2);
        assert_eq!(diff.bounds, (2, 3, 4, 4));
        assert_eq!(*diff.image.get_pixel(2, 3), MARK);
        assert_eq!(*diff.image.get_pixel(0, 0), Rgba([30, 30, 30, 255]));
    }

    #[test]
    fn a_new_size_changes_everything() {
        let before = RgbaImage::new(4, 4);
        let after = RgbaImage::new(4, 5);
        assert_eq!(diff(&before, &after).unwrap().changed, 20);
    }
}

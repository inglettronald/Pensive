//! The interface's lengths: `Dp`, a pixel at 100% zoom that styles elements as rems so it grows
//! and shrinks with the window's rem size, and the few lines that stay fixed at any zoom.

use std::{
    fmt,
    ops::{Add, Div, Mul, Neg, Sub},
};

use gpui::{AbsoluteLength, DefiniteLength, Length, Pixels, Rems, px, rems};

/// GPUI's rem size at 100% zoom.
const REM: f32 = 16.;

/// Dividers and underlines.
#[expect(
    clippy::disallowed_methods,
    reason = "lines stay one pixel at any zoom"
)]
pub const HAIRLINE: Pixels = px(1.);

/// The text cursor's width.
#[expect(
    clippy::disallowed_methods,
    reason = "the cursor stays thin at any zoom"
)]
pub const CARET: Pixels = px(1.5);

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Dp(pub f32);

pub const fn dp(pixels: f32) -> Dp {
    Dp(pixels)
}

impl Dp {
    /// The length on screen, for painting and layout math.
    pub fn to_pixels(self, rem_size: Pixels) -> Pixels {
        rem_size * (self.0 / REM)
    }

    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }
}

impl fmt::Display for Dp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}px", self.0)
    }
}

impl Add for Dp {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for Dp {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl Mul<f32> for Dp {
    type Output = Self;

    fn mul(self, factor: f32) -> Self {
        Self(self.0 * factor)
    }
}

impl Div<f32> for Dp {
    type Output = Self;

    fn div(self, divisor: f32) -> Self {
        Self(self.0 / divisor)
    }
}

impl Neg for Dp {
    type Output = Self;

    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl From<Dp> for Rems {
    fn from(length: Dp) -> Self {
        rems(length.0 / REM)
    }
}

impl From<Dp> for AbsoluteLength {
    fn from(length: Dp) -> Self {
        Rems::from(length).into()
    }
}

impl From<Dp> for DefiniteLength {
    fn from(length: Dp) -> Self {
        Rems::from(length).into()
    }
}

impl From<Dp> for Length {
    fn from(length: Dp) -> Self {
        Rems::from(length).into()
    }
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "compares against pixels on screen"
)]
mod tests {
    use super::*;

    #[test]
    fn whole_and_half_pixels_land_exactly_at_the_default_rem() {
        for pixels in [1., 1.5, 5.5, 13., 28., 280., 680.] {
            assert_eq!(dp(pixels).to_pixels(px(REM)), px(pixels));
        }
        assert_eq!(dp(24.).to_pixels(px(20.)), px(30.));
    }
}

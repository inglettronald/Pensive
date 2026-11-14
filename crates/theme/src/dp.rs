//! `Dp`, a length in pixels at 100% zoom. It styles elements as rems, so everything sized in it
//! grows and shrinks with the window's rem size.

use std::{
    fmt,
    ops::{Add, Div, Mul, Neg, Sub},
};

use gpui::{AbsoluteLength, DefiniteLength, Length, Pixels, Rems, px, rems};

/// GPUI's rem size at 100% zoom.
const REM: f32 = 16.;

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Dp(pub f32);

pub const fn dp(pixels: f32) -> Dp {
    Dp(pixels)
}

impl Dp {
    /// The length on screen, for painting and layout math.
    pub fn to_pixels(self, rem_size: Pixels) -> Pixels {
        px(self.0 / REM * rem_size.as_f32())
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

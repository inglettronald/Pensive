//! The window sizes, zooms and densities `cargo mocks --matrix` renders window mocks at.

use theme::{DensityPreset, VariantArray};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
    /// The setup that gives a window this size.
    pub setup: &'static str,
}

const fn size(width: u32, height: u32, setup: &'static str) -> WindowSize {
    WindowSize {
        width,
        height,
        setup,
    }
}

/// What the other sizes are compared with: a window with room for everything, so what's short of
/// room only at another size stands out.
pub const REFERENCE: WindowSize = size(3840, 2160, "maximized on 4K");

pub const SIZES: [WindowSize; 9] = [
    size(960, 1080, "half of 1080p"),
    size(1280, 1440, "half of 1440p"),
    size(960, 540, "quarter of 1080p"),
    size(1080, 1920, "portrait 1080p"),
    size(3440, 1440, "ultrawide"),
    size(1720, 1440, "half of ultrawide"),
    size(1280, 800, "laptop"),
    size(1440, 900, "laptop"),
    size(720, 480, "minimum window"),
];

pub const ZOOMS: [u32; 2] = [80, 200];

/// One window size at one zoom and density.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub size: WindowSize,
    pub zoom: u32,
    pub density: DensityPreset,
}

impl Cell {
    /// `<width>x<height>/zoom<zoom>-<density>`, which is also its folder.
    pub fn id(&self) -> String {
        let density: &str = self.density.into();
        format!(
            "{}x{}/zoom{}-{}",
            self.size.width,
            self.size.height,
            self.zoom,
            density.to_lowercase()
        )
    }

    /// The same zoom and density at the reference size.
    pub fn reference(&self) -> Self {
        Self {
            size: REFERENCE,
            ..*self
        }
    }
}

/// Every cell of `size`.
pub fn cells_of(size: WindowSize) -> impl Iterator<Item = Cell> {
    ZOOMS.into_iter().flat_map(move |zoom| {
        DensityPreset::VARIANTS.iter().map(move |&density| Cell {
            size,
            zoom,
            density,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_size_has_a_cell_for_each_zoom_and_density() {
        let ids: Vec<String> = cells_of(SIZES[8]).map(|cell| cell.id()).collect();
        assert_eq!(
            ids,
            [
                "720x480/zoom80-calm",
                "720x480/zoom80-dense",
                "720x480/zoom200-calm",
                "720x480/zoom200-dense",
            ]
        );
    }
}

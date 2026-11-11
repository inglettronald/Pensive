//! Stateless building blocks that render from props and the theme.

#[cfg(feature = "mocks")]
mod gallery;
#[cfg(feature = "mocks")]
mod theme_gallery;

#[cfg(feature = "mocks")]
pub use gallery::Gallery;

/// Every component's mocks.
#[cfg(feature = "mocks")]
pub fn mocks() -> Vec<mock::Mock> {
    theme_gallery::mocks()
}

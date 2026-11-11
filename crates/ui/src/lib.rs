//! Views: the screens and panels that read models and own their view state.

mod root;

pub use root::Pensive;

/// Every view's mocks.
#[cfg(feature = "mocks")]
pub fn mocks() -> Vec<mock::Mock> {
    root::mocks()
}

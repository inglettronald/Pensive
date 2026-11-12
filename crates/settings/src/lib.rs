//! Declared settings, and the store that holds the settings file's text and the values in it.
//!
//! Each crate declares the settings it reads as `static`s and passes them to
//! `Settings::declare`. The store does no I/O. The app hands it the file's text and writes back
//! `Settings::source` when it changes.

mod setting;
mod store;

pub use setting::{AnySetting, Choice, Control, ControlState, Setting};
pub use store::Settings;

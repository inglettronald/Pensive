//! Declared settings, and the store that reads and edits the user's settings file.
//!
//! Each crate declares the settings it reads as `static`s and passes them to
//! `Settings::declare`. The store never touches the disk: the app hands it the file's text and
//! writes back `Settings::source` when it changes.

mod setting;
mod store;

pub use setting::{AnySetting, Choice, Control, ControlState, Setting, SettingValue};
pub use store::Settings;

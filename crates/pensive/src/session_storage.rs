//! A `ConfigBackend` for the browser that keeps `settings.toml` in the tab's session storage, so a
//! reload keeps the settings and a new tab starts from the defaults.

use anyhow::{Context, Result, anyhow};
use model::ConfigBackend;
use web_sys::{
    Storage, js_sys,
    wasm_bindgen::{JsCast, JsValue},
};

const SETTINGS_KEY: &str = "pensive.settings";
const PROBE_KEY: &str = "pensive.probe";

#[derive(Debug)]
pub struct SessionStorageConfig;

impl SessionStorageConfig {
    /// `None` when the browser won't let this page store anything, as when site data is blocked
    /// or the storage quota is zero.
    pub fn open() -> Option<Self> {
        let storage = storage().ok()?;
        storage.set_item(PROBE_KEY, "").ok()?;
        storage.remove_item(PROBE_KEY).ok()?;
        Some(Self)
    }
}

impl ConfigBackend for SessionStorageConfig {
    fn read_settings(&self) -> Result<String> {
        let text = storage()?.get_item(SETTINGS_KEY).map_err(js_error)?;
        Ok(text.unwrap_or_default())
    }

    fn write_settings(&self, text: &str) -> Result<()> {
        storage()?.set_item(SETTINGS_KEY, text).map_err(js_error)
    }

    fn read_themes(&self) -> Result<Vec<(String, String)>> {
        Ok(Vec::new())
    }

    /// Session storage belongs to one tab, so nothing else can change it while the app runs.
    fn watch(&self, _: Box<dyn Fn() + Send + Sync>) -> Result<Box<dyn Send>> {
        Ok(Box::new(()))
    }
}

/// The tab's session storage, looked up on each call because a `Storage` can't be sent between
/// threads, which `ConfigBackend` requires.
fn storage() -> Result<Storage> {
    web_sys::window()
        .context("there's no browser window")?
        .session_storage()
        .map_err(js_error)?
        .context("the browser has no session storage")
}

/// The message of what JavaScript threw, which is usually an `Error` such as a `DOMException`.
fn js_error(error: JsValue) -> anyhow::Error {
    let message = error.as_string().or_else(|| {
        let error = error.dyn_ref::<js_sys::Error>()?;
        Some(error.message().into())
    });
    anyhow!(message.unwrap_or_else(|| format!("{error:?}")))
}

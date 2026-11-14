//! A `ConfigBackend` for the browser that keeps `settings.toml` in the tab's session storage, so a
//! reload keeps the settings and a new tab starts from the defaults.

use anyhow::{Context, Result, anyhow};
use model::ConfigBackend;
use web_sys::{Storage, wasm_bindgen::JsValue};

const SETTINGS_KEY: &str = "pensive.settings";

#[derive(Debug)]
pub struct SessionStorageConfig;

impl SessionStorageConfig {
    /// `None` when the browser denies this page session storage, as it can when site data is
    /// blocked.
    pub fn open() -> Option<Self> {
        storage().ok().map(|_| Self)
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

fn js_error(error: JsValue) -> anyhow::Error {
    anyhow!("{error:?}")
}

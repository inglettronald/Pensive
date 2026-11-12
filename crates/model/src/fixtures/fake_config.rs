//! A `ConfigBackend` that keeps the files in memory instead of in the config folder.

use std::sync::Mutex;

use anyhow::Result;

use crate::ConfigBackend;

#[derive(Debug, Default)]
pub struct FakeConfig {
    settings: Mutex<String>,
    themes: Vec<(String, String)>,
}

impl FakeConfig {
    pub fn new(settings: &str, themes: &[(&str, &str)]) -> Self {
        Self {
            settings: Mutex::new(settings.into()),
            themes: themes
                .iter()
                .map(|(name, text)| (name.to_string(), text.to_string()))
                .collect(),
        }
    }

    /// `settings.toml` as it is now.
    pub fn settings(&self) -> String {
        self.settings.lock().unwrap().clone()
    }

    /// Changes `settings.toml` the way saving it in an editor would.
    pub fn edit_settings(&self, text: &str) {
        *self.settings.lock().unwrap() = text.into();
    }
}

impl ConfigBackend for FakeConfig {
    fn read_settings(&self) -> Result<String> {
        Ok(self.settings())
    }

    fn write_settings(&self, text: &str) -> Result<()> {
        self.edit_settings(text);
        Ok(())
    }

    fn read_themes(&self) -> Result<Vec<(String, String)>> {
        Ok(self.themes.clone())
    }

    fn watch(&self, _: Box<dyn Fn() + Send + Sync>) -> Result<Box<dyn Send>> {
        Ok(Box::new(()))
    }
}

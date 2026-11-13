//! The user's settings file, as its text and as checked values, and every declared setting.

use gpui::{App, Global};
use toml_edit::{DocumentMut, Item, TableLike};

use crate::{AnySetting, Page};

#[derive(Default)]
pub struct Settings {
    /// Each setting with the page it's listed on.
    declared: Vec<(Page, &'static dyn AnySetting)>,
    source: String,
    /// The values in `source` that belong to a declared setting and are valid for it.
    values: toml::Table,
    /// Set while `source` isn't valid TOML. Changes then stay in memory, so the file is left for
    /// the user to fix.
    broken: bool,
}

impl Global for Settings {}

impl Settings {
    /// Adds `settings` to the ones the palette and the settings view list, on `page`. Call it
    /// before loading the file, so their keys are known. A key that's already declared is left
    /// where it is, so declaring twice, as a mock does in a running app, lists it once.
    pub fn declare(
        page: Page,
        settings: impl IntoIterator<Item = &'static dyn AnySetting>,
        cx: &mut App,
    ) {
        let store = cx.default_global::<Settings>();
        for setting in settings {
            if store.find(setting.key()).is_none() {
                store.declared.push((page, setting));
            }
        }
    }

    /// Every declared setting with its page, in the order they were declared.
    pub fn declared(&self) -> impl Iterator<Item = (Page, &'static dyn AnySetting)> {
        self.declared.iter().copied()
    }

    pub fn find(&self, key: &str) -> Option<&'static dyn AnySetting> {
        self.declared()
            .map(|(_, setting)| setting)
            .find(|setting| setting.key() == key)
    }

    /// The file's text, with any changes made in the app.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Reads `source` as the settings file and returns what's wrong with it. When it isn't valid
    /// TOML, the values stay as they were.
    pub fn load(&mut self, source: &str) -> Vec<String> {
        self.source = source.to_string();
        match source.parse::<toml::Table>() {
            Ok(table) => {
                self.broken = false;
                let mut problems = Vec::new();
                self.values = self.checked(table, "", &mut problems);
                problems
            }
            Err(error) => {
                self.broken = true;
                vec![error.to_string()]
            }
        }
    }

    pub(crate) fn value(&self, key: &str) -> Option<&toml::Value> {
        let (parents, last) = split(key);
        let mut table = &self.values;
        for parent in parents {
            table = table.get(parent)?.as_table()?;
        }
        table.get(last)
    }

    /// Sets `key` to `value`, or removes it for `None`, in the values and in the file's text.
    pub(crate) fn edit(&mut self, key: &str, value: Option<toml::Value>) {
        set_value(&mut self.values, key, value.clone());
        if self.broken {
            return;
        }
        let mut document: DocumentMut = self.source.parse().expect("the source is valid TOML");
        set_item(document.as_table_mut(), key, value);
        self.source = document.to_string();
    }

    /// The entries of `table` that are valid settings. Reports the others in `problems`.
    fn checked(&self, table: toml::Table, prefix: &str, problems: &mut Vec<String>) -> toml::Table {
        let mut valid = toml::Table::new();
        for (key, value) in table {
            let path = match prefix {
                "" => key.clone(),
                prefix => format!("{prefix}.{key}"),
            };
            if let Some(setting) = self.find(&path) {
                match setting.check(&value) {
                    Ok(()) => {
                        valid.insert(key, value);
                    }
                    Err(error) => problems.push(format!(
                        "`{path}` uses the default because its value is wrong: {}",
                        error.trim()
                    )),
                }
            } else if let toml::Value::Table(table) = value {
                let table = self.checked(table, &path, problems);
                if !table.is_empty() {
                    valid.insert(key, toml::Value::Table(table));
                }
            } else {
                problems.push(format!("`{path}` isn't a setting"));
            }
        }
        valid
    }
}

/// The tables a dotted key sits in, and its last segment.
fn split(key: &str) -> (impl Iterator<Item = &str>, &str) {
    let (parents, last) = key.rsplit_once('.').unwrap_or(("", key));
    (parents.split('.').filter(|parent| !parent.is_empty()), last)
}

fn set_value(table: &mut toml::Table, key: &str, value: Option<toml::Value>) {
    let (parents, last) = split(key);
    let mut table = table;
    for parent in parents {
        let entry = table
            .entry(parent)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            *entry = toml::Value::Table(toml::Table::new());
        }
        table = entry.as_table_mut().expect("just made a table");
    }
    match value {
        Some(value) => table.insert(last.to_string(), value),
        None => table.remove(last),
    };
}

/// Like `set_value`, but in the file's text, keeping its comments and layout.
fn set_item(table: &mut dyn TableLike, key: &str, value: Option<toml::Value>) {
    let (parents, last) = split(key);
    let mut table = table;
    for parent in parents {
        if table.get(parent).and_then(Item::as_table_like).is_none() {
            table.insert(parent, toml_edit::table());
        }
        table = table
            .get_mut(parent)
            .and_then(Item::as_table_like_mut)
            .expect("just made a table");
    }
    let Some(value) = value else {
        table.remove(last);
        return;
    };
    let mut value: toml_edit::Value = value
        .to_string()
        .parse()
        .expect("a TOML value prints as TOML");
    match table.get_mut(last).and_then(Item::as_value_mut) {
        Some(old) => {
            *value.decor_mut() = old.decor().clone();
            *old = value;
        }
        None => {
            table.insert(last, Item::Value(value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Choice, Control, Setting};

    const PALETTE: Page = Page::new("Command palette", "Tabs");

    static SHOW_TABS: Setting<bool> = Setting {
        key: "palette.show_tabs",
        name: "Tabs",
        description: "",
        default: true,
        control: Control::Toggle,
    };

    static DAYS: Setting<u32> = Setting {
        key: "palette.active_days",
        name: "Active",
        description: "",
        default: 7,
        control: Control::Select(|_| vec![Choice::new(7, "7 days")]),
    };

    static DENSITY: Setting<String> = Setting {
        key: "density",
        name: "Density",
        description: "",
        default: String::new(),
        control: Control::Select(|_| Vec::new()),
    };

    fn settings(source: &str) -> (Settings, Vec<String>) {
        let mut settings = Settings {
            declared: vec![
                (PALETTE, &SHOW_TABS),
                (PALETTE, &DAYS),
                (Page::new("Appearance", "Size"), &DENSITY),
            ],
            ..Default::default()
        };
        let problems = settings.load(source);
        (settings, problems)
    }

    #[test]
    fn unknown_keys_and_bad_values_are_reported_and_the_rest_apply() {
        let (settings, problems) = settings(
            r#"
            density = "dense"

            [palette]
            show_tab = false
            active_days = "a week"
            "#,
        );
        assert_eq!(settings.value("density").unwrap().as_str(), Some("dense"));
        assert!(settings.value("palette.active_days").is_none());
        assert_eq!(problems.len(), 2);
        assert!(problems[0].starts_with("`palette.show_tab` isn't a setting"));
        assert!(problems[1].starts_with("`palette.active_days` "));
    }

    #[test]
    fn invalid_toml_keeps_the_last_good_values() {
        let (mut settings, _) = settings("density = \"dense\"");
        let problems = settings.load("density = ");
        assert_eq!(problems.len(), 1);
        assert_eq!(settings.value("density").unwrap().as_str(), Some("dense"));
    }

    #[test]
    fn edits_keep_comments_and_add_tables() {
        let (mut settings, _) = settings("# Mine\ndensity = \"dense\" # for the laptop\n");
        settings.edit("density", Some("calm".into()));
        settings.edit("palette.show_tabs", Some(false.into()));
        assert_eq!(
            settings.source(),
            "# Mine\ndensity = \"calm\" # for the laptop\n\n[palette]\nshow_tabs = false\n"
        );
        settings.edit("palette.show_tabs", None);
        assert!(settings.value("palette.show_tabs").is_none());
        assert!(!settings.source().contains("show_tabs"));
    }

    #[test]
    fn edits_to_a_broken_file_stay_in_memory() {
        let (mut settings, _) = settings("density = ");
        settings.edit("density", Some("dense".into()));
        assert_eq!(settings.source(), "density = ");
        assert_eq!(settings.value("density").unwrap().as_str(), Some("dense"));
    }
}

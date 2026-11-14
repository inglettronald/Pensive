//! One setting: its key in the file, what the palette and settings window show for it, and how
//! its value is read and changed.

use gpui::{App, SharedString, UpdateGlobal};
use serde::{Serialize, de::DeserializeOwned};

use crate::Settings;

/// A group of pages, such as Appearance, declared as a `static` in the crate that owns it.
#[derive(Debug, PartialEq, Eq)]
pub struct Category {
    name: &'static str,
    description: &'static str,
}

impl Category {
    /// Every category shows a description under its name, so an empty one fails to compile.
    ///
    /// ```compile_fail
    /// static PLUGINS: settings::Category = settings::Category::new("Plugins", "");
    /// ```
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        assert!(
            !description.is_empty(),
            "a settings category needs a description"
        );
        Self { name, description }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn description(&self) -> &'static str {
        self.description
    }
}

/// Where a setting is listed: a category, such as Appearance, and a page within it, such as Fonts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub category: &'static Category,
    pub name: &'static str,
}

impl Page {
    pub const fn new(category: &'static Category, name: &'static str) -> Self {
        Self { category, name }
    }
}

/// A setting, declared as a `static` in the crate that reads it.
pub struct Setting<T: 'static> {
    /// Its dotted path in `settings.toml`, like `palette.show_tabs`.
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub default: T,
    pub control: Control<T>,
}

pub enum Control<T: 'static> {
    /// On or off, for a `bool`.
    Toggle,
    /// Two to four short choices side by side.
    Segmented(fn(&App) -> Vec<Choice<T>>),
    /// A longer list of choices, or one only known at runtime.
    Select(fn(&App) -> Vec<Choice<T>>),
}

pub struct Choice<T> {
    pub value: T,
    pub label: SharedString,
}

impl<T> Choice<T> {
    pub fn new(value: T, label: impl Into<SharedString>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

/// What a setting's control shows right now.
#[derive(Clone, Debug, PartialEq)]
pub enum ControlState {
    Toggle(bool),
    Segmented {
        labels: Vec<SharedString>,
        selected: Option<usize>,
    },
    Select {
        label: SharedString,
    },
}

impl<T: Clone + PartialEq + Serialize + DeserializeOwned + Sync> Setting<T> {
    /// The user's value, or the default when they haven't set one or it isn't valid.
    pub fn get(&self, cx: &App) -> T {
        cx.try_global::<Settings>()
            .and_then(|settings| settings.value(self.key))
            .and_then(|value| value.clone().try_into().ok())
            .unwrap_or_else(|| self.default.clone())
    }

    /// Changes the value. Setting the default removes the key, so the file only holds changes.
    pub fn set(&self, value: T, cx: &mut App) {
        let value = (value != self.default)
            .then(|| toml::Value::try_from(value).expect("setting values serialize to TOML"));
        cx.default_global::<Settings>();
        Settings::update_global(cx, |settings, _| settings.edit(self.key, value));
    }

    /// What a control shows for `value`: a choice's label, On or Off, or the value as written.
    fn label(&self, value: &T, cx: &App) -> SharedString {
        if let Control::Toggle = self.control {
            return if as_bool(value) { "On" } else { "Off" }.into();
        }
        match self
            .choices(cx)
            .into_iter()
            .find(|choice| choice.value == *value)
        {
            Some(choice) => choice.label,
            None => match toml::Value::try_from(value) {
                Ok(toml::Value::String(text)) => text.into(),
                Ok(value) => value.to_string().into(),
                Err(_) => SharedString::default(),
            },
        }
    }

    fn choices(&self, cx: &App) -> Vec<Choice<T>> {
        match self.control {
            Control::Toggle => Vec::new(),
            Control::Segmented(choices) | Control::Select(choices) => choices(cx),
        }
    }
}

/// A setting of any type, for listing settings and changing them from a control.
pub trait AnySetting: Sync {
    fn key(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn state(&self, cx: &App) -> ControlState;
    /// Flips a toggle, or moves `step` choices over, wrapping around.
    fn step(&self, step: isize, cx: &mut App);
    /// Picks the choice at `index`.
    fn choose(&self, index: usize, cx: &mut App);
    /// The default's label, when the setting holds something else.
    fn changed_from(&self, cx: &App) -> Option<SharedString>;
    /// Goes back to the default, which removes the key from the file.
    fn reset(&self, cx: &mut App);
    /// Whether `value` can be this setting's value, and why not.
    fn check(&self, value: &toml::Value) -> Result<(), String>;
}

impl<T: Clone + PartialEq + Serialize + DeserializeOwned + Sync> AnySetting for Setting<T> {
    fn key(&self) -> &'static str {
        self.key
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn state(&self, cx: &App) -> ControlState {
        let value = self.get(cx);
        let choices = self.choices(cx);
        let selected = choices.iter().position(|choice| choice.value == value);
        match self.control {
            Control::Toggle => ControlState::Toggle(as_bool(&value)),
            Control::Segmented(_) => ControlState::Segmented {
                labels: choices.into_iter().map(|choice| choice.label).collect(),
                selected,
            },
            Control::Select(_) => ControlState::Select {
                label: self.label(&value, cx),
            },
        }
    }

    fn step(&self, step: isize, cx: &mut App) {
        let value = self.get(cx);
        if let Control::Toggle = self.control {
            let flipped = toml::Value::Boolean(!as_bool(&value));
            self.set(flipped.try_into().expect("a toggle holds a bool"), cx);
            return;
        }
        let choices = self.choices(cx);
        if choices.is_empty() {
            return;
        }
        let index = match choices.iter().position(|choice| choice.value == value) {
            Some(index) => (index as isize + step).rem_euclid(choices.len() as isize) as usize,
            None => 0,
        };
        self.choose(index, cx);
    }

    fn choose(&self, index: usize, cx: &mut App) {
        if let Some(choice) = self.choices(cx).into_iter().nth(index) {
            self.set(choice.value, cx);
        }
    }

    fn changed_from(&self, cx: &App) -> Option<SharedString> {
        (self.get(cx) != self.default).then(|| self.label(&self.default, cx))
    }

    fn reset(&self, cx: &mut App) {
        self.set(self.default.clone(), cx);
    }

    fn check(&self, value: &toml::Value) -> Result<(), String> {
        value
            .clone()
            .try_into::<T>()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn as_bool(value: &impl Serialize) -> bool {
    toml::Value::try_from(value)
        .ok()
        .and_then(|value| value.as_bool())
        .expect("a toggle holds a bool")
}

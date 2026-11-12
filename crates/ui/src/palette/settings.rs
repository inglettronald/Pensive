//! The palette's own settings.

use gpui::App;
use settings::{AnySetting, Choice, Control, Setting, Settings};

use super::Tab;

pub static SHOW_TABS: Setting<bool> = Setting {
    key: "palette.show_tabs",
    section: "Palette",
    name: "Tabs",
    description: "All, Active and Commands in the search field",
    default: true,
    control: Control::Toggle,
};

pub static OPEN_ON: Setting<Tab> = Setting {
    key: "palette.open_on",
    section: "Palette",
    name: "Ctrl K opens on",
    description: "The tab the palette starts on",
    default: Tab::All,
    control: Control::Segmented(|_| {
        Tab::ALL
            .into_iter()
            .zip(Tab::LABELS)
            .map(|(tab, label)| Choice::new(tab, label))
            .collect()
    }),
};

pub static ACTIVE_DAYS: Setting<u32> = Setting {
    key: "palette.active_days",
    section: "Palette",
    name: "Active looks back",
    description: "How old work can be and still show in Active",
    default: 7,
    control: Control::Select(|_| {
        [1, 3, 7, 14, 30]
            .map(|days| Choice::new(days, days_label(days)))
            .into()
    }),
};

pub static OPACITY: Setting<u32> = Setting {
    key: "palette.opacity",
    section: "Palette",
    name: "Opacity",
    description: "How solid the palette's background is over a window",
    default: 100,
    control: Control::Select(|_| percents([70, 80, 90, 100])),
};

pub static SCRIM: Setting<u32> = Setting {
    key: "palette.scrim",
    section: "Palette",
    name: "Scrim",
    description: "How much the window behind the palette dims",
    default: 50,
    control: Control::Segmented(|_| {
        let mut choices = percents([0, 25, 50, 70]);
        choices[0].label = "None".into();
        choices
    }),
};

fn percents<const N: usize>(steps: [u32; N]) -> Vec<Choice<u32>> {
    steps
        .map(|percent| Choice::new(percent, format!("{percent}%")))
        .into()
}

pub(crate) fn days_label(days: u32) -> String {
    match days {
        1 => "1 day".into(),
        days => format!("{days} days"),
    }
}

pub(crate) fn declare(cx: &mut App) {
    let settings: [&'static dyn AnySetting; 5] =
        [&SHOW_TABS, &OPEN_ON, &ACTIVE_DAYS, &OPACITY, &SCRIM];
    Settings::declare(settings, cx);
}

/// A fraction from a setting kept in percent.
pub(crate) fn fraction(setting: &Setting<u32>, cx: &App) -> f32 {
    setting.get(cx).min(100) as f32 / 100.
}

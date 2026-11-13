//! The palette's own settings.

use gpui::{App, Hsla, hsla};
use settings::{AnySetting, Choice, Control, Page, Setting, Settings};

use super::Tab;

pub(crate) static SHOW_TABS: Setting<bool> = Setting {
    key: "palette.show_tabs",
    name: "Tabs",
    description: "All, Active and Commands in the search field",
    default: true,
    control: Control::Toggle,
};

pub(crate) static OPEN_ON: Setting<Tab> = Setting {
    key: "palette.open_on",
    name: "Starting tab",
    description: "The tab the palette opens on",
    default: Tab::All,
    control: Control::Segmented(|_| {
        Tab::ALL
            .into_iter()
            .zip(Tab::LABELS)
            .map(|(tab, label)| Choice::new(tab, label))
            .collect()
    }),
};

pub(crate) static ACTIVE_DAYS: Setting<u32> = Setting {
    key: "palette.active_days",
    name: "Active looks back",
    description: "How old work can be and still show in Active",
    default: 7,
    control: Control::Select(|_| {
        [1, 3, 7, 14, 30]
            .map(|days| Choice::new(days, days_label(days)))
            .into()
    }),
};

pub(crate) static OPACITY: Setting<u32> = Setting {
    key: "palette.opacity",
    name: "Opacity",
    description: "How solid the palette's background is over a window",
    default: 100,
    control: Control::Select(|_| percents([70, 80, 90, 100])),
};

pub(crate) static SCRIM: Setting<u32> = Setting {
    key: "palette.scrim",
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
    let page = |name| Page::new("Command palette", name);
    let tabs: [&'static dyn AnySetting; 2] = [&SHOW_TABS, &OPEN_ON];
    Settings::declare(page("Tabs"), tabs, cx);
    Settings::declare(page("Active"), [&ACTIVE_DAYS as &dyn AnySetting], cx);
    let backdrop: [&'static dyn AnySetting; 2] = [&OPACITY, &SCRIM];
    Settings::declare(page("Backdrop"), backdrop, cx);
}

/// A fraction from a setting kept in percent.
pub(crate) fn fraction(setting: &Setting<u32>, cx: &App) -> f32 {
    setting.get(cx).min(100) as f32 / 100.
}

/// The color that dims the window behind the palette.
pub(crate) fn scrim(cx: &App) -> Hsla {
    hsla(0., 0., 0., fraction(&SCRIM, cx))
}

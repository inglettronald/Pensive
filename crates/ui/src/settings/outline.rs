//! What the settings view lists: each category with its icon and description, then its pages of
//! declared settings, in the order the rail shows them.

use components::IconName;
use gpui::App;
use settings::{AnySetting, Settings};

/// The categories Pensive knows, in the rail's order. A setting can name a category that isn't
/// here, and it's listed last with the gear for its icon.
const KNOWN: [(&str, IconName, &str); 3] = [
    ("Appearance", IconName::Palette, "Theme, size and fonts"),
    (
        "Command palette",
        IconName::Command,
        "Tabs, Active and the backdrop",
    ),
    ("Notices", IconName::Bell, "Toasts and the badge"),
];

pub(crate) struct Category {
    pub name: &'static str,
    pub icon: IconName,
    pub description: &'static str,
    pub pages: Vec<Page>,
}

pub(crate) struct Page {
    pub name: &'static str,
    pub settings: Vec<&'static dyn AnySetting>,
}

/// Every declared setting, by category and page.
pub(crate) fn outline(cx: &App) -> Vec<Category> {
    let mut categories: Vec<Category> = Vec::new();
    let declared = cx.try_global::<Settings>().into_iter();
    for (page, setting) in declared.flat_map(Settings::declared) {
        let category = match categories
            .iter()
            .position(|category| category.name == page.category)
        {
            Some(index) => &mut categories[index],
            None => {
                let (icon, description) = KNOWN
                    .iter()
                    .find(|(name, ..)| *name == page.category)
                    .map_or((IconName::Settings, ""), |(_, icon, description)| {
                        (*icon, *description)
                    });
                categories.push(Category {
                    name: page.category,
                    icon,
                    description,
                    pages: Vec::new(),
                });
                categories.last_mut().expect("just pushed")
            }
        };
        match category
            .pages
            .iter_mut()
            .find(|known| known.name == page.name)
        {
            Some(known) => known.settings.push(setting),
            None => category.pages.push(Page {
                name: page.name,
                settings: vec![setting],
            }),
        }
    }
    categories.sort_by_key(|category| {
        KNOWN
            .iter()
            .position(|(name, ..)| *name == category.name)
            .unwrap_or(KNOWN.len())
    });
    categories
}

//! What the settings view lists: each category with its icon and description, then its pages of
//! declared settings, in the order the rail shows them.

use components::IconName;
use gpui::App;
use settings::{AnySetting, Settings};

use crate::{editor, notices, palette, project};

/// The categories Pensive knows, in the rail's order, with their icons. A setting can name a
/// category that isn't here, and it's listed last with the gear for its icon.
static KNOWN: [(&settings::Category, IconName); 5] = [
    (&theme::APPEARANCE, IconName::Palette),
    (&palette::CATEGORY, IconName::Command),
    (&editor::CATEGORY, IconName::FileText),
    (&project::files_options::CATEGORY, IconName::Files),
    (&notices::CATEGORY, IconName::Bell),
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
        let declared = page.category;
        let category = match categories
            .iter()
            .position(|category| category.name == declared.name())
        {
            Some(index) => &mut categories[index],
            None => {
                let icon = KNOWN
                    .iter()
                    .find(|(known, _)| *known == declared)
                    .map_or(IconName::Settings, |(_, icon)| *icon);
                categories.push(Category {
                    name: declared.name(),
                    icon,
                    description: declared.description(),
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
            .position(|(known, _)| known.name() == category.name)
            .unwrap_or(KNOWN.len())
    });
    categories
}

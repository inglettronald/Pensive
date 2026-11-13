//! Which settings match a search: those in the category being read first, then every other
//! category's together, best match first.

use std::ops::Range;

use settings::AnySetting;

use super::outline::Category;
use crate::fuzzy::Fuzzy;

pub(crate) struct Hit {
    pub category: usize,
    pub page: usize,
    pub setting: &'static dyn AnySetting,
    /// Byte ranges of the setting's name that matched, empty when something else did.
    pub matches: Vec<Range<usize>>,
    score: u32,
}

pub(crate) struct Hits {
    pub here: Vec<Hit>,
    pub elsewhere: Vec<Hit>,
}

impl Hits {
    pub fn is_empty(&self) -> bool {
        self.here.is_empty() && self.elsewhere.is_empty()
    }

    pub fn in_category(&self, category: usize) -> bool {
        self.here
            .iter()
            .chain(&self.elsewhere)
            .any(|hit| hit.category == category)
    }
}

/// The settings matching `query`, with the ones in category `current` apart. A name or a page
/// matches fuzzily, and its letters are marked. A description has to contain the query, since
/// a long sentence matches almost any letters out of order.
pub(crate) fn search(outline: &[Category], query: &str, current: usize) -> Hits {
    let mut fuzzy = Fuzzy::new(query);
    let needle = query.to_lowercase();
    let mut hits = Hits {
        here: Vec::new(),
        elsewhere: Vec::new(),
    };
    for (category_index, category) in outline.iter().enumerate() {
        for (page_index, page) in category.pages.iter().enumerate() {
            let page_score = fuzzy.score(page.name).map(|(score, _)| score / 2);
            for setting in &page.settings {
                let name = fuzzy.score(setting.name());
                let described = setting
                    .description()
                    .to_lowercase()
                    .contains(&needle)
                    .then_some(1);
                let score = [
                    name.as_ref().map(|(score, _)| *score),
                    page_score,
                    described,
                ]
                .into_iter()
                .flatten()
                .max();
                let Some(score) = score else { continue };
                let hit = Hit {
                    category: category_index,
                    page: page_index,
                    setting: *setting,
                    matches: name.map(|(_, matches)| matches).unwrap_or_default(),
                    score,
                };
                if category_index == current {
                    hits.here.push(hit);
                } else {
                    hits.elsewhere.push(hit);
                }
            }
        }
    }
    hits.here.sort_by_key(|hit| std::cmp::Reverse(hit.score));
    hits.elsewhere
        .sort_by_key(|hit| std::cmp::Reverse(hit.score));
    // Each other category's matches stay together, the category with the best match first.
    let mut order: Vec<usize> = Vec::new();
    for hit in &hits.elsewhere {
        if !order.contains(&hit.category) {
            order.push(hit.category);
        }
    }
    hits.elsewhere
        .sort_by_key(|hit| order.iter().position(|category| *category == hit.category));
    hits
}

#[cfg(test)]
mod tests {
    use components::IconName;
    use settings::{Control, Setting};

    use super::*;
    use crate::settings::outline::Page;

    static TABS: Setting<bool> = Setting {
        key: "palette.show_tabs",
        name: "Tabs",
        description: "All, Active and Commands in the search field",
        default: true,
        control: Control::Toggle,
    };
    static TOASTS: Setting<bool> = Setting {
        key: "notices.toasts",
        name: "Toasts",
        description: "Show a toast when long work finishes",
        default: true,
        control: Control::Toggle,
    };
    static THEME: Setting<bool> = Setting {
        key: "theme",
        name: "Theme",
        description: "The colors",
        default: true,
        control: Control::Toggle,
    };

    fn outline() -> Vec<Category> {
        let category = |name, setting: &'static dyn AnySetting| Category {
            name,
            icon: IconName::Settings,
            description: "",
            pages: vec![Page {
                name: "Page",
                settings: vec![setting],
            }],
        };
        vec![
            category("Appearance", &THEME),
            category("Command palette", &TABS),
            category("Notices", &TOASTS),
        ]
    }

    #[test]
    fn the_category_being_read_ranks_first() {
        let hits = search(&outline(), "t", 2);
        let names = |hits: &[Hit]| {
            hits.iter()
                .map(|hit| hit.setting.name())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&hits.here), ["Toasts"]);
        assert!(names(&hits.elsewhere).contains(&"Tabs"));
        assert!(!names(&hits.elsewhere).contains(&"Toasts"));
    }

    #[test]
    fn each_other_category_stays_together() {
        let mut outline = outline();
        outline[2].pages[0].settings.push(&THEME);
        let hits = search(&outline, "t", 0);
        let categories: Vec<usize> = hits.elsewhere.iter().map(|hit| hit.category).collect();
        let mut grouped = categories.clone();
        grouped.dedup();
        assert_eq!(grouped.len(), 2, "{categories:?}");
    }

    #[test]
    fn a_description_matches_only_when_it_contains_the_query() {
        let hits = search(&outline(), "search field", 0);
        let found: Vec<_> = hits
            .elsewhere
            .iter()
            .map(|hit| hit.setting.name())
            .collect();
        assert_eq!(found, ["Tabs"]);
        assert!(hits.elsewhere[0].matches.is_empty());
        assert!(search(&outline(), "colrs", 0).is_empty());
    }
}

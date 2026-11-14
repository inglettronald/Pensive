//! The settings page beside the sidebar: every category's settings in one scroll, or a search's
//! matches.

use components::{
    Card, CategoryHeading, Icon, IconName, NavItem, NavList, NextCard, PageHeading, SettingRow,
};
use gpui::{AnyElement, App, Context, SharedString, div, prelude::*, px};
use settings::AnySetting;
use theme::{ActiveTheme, Typeset, dp};

use super::{
    Anchor, SettingsView, Spot,
    outline::Category,
    search::{Hit, Hits},
};
use crate::{setting_control::setting_control, view_callback, view_callback_with};

impl SettingsView {
    /// The search results, the home category's first.
    pub(super) fn results_list(
        &self,
        outline: &[Category],
        hits: &Hits,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let here = self.results(
            &hits.here,
            |hit| outline[hit.category].pages[hit.page].name,
            cx,
        );
        let elsewhere = self.results(&hits.elsewhere, |hit| outline[hit.category].name, cx);
        let colors = &cx.theme().colors;
        let density = cx.density();
        let label = |text: SharedString, icon: Option<IconName>| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .pt_2p5()
                .pb_1()
                .px(dp(18.))
                .typeset(density.label)
                .text_color(colors.text_faint)
                .children(icon.map(|icon| Icon::new(icon).size(dp(14.)).color(colors.text_faint)))
                .child(text.to_uppercase())
        };
        let home = &outline[self.home()];
        let both = !hits.here.is_empty() && !hits.elsewhere.is_empty();
        div()
            .flex()
            .flex_col()
            .when_some(here, |list, here| {
                list.child(label(format!("In {}", home.name).into(), Some(home.icon)))
                    .child(here)
            })
            .when(both, |list| {
                list.child(div().h(px(1.)).mx_4().mt_2p5().bg(colors.border_subtle))
            })
            .when_some(elsewhere, |list, elsewhere| {
                list.child(label("Other categories".into(), None))
                    .child(elsewhere)
            })
            .into_any_element()
    }

    /// `hits` as a list, each naming where it is on the right. Clicking one shows it in place.
    pub(super) fn results(
        &self,
        hits: &[Hit],
        place: impl Fn(&Hit) -> &'static str,
        cx: &mut Context<Self>,
    ) -> Option<NavList> {
        if hits.is_empty() {
            return None;
        }
        let items = hits
            .iter()
            .map(|hit| NavItem {
                label: hit.setting.name().into(),
                matches: hit.matches.clone(),
                trailing: Some(place(hit).into()),
            })
            .collect();
        let spots: Vec<Spot> = hits
            .iter()
            .map(|hit| Spot {
                category: hit.category,
                page: hit.page,
            })
            .collect();
        let id = if hits[0].category == self.home() {
            "here"
        } else {
            "elsewhere"
        };
        Some(NavList::new(id, items).on_select(view_callback_with(
            cx,
            move |view: &mut Self, index: usize, _, cx| view.show_after_search(spots[index], cx),
        )))
    }

    fn row(setting: &'static dyn AnySetting, hit: Option<&Hit>, cx: &App) -> SettingRow {
        SettingRow::new(
            setting.key(),
            setting.name(),
            setting.description(),
            setting_control(setting, &setting.state(cx)),
        )
        .matches(hit.map(|hit| hit.matches.clone()).unwrap_or_default())
        .changed_from(setting.changed_from(cx))
        .on_reset(move |_, cx| setting.reset(cx))
    }

    /// Every category with its pages, then the card that leads to the next category. Each is
    /// one child of the scroll, with its anchor.
    pub(super) fn every_page(
        &self,
        outline: &[Category],
        cx: &mut Context<Self>,
    ) -> Vec<(Anchor, AnyElement)> {
        let mut blocks = Vec::new();
        for (index, category) in outline.iter().enumerate() {
            blocks.push((
                Anchor::Category(index),
                CategoryHeading::new(category.icon, category.name, category.description)
                    .into_any_element(),
            ));
            for (page_index, page) in category.pages.iter().enumerate() {
                let heading = PageHeading::new(page.name);
                let heading = if page_index == 0 {
                    heading.first()
                } else {
                    heading
                };
                let rows = page
                    .settings
                    .iter()
                    .map(|setting| Self::row(*setting, None, cx));
                blocks.push((
                    Anchor::Page(Spot {
                        category: index,
                        page: page_index,
                    }),
                    div()
                        .child(heading)
                        .child(
                            div()
                                .mt_1p5()
                                .child(Card::new(("page", index * 100 + page_index)).rows(rows)),
                        )
                        .into_any_element(),
                ));
            }
            if let Some(next) = outline.get(index + 1) {
                let card = NextCard::new(("next", index), next.icon, next.name, next.description)
                    .on_click(view_callback(cx, move |view: &mut Self, _, cx| {
                        view.show_category(index + 1, cx)
                    }));
                blocks.push((Anchor::Other, card.into_any_element()));
            }
        }
        blocks
    }

    /// The matching settings, the home category's under its heading and the others' under
    /// their names, in the list's order.
    pub(super) fn matching(
        &self,
        outline: &[Category],
        hits: &Hits,
        cx: &mut Context<Self>,
    ) -> Vec<(Anchor, AnyElement)> {
        let colors = &cx.theme().colors;
        let density = cx.density();
        if hits.is_empty() {
            return vec![(
                Anchor::Other,
                div()
                    .pt_7()
                    .px(density.row_padding)
                    .typeset(density.body)
                    .text_color(colors.text_faint)
                    .child(format!("No settings match {}", self.query(cx)))
                    .into_any_element(),
            )];
        }
        let card = |id: &'static str, hits: &[&Hit], cx: &App| {
            Card::new(id).rows(hits.iter().map(|hit| Self::row(hit.setting, Some(hit), cx)))
        };
        let mut blocks = Vec::new();
        if !hits.here.is_empty() {
            let home = &outline[self.home()];
            let here: Vec<&Hit> = hits.here.iter().collect();
            blocks.push((
                Anchor::Other,
                div()
                    .child(CategoryHeading::new(home.icon, home.name, home.description))
                    .child(div().mt_2().child(card("here", &here, cx)))
                    .into_any_element(),
            ));
        }
        if !hits.elsewhere.is_empty() {
            blocks.push((
                Anchor::Other,
                div()
                    .mt_6()
                    .pt_3p5()
                    .border_t_1()
                    .border_color(colors.border_subtle)
                    .px(density.row_padding)
                    .typeset(density.label)
                    .text_color(colors.text_faint)
                    .child("OTHER CATEGORIES")
                    .into_any_element(),
            ));
            let mut start = 0;
            while start < hits.elsewhere.len() {
                let category = hits.elsewhere[start].category;
                let group: Vec<&Hit> = hits.elsewhere[start..]
                    .iter()
                    .take_while(|hit| hit.category == category)
                    .collect();
                start += group.len();
                let entry = &outline[category];
                blocks.push((
                    Anchor::Other,
                    div()
                        .child(CategoryHeading::new(entry.icon, entry.name, "").compact())
                        .child(div().mt_1p5().child(card(entry.name, &group, cx)))
                        .into_any_element(),
                ));
            }
        }
        blocks
    }
}

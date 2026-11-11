//! The command palette: a search field, grouped results, and the selected entry's key hints.

mod entry;
mod field;
#[cfg(feature = "mocks")]
mod mocks;

pub use entry::{Leading, PaletteEntry, TrailingText};
pub use field::PaletteChip;
#[cfg(feature = "mocks")]
pub(crate) use mocks::mocks;

use std::rc::Rc;

use gpui::{
    AnyElement, App, BoxShadow, ScrollHandle, SharedString, Window, div, hsla, prelude::*, px,
};
use theme::{ActiveTheme, Typeset};

use crate::{Callback, Icon, IconName, Kbd, TabStrip};
use field::Field;

/// A key and what pressing it does, shown in the footer.
#[derive(Clone)]
pub struct KeyHint {
    keys: SharedString,
    label: SharedString,
}

impl KeyHint {
    pub fn new(keys: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            keys: keys.into(),
            label: label.into(),
        }
    }
}

/// A group of results under a small label.
pub struct PaletteSection {
    label: SharedString,
    aside: Option<SharedString>,
    entries: Vec<PaletteEntry>,
}

impl PaletteSection {
    pub fn new(
        label: impl Into<SharedString>,
        entries: impl IntoIterator<Item = PaletteEntry>,
    ) -> Self {
        Self {
            label: label.into(),
            aside: None,
            entries: entries.into_iter().collect(),
        }
    }

    /// Gray text at the right end of the label, such as a count.
    pub fn aside(mut self, aside: impl Into<SharedString>) -> Self {
        self.aside = Some(aside.into());
        self
    }
}

#[derive(IntoElement)]
pub struct CommandPalette {
    chips: Vec<PaletteChip>,
    query: SharedString,
    placeholder: SharedString,
    input: Option<AnyElement>,
    tabs: Vec<SharedString>,
    tab: Option<usize>,
    empty: Option<(SharedString, SharedString)>,
    sections: Vec<PaletteSection>,
    selected: Option<usize>,
    scroll: Option<ScrollHandle>,
    overlay: bool,
    opacity: f32,
    on_tab: Option<Callback<usize>>,
    on_activate: Option<Callback<usize>>,
    on_settings: Option<Callback<()>>,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            chips: Vec::new(),
            query: SharedString::default(),
            placeholder: "Search".into(),
            input: None,
            tabs: Vec::new(),
            tab: None,
            empty: None,
            sections: Vec::new(),
            selected: None,
            scroll: None,
            overlay: false,
            opacity: 1.,
            on_tab: None,
            on_activate: None,
            on_settings: None,
        }
    }

    pub fn chip(mut self, chip: PaletteChip) -> Self {
        self.chips.push(chip);
        self
    }

    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = query.into();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The editable field the view owns. Without one, the query and placeholder show as text.
    pub fn input(mut self, input: impl IntoElement) -> Self {
        self.input = Some(input.into_any_element());
        self
    }

    /// The tabs Tab cycles through. With none selected, the strip dims.
    pub fn tabs(
        mut self,
        tabs: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: Option<usize>,
    ) -> Self {
        self.tabs = tabs.into_iter().map(Into::into).collect();
        self.tab = selected;
        self
    }

    /// Shown above the sections when nothing matches.
    pub fn empty(
        mut self,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
    ) -> Self {
        self.empty = Some((title.into(), detail.into()));
        self
    }

    pub fn section(mut self, section: PaletteSection) -> Self {
        self.sections.push(section);
        self
    }

    /// The selected entry, counted across every section.
    pub fn selected(mut self, index: usize) -> Self {
        self.selected = Some(index);
        self
    }

    /// Tracks the results' scroll position, so [`CommandPalette::reveal`] can move it.
    pub fn scroll(mut self, handle: ScrollHandle) -> Self {
        self.scroll = Some(handle);
        self
    }

    /// Scrolls the results tracked by `handle` just enough to show the entry at `selected`.
    /// `section_sizes` are the entry counts of the sections, in order.
    pub fn reveal(
        handle: &ScrollHandle,
        has_empty: bool,
        section_sizes: impl IntoIterator<Item = usize>,
        selected: usize,
    ) {
        let mut child = usize::from(has_empty);
        let mut first = 0;
        for size in section_sizes {
            child += 1;
            if selected < first + size {
                handle.scroll_to_item(child + selected - first);
                return;
            }
            child += size;
            first += size;
        }
    }

    /// Adds the shadow the palette casts over a window.
    pub fn overlay(mut self) -> Self {
        self.overlay = true;
        self
    }

    /// The background's opacity, from 0 to 1.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn on_tab(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_tab = Some(Rc::new(handler));
        self
    }

    /// Called with the index of an entry that was clicked.
    pub fn on_activate(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Called when the footer's gear is clicked, to open Settings.
    pub fn on_settings(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_settings = Some(Rc::new(move |(), window, cx| handler(window, cx)));
        self
    }

    fn footer_hints(&self) -> Vec<KeyHint> {
        let entry_hints = self
            .selected
            .and_then(|index| self.sections.iter().flat_map(|s| &s.entries).nth(index))
            .and_then(|entry| entry.hints.clone())
            .unwrap_or_else(|| vec![KeyHint::new("Enter", "Run")]);
        let mut hints = vec![KeyHint::new("↑↓", "Select")];
        hints.extend(entry_hints);
        hints
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let density = cx.density();
        let colors = &theme.colors;
        let hints = self.footer_hints();

        let tabs = (!self.tabs.is_empty()).then(|| {
            let strip = TabStrip::new("tabs", self.tabs, self.tab);
            match self.on_tab {
                Some(handler) => {
                    strip.on_select(move |index, window, cx| handler(index, window, cx))
                }
                None => strip,
            }
        });
        let field = Field {
            chips: self.chips,
            query: self.query,
            placeholder: self.placeholder,
            input: self.input,
            tabs,
        };

        let mut results: Vec<AnyElement> = Vec::new();
        let has_empty = self.empty.is_some();
        if let Some((title, detail)) = self.empty {
            results.push(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .pt_6()
                    .px_4()
                    .pb_3()
                    .child(
                        div().mb_1().child(
                            Icon::new(IconName::SearchX)
                                .size(px(20.))
                                .color(colors.text_faint),
                        ),
                    )
                    .child(
                        div()
                            .typeset(density.body)
                            .text_color(colors.text_strong)
                            .child(title),
                    )
                    .child(
                        div()
                            .typeset(density.small)
                            .text_color(colors.text_faint)
                            .child(detail),
                    )
                    .into_any_element(),
            );
        }
        let mut index = 0;
        for (position, section) in self.sections.into_iter().enumerate() {
            let top = if position == 0 && !has_empty {
                px(8.)
            } else {
                px(12.)
            };
            results.push(
                div()
                    .flex()
                    .flex_none()
                    .pt(top)
                    .px_2()
                    .pb_1()
                    .typeset(density.label)
                    .text_color(colors.text_faint)
                    .child(div().flex_1().child(section.label))
                    .children(
                        section
                            .aside
                            .map(|aside| div().font_weight(gpui::FontWeight::NORMAL).child(aside)),
                    )
                    .into_any_element(),
            );
            for entry in section.entries {
                let row_index = index;
                index += 1;
                let row = entry.render(row_index, self.selected == Some(row_index), theme, density);
                let row = match self.on_activate.clone() {
                    Some(handler) => {
                        row.on_click(move |_, window, cx| handler(row_index, window, cx))
                    }
                    None => row,
                };
                results.push(row.into_any_element());
            }
        }

        div()
            .w(px(576.))
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(density.radius_lg)
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface_panel.opacity(self.opacity))
            .when(self.overlay, |palette| {
                palette.shadow(vec![
                    BoxShadow::new(px(0.), px(16.), hsla(0., 0., 0., 0.6)).blur_radius(px(48.)),
                ])
            })
            .font_family(theme.fonts.ui.clone())
            .text_color(colors.text)
            .child(field.render(theme, density))
            .child(
                div()
                    .id("results")
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .p_1()
                    .overflow_y_scroll()
                    .when_some(self.scroll, |list, handle| list.track_scroll(&handle))
                    .children(results),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_4()
                    .h(density.row_height + px(8.))
                    .pl_3()
                    .pr_1p5()
                    .border_t_1()
                    .border_color(colors.border_subtle)
                    .typeset(density.small)
                    .text_color(colors.text_faint)
                    .children(hints.into_iter().map(|hint| {
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .whitespace_nowrap()
                            .child(Kbd::new(hint.keys))
                            .child(hint.label)
                    }))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("settings")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(24.))
                            .rounded(density.radius_sm)
                            .child(
                                Icon::new(IconName::Settings)
                                    .size(px(14.))
                                    .color(colors.text_faint),
                            )
                            .when_some(self.on_settings, |gear, handler| {
                                gear.on_click(move |_, window, cx| handler((), window, cx))
                            }),
                    ),
            )
    }
}

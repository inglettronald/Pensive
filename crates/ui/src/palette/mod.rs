//! The command palette's view: the query, tab, keyword and selection, and what the keys do.

mod commands;
mod fuzzy;
mod results;

use components::{CommandPalette, PaletteChip, PaletteSection};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, ScrollHandle, Subscription, Window,
    actions, div, prelude::*,
};
use model::{ProjectId, Projects};

use crate::{
    key_binding,
    text_input::{self, Changed, TextInput},
    view_callback_with,
};
use results::{Query, Results, results};

actions!(
    command_palette,
    [
        SelectNext,
        SelectPrevious,
        NextTab,
        PreviousTab,
        Confirm,
        ConfirmSecondary,
        Dismiss,
    ]
);

const CONTEXT: &str = "CommandPalette";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        key_binding("down", SelectNext, Some(CONTEXT), cx),
        key_binding("up", SelectPrevious, Some(CONTEXT), cx),
        key_binding("tab", NextTab, Some(CONTEXT), cx),
        key_binding("shift-tab", PreviousTab, Some(CONTEXT), cx),
        key_binding("enter", Confirm, Some(CONTEXT), cx),
        key_binding("secondary-enter", ConfirmSecondary, Some(CONTEXT), cx),
        key_binding("escape", Dismiss, Some(CONTEXT), cx),
    ]);
}

/// The three fixed tabs. Plugins add keywords, not tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    All,
    Active,
    Commands,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::All, Tab::Active, Tab::Commands];
    const LABELS: [&str; 3] = ["All", "Active", "Commands"];

    fn index(self) -> usize {
        Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }

    fn placeholder(self) -> &'static str {
        match self {
            Tab::All => "Search, or paste a clone URL",
            Tab::Active => "Filter recent work",
            Tab::Commands => "Search commands",
        }
    }
}

/// A keyword narrowing the results. While one is set, no tab is selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Keyword {
    /// One project's branches and worktrees, or the project list until one is picked.
    Project(Option<ProjectId>),
    /// Branches and worktrees in every project.
    Branch,
}

/// A keyword typed at the start of the query, which becomes a chip or switches tab.
#[derive(Debug, PartialEq, Eq)]
enum Typed {
    Keyword(Keyword),
    Tab(Tab),
}

impl Typed {
    /// The keyword `text` starts with, and the query after it.
    fn parse(text: &str) -> Option<(Typed, &str)> {
        [
            ("project:", Typed::Keyword(Keyword::Project(None))),
            ("branch:", Typed::Keyword(Keyword::Branch)),
            ("active:", Typed::Tab(Tab::Active)),
            (">", Typed::Tab(Tab::Commands)),
        ]
        .into_iter()
        .find_map(|(prefix, typed)| Some((typed, text.strip_prefix(prefix)?.trim_start())))
    }
}

/// What choosing a row does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Open {
        project: ProjectId,
        new_window: bool,
    },
    Narrow(Keyword),
    /// Drops the keyword and keeps the query, to search everywhere.
    ClearKeyword,
    /// Not wired up yet.
    None,
}

pub enum PaletteEvent {
    Open {
        project: ProjectId,
        new_window: bool,
    },
    Dismiss,
}

pub struct PaletteView {
    projects: Entity<Projects>,
    current: Option<ProjectId>,
    input: Entity<TextInput>,
    tab: Tab,
    keyword: Option<Keyword>,
    selected: usize,
    scroll: ScrollHandle,
    overlay: bool,
    _subscription: Subscription,
}

impl EventEmitter<PaletteEvent> for PaletteView {}

impl PaletteView {
    pub fn new(projects: Entity<Projects>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(TextInput::new);
        let subscription = cx.subscribe(&input, |palette, _, _: &Changed, cx| {
            palette.query_changed(cx)
        });
        let mut palette = Self {
            projects,
            current: None,
            input,
            tab: Tab::All,
            keyword: None,
            selected: 0,
            scroll: ScrollHandle::new(),
            overlay: false,
            _subscription: subscription,
        };
        palette.update_placeholder(cx);
        palette
    }

    /// Starts over on `tab`, for the project open in this window. `overlay` adds the shadow the
    /// palette casts over a window.
    pub fn reset(
        &mut self,
        current: Option<ProjectId>,
        tab: Tab,
        overlay: bool,
        cx: &mut Context<Self>,
    ) {
        self.current = current;
        self.tab = tab;
        self.keyword = None;
        self.selected = 0;
        self.overlay = overlay;
        self.input.update(cx, |input, cx| input.set_text("", cx));
        self.update_placeholder(cx);
    }

    pub fn set_keyword(&mut self, keyword: Option<Keyword>, cx: &mut Context<Self>) {
        self.keyword = keyword;
        self.selected = 0;
        self.update_placeholder(cx);
        cx.notify();
    }

    pub fn set_query(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_text(text.to_string(), cx));
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        self.keyword = None;
        self.selected = 0;
        self.update_placeholder(cx);
        cx.notify();
    }

    /// Turns a keyword typed at the start of the query into its chip or tab.
    fn query_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().clone();
        if let Some((typed, rest)) = Typed::parse(&text) {
            match typed {
                Typed::Tab(tab) => self.set_tab(tab, cx),
                Typed::Keyword(keyword) => self.set_keyword(Some(keyword), cx),
            }
            self.set_query(rest, cx);
        }
        self.selected = 0;
        self.scroll.scroll_to_item(0);
        cx.notify();
    }

    fn update_placeholder(&mut self, cx: &mut Context<Self>) {
        let placeholder = match &self.keyword {
            Some(Keyword::Project(Some(_))) => "Filter branches and worktrees",
            Some(Keyword::Project(None)) => "Search projects",
            Some(Keyword::Branch) => "Search branches and worktrees",
            None => self.tab.placeholder(),
        };
        self.input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
    }

    fn results(&self, cx: &App) -> Results {
        let text = self.input.read(cx).text();
        results(&Query {
            projects: self.projects.read(cx),
            current: self.current,
            tab: self.tab,
            keyword: self.keyword.as_ref(),
            text,
        })
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(1, cx);
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(-1, cx);
    }

    /// Moves the selection by `step` rows, wrapping around, and scrolls it into view.
    fn move_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        let results = self.results(cx);
        let sizes: Vec<usize> = results.sections.iter().map(|s| s.items.len()).collect();
        let count = sizes.iter().sum::<usize>() as isize;
        if count == 0 {
            return;
        }
        self.selected = (self.selected as isize + step).rem_euclid(count) as usize;
        CommandPalette::reveal(&self.scroll, results.empty.is_some(), sizes, self.selected);
        cx.notify();
    }

    fn next_tab(&mut self, _: &NextTab, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_tab(1, cx);
    }

    fn previous_tab(&mut self, _: &PreviousTab, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_tab(-1, cx);
    }

    /// Moves `step` tabs over, wrapping around. With a keyword set, Tab drops it and goes to All.
    fn cycle_tab(&mut self, step: isize, cx: &mut Context<Self>) {
        let tab = match self.keyword {
            Some(_) => Tab::All,
            None => {
                let index = (self.tab.index() as isize + step).rem_euclid(Tab::ALL.len() as isize);
                Tab::ALL[index as usize]
            }
        };
        self.set_tab(tab, cx);
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        self.activate(self.selected, false, cx);
    }

    fn confirm_secondary(&mut self, _: &ConfirmSecondary, _: &mut Window, cx: &mut Context<Self>) {
        self.activate(self.selected, true, cx);
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(PaletteEvent::Dismiss);
    }

    fn activate(&mut self, index: usize, secondary: bool, cx: &mut Context<Self>) {
        let Some(item) = self.results(cx).into_items().nth(index) else {
            return;
        };
        let action = if secondary {
            item.secondary
        } else {
            item.action
        };
        match action {
            Action::Open {
                project,
                new_window,
            } => cx.emit(PaletteEvent::Open {
                project,
                new_window,
            }),
            Action::Narrow(keyword) => {
                self.set_keyword(Some(keyword), cx);
                if self.keyword != Some(Keyword::Project(None))
                    && self.keyword != Some(Keyword::Branch)
                {
                    self.set_query("", cx);
                }
            }
            Action::ClearKeyword => self.set_keyword(None, cx),
            Action::None => {}
        }
    }

    /// → at the end of the query applies the selected row's keyword, such as `project:`.
    fn narrow(&mut self, _: &text_input::Right, _: &mut Window, cx: &mut Context<Self>) {
        let at_end = self.input.read(cx).cursor_at_end();
        let narrow = at_end
            .then(|| self.results(cx).into_items().nth(self.selected))
            .flatten()
            .and_then(|item| item.narrow);
        match narrow {
            Some(keyword) => {
                self.set_keyword(Some(keyword), cx);
                self.set_query("", cx);
            }
            None => cx.propagate(),
        }
    }

    /// Backspace in an empty query removes the keyword.
    fn remove_keyword(
        &mut self,
        _: &text_input::Backspace,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.keyword.is_some() && self.input.read(cx).text().is_empty() {
            self.set_keyword(None, cx);
        } else {
            cx.propagate();
        }
    }
}

impl Focusable for PaletteView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for PaletteView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let results = self.results(cx);
        let count = results
            .sections
            .iter()
            .map(|s| s.items.len())
            .sum::<usize>();
        let selected = self.selected.min(count.saturating_sub(1));
        let chip = self.keyword.as_ref().map(|keyword| match keyword {
            Keyword::Project(Some(id)) => {
                let name = self
                    .projects
                    .read(cx)
                    .project(*id)
                    .map(|project| project.name.clone())
                    .unwrap_or_default();
                PaletteChip::new("project:").value(name).removable()
            }
            Keyword::Project(None) => PaletteChip::new("project:").removable(),
            Keyword::Branch => PaletteChip::new("branch:").removable(),
        });
        let tab = self.keyword.is_none().then(|| self.tab.index());
        let on_tab = view_callback_with(cx, |palette: &mut Self, index: usize, _, cx| {
            palette.set_tab(Tab::ALL[index], cx)
        });
        let on_activate = view_callback_with(cx, |palette: &mut Self, index: usize, _, cx| {
            palette.activate(index, false, cx)
        });

        let mut palette = CommandPalette::new()
            .input(self.input.clone())
            .scroll(self.scroll.clone())
            .tabs(Tab::LABELS, tab)
            .on_tab(on_tab)
            .on_activate(on_activate);
        if let Some(chip) = chip {
            palette = palette.chip(chip);
        }
        if let Some((title, detail)) = results.empty {
            palette = palette.empty(title, detail);
        }
        for section in results.sections {
            let mut palette_section = PaletteSection::new(
                section.label,
                section.items.into_iter().map(|item| item.entry),
            );
            if let Some(aside) = section.aside {
                palette_section = palette_section.aside(aside);
            }
            palette = palette.section(palette_section);
        }
        if count > 0 {
            palette = palette.selected(selected);
        }
        if self.overlay {
            palette = palette.overlay();
        }

        div()
            .flex()
            .flex_col()
            .min_h_0()
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::confirm_secondary))
            .on_action(cx.listener(Self::dismiss))
            .capture_action(cx.listener(Self::narrow))
            .capture_action(cx.listener(Self::remove_keyword))
            .child(palette)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_keyword_becomes_a_chip_and_keeps_the_rest() {
        assert_eq!(
            Typed::parse("project: multi"),
            Some((Typed::Keyword(Keyword::Project(None)), "multi"))
        );
        assert_eq!(
            Typed::parse(">fetch"),
            Some((Typed::Tab(Tab::Commands), "fetch"))
        );
        assert_eq!(Typed::parse("active:"), Some((Typed::Tab(Tab::Active), "")));
        assert_eq!(Typed::parse("projects"), None);
    }
}

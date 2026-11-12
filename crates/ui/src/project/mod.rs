//! The open project: the branch timeline in the sidebar and the selected file's diff beside it.

mod files;
mod main_view;
mod sidebar;
mod tree;

use std::time::Duration;

use gpui::{
    App, ClipboardItem, Context, Entity, FocusHandle, Focusable, SharedString, Subscription, Task,
    Window, actions, div, prelude::*,
};
use model::{Changes, FileChange};

use crate::{key_binding, text_input::TextInput};

actions!(project, [CopyPath, Commit]);

const CONTEXT: &str = "ProjectView";
/// How long the pointer rests on a file before its path shows.
const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        key_binding("secondary-shift-c", CopyPath, Some(CONTEXT), cx),
        key_binding("secondary-enter", Commit, Some(CONTEXT), cx),
    ]);
}

/// How the sidebar lists files, for the uncommitted work and any commit that's open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesView {
    List,
    Tree,
    /// Each file with the functions, types and headings its hunks touch.
    Symbols,
}

/// Which list a file is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Uncommitted,
    Commit(SharedString),
}

/// A file in one of the sidebar's lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileKey {
    source: Source,
    index: usize,
}

pub struct ProjectView {
    changes: Entity<Changes>,
    view: FilesView,
    uncommitted_open: bool,
    open_commit: Option<SharedString>,
    selected: Option<FileKey>,
    /// A symbol of the selected file, picked in the symbol view.
    selected_symbol: Option<usize>,
    /// Files whose symbols are hidden in the symbol view.
    folded: Vec<FileKey>,
    hovered: Option<FileKey>,
    tooltip: Option<FileKey>,
    tooltip_delay: Option<Task<()>>,
    message: Entity<TextInput>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

impl ProjectView {
    pub fn new(changes: Entity<Changes>, cx: &mut Context<Self>) -> Self {
        let message = cx.new(|cx| {
            let mut input = TextInput::new(cx);
            input.set_placeholder("Commit message", cx);
            input
        });
        let selected = (!changes.read(cx).uncommitted.is_empty()).then_some(FileKey {
            source: Source::Uncommitted,
            index: 0,
        });
        let subscription = cx.observe(&changes, |_, _, cx| cx.notify());
        Self {
            changes,
            view: FilesView::List,
            uncommitted_open: true,
            open_commit: None,
            selected,
            selected_symbol: None,
            folded: Vec::new(),
            hovered: None,
            tooltip: None,
            tooltip_delay: None,
            message,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
        }
    }

    pub fn set_view(&mut self, view: FilesView, cx: &mut Context<Self>) {
        self.view = view;
        self.selected_symbol = None;
        cx.notify();
    }

    pub fn set_uncommitted_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.uncommitted_open = open;
        cx.notify();
    }

    /// Opens the commit with `sha`, or closes it when it's already open. Opening one folds the
    /// uncommitted files and selects its first file.
    pub fn toggle_commit(&mut self, sha: SharedString, cx: &mut Context<Self>) {
        if self.open_commit.as_ref() == Some(&sha) {
            self.open_commit = None;
        } else {
            let has_files = self
                .changes
                .read(cx)
                .commit(&sha)
                .is_some_and(|commit| !commit.files.is_empty());
            self.uncommitted_open = false;
            if has_files {
                self.select(
                    FileKey {
                        source: Source::Commit(sha.clone()),
                        index: 0,
                    },
                    None,
                    cx,
                );
            }
            self.open_commit = Some(sha);
        }
        cx.notify();
    }

    pub fn set_all_staged(&mut self, staged: bool, cx: &mut Context<Self>) {
        self.changes.update(cx, |changes, cx| {
            changes.set_all_staged(staged);
            cx.notify();
        });
    }

    /// Shows the path of the `index`th uncommitted file at once, as if the pointer had rested
    /// on it.
    pub fn show_path(&mut self, index: usize, cx: &mut Context<Self>) {
        let key = FileKey {
            source: Source::Uncommitted,
            index,
        };
        self.hovered = Some(key.clone());
        self.tooltip = Some(key);
        cx.notify();
    }

    /// Picks the `index`th symbol of the selected file, as clicking it in the symbol view does.
    pub fn select_symbol(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(key) = self.selected.clone() {
            self.select(key, Some(index), cx);
        }
    }

    fn select(&mut self, key: FileKey, symbol: Option<usize>, cx: &mut Context<Self>) {
        self.selected = Some(key);
        self.selected_symbol = symbol;
        cx.notify();
    }

    fn toggle_folded(&mut self, key: FileKey, cx: &mut Context<Self>) {
        match self.folded.iter().position(|folded| *folded == key) {
            Some(position) => {
                self.folded.remove(position);
            }
            None => self.folded.push(key),
        }
        cx.notify();
    }

    fn toggle_staged(&mut self, index: usize, cx: &mut Context<Self>) {
        self.changes.update(cx, |changes, cx| {
            if let Some(file) = changes.uncommitted.get(index) {
                let path = file.path.clone();
                let staged = !file.staged;
                changes.set_staged(&path, staged);
                cx.notify();
            }
        });
    }

    /// Stages everything, or unstages everything once all of it is staged.
    fn toggle_all_staged(&mut self, cx: &mut Context<Self>) {
        let changes = self.changes.read(cx);
        let all = changes.staged_count() == changes.uncommitted.len();
        self.set_all_staged(!all, cx);
    }

    /// Starts the delay before the path shows, or hides it when the pointer leaves.
    fn hover(&mut self, key: FileKey, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            if self.hovered.as_ref() == Some(&key) {
                self.hovered = None;
                self.tooltip = None;
                self.tooltip_delay = None;
                cx.notify();
            }
            return;
        }
        self.hovered = Some(key.clone());
        self.tooltip_delay = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOOLTIP_DELAY).await;
            this.update(cx, |this, cx| {
                if this.hovered.as_ref() == Some(&key) {
                    this.tooltip = Some(key);
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn file(&self, key: &FileKey, cx: &App) -> Option<FileChange> {
        let changes = self.changes.read(cx);
        let files = match &key.source {
            Source::Uncommitted => &changes.uncommitted,
            Source::Commit(sha) => &changes.commit(sha)?.files,
        };
        files.get(key.index).cloned()
    }

    fn copy_path(&mut self, _: &CopyPath, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.tooltip.as_ref().and_then(|key| self.file(key, cx)) else {
            cx.propagate();
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(file.path.display().to_string()));
    }

    /// Committing waits on a Git backend.
    fn commit(&mut self, _: &Commit, _window: &mut Window, _cx: &mut Context<Self>) {}

    /// Wraps a handler for components, which call back with only the window and app.
    fn handler(
        &self,
        cx: &Context<Self>,
        handler: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let handler = self.handler_with(cx, move |this, (), cx| handler(this, cx));
        move |window, cx| handler((), window, cx)
    }

    /// [`Self::handler`] for components that call back with a value, such as a selected index.
    fn handler_with<T: 'static>(
        &self,
        cx: &Context<Self>,
        handler: impl Fn(&mut Self, T, &mut Context<Self>) + 'static,
    ) -> impl Fn(T, &mut Window, &mut App) + 'static {
        let view = cx.entity().downgrade();
        move |value, _, cx| {
            view.update(cx, |this, cx| handler(this, value, cx)).ok();
        }
    }
}

impl Focusable for ProjectView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ProjectView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let changes = self.changes.read(cx).clone();
        div()
            .id("project")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::copy_path))
            .on_action(cx.listener(Self::commit))
            .flex()
            .flex_1()
            .min_h_0()
            .child(self.render_sidebar(&changes, cx))
            .child(self.render_main_view(cx))
    }
}

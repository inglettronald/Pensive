//! The open project: the branch timeline in the sidebar and the selected file's diff beside it.

mod files;
mod main_view;
#[cfg(feature = "mocks")]
mod mocks;
mod sidebar;
mod tree;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use components::IconName;
use gpui::{
    App, ClipboardItem, Context, Entity, FocusHandle, Focusable, SharedString, Subscription, Task,
    Window, actions, div, prelude::*,
};
use model::{Changes, FileChange, Notice, ProjectId, Projects, Staging};
use strum::VariantArray;

use crate::{key_binding, text_input::TextInput};

#[cfg(feature = "mocks")]
pub(crate) use mocks::mocks;

actions!(project, [CopyPath, CommitStaged]);

const CONTEXT: &str = "ProjectView";
/// How long the pointer rests on a file before its path shows.
const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
/// How long the tooltip says the path was copied before showing the path again.
const COPIED_FOR: Duration = Duration::from_secs(1);
/// How long a notice stays on the control it's about.
const NOTICE_FOR: Duration = Duration::from_secs(3);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        key_binding("secondary-shift-c", CopyPath, Some(CONTEXT), cx),
        key_binding("secondary-enter", CommitStaged, Some(CONTEXT), cx),
    ]);
}

/// How the sidebar lays out files, for the uncommitted work and any commit that's open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, VariantArray)]
pub enum FileLayout {
    List,
    Tree,
    /// Each file with the functions, types and headings its hunks touch.
    Symbols,
}

impl FileLayout {
    /// The icon on its segment of the switcher.
    fn icon(self) -> IconName {
        match self {
            FileLayout::List => IconName::List,
            FileLayout::Tree => IconName::ListTree,
            FileLayout::Symbols => IconName::Braces,
        }
    }
}

/// A sidebar control that answers with a notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Fetch,
    Pull,
    Push,
    Commit,
}

impl Control {
    /// None of them has a Git backend behind it yet.
    fn notice(self) -> Notice {
        Notice::not_wired(match self {
            Control::Fetch => "Fetch",
            Control::Pull => "Pull",
            Control::Push => "Push",
            Control::Commit => "Commit",
        })
    }
}

/// Which list a file is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Uncommitted,
    Commit(SharedString),
}

/// A file in one of the sidebar's lists, by its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileKey {
    source: Source,
    path: PathBuf,
}

pub struct ProjectView {
    projects: Entity<Projects>,
    /// The worktree shown: for now, always the project's main one.
    worktree: PathBuf,
    layout: FileLayout,
    uncommitted_open: bool,
    open_commit: Option<SharedString>,
    selected: Option<FileKey>,
    /// A symbol of the selected file, picked in the symbol layout.
    selected_symbol: Option<usize>,
    /// Files whose symbols are hidden in the symbol layout.
    folded: Vec<FileKey>,
    hovered: Option<FileKey>,
    hovered_commit: Option<SharedString>,
    tooltip: Option<FileKey>,
    tooltip_delay: Option<Task<()>>,
    /// Set after Ctrl Shift C, until the tooltip goes back to the path.
    copied: Option<Task<()>>,
    /// What the control used last had to say, until the task clears it.
    notice: Option<(Control, Notice, Task<()>)>,
    message: Entity<TextInput>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

impl ProjectView {
    /// The view on `project`'s main worktree. Its changes are read the first time it's shown.
    pub fn new(projects: Entity<Projects>, project: ProjectId, cx: &mut Context<Self>) -> Self {
        let message = cx.new(|cx| {
            let mut input = TextInput::new(cx);
            input.set_placeholder("Commit message", cx);
            input
        });
        let worktree = projects
            .read(cx)
            .project(project)
            .map(|project| project.repository.root.clone())
            .unwrap_or_default();
        if projects.read(cx).changes(&worktree).is_none() {
            // A backend that can't read changes yet leaves the timeline empty.
            projects
                .update(cx, |projects, cx| {
                    projects.read_changes(worktree.clone(), cx)
                })
                .detach();
        }
        let subscription = cx.observe(&projects, |_, _, cx| cx.notify());
        let mut view = Self {
            projects,
            worktree,
            layout: FileLayout::List,
            uncommitted_open: true,
            open_commit: None,
            selected: None,
            selected_symbol: None,
            folded: Vec::new(),
            hovered: None,
            hovered_commit: None,
            tooltip: None,
            tooltip_delay: None,
            copied: None,
            notice: None,
            message,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
        };
        view.selected = view.changes(cx).uncommitted.first().map(|file| FileKey {
            source: Source::Uncommitted,
            path: file.path.clone(),
        });
        view
    }

    /// The worktree's changes as cached, or none until they've been read.
    fn changes(&self, cx: &App) -> Changes {
        self.projects
            .read(cx)
            .changes(&self.worktree)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_layout(&mut self, layout: FileLayout, cx: &mut Context<Self>) {
        self.layout = layout;
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
            let first = self
                .changes(cx)
                .commit(&sha)
                .and_then(|commit| commit.files.first().map(|file| file.path.clone()));
            self.uncommitted_open = false;
            if let Some(path) = first {
                self.select(
                    FileKey {
                        source: Source::Commit(sha.clone()),
                        path,
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
        let worktree = self.worktree.clone();
        self.projects.update(cx, |projects, cx| {
            projects.set_all_staged(&worktree, staged, cx)
        });
    }

    /// Shows the path of the `index`th uncommitted file at once, as if the pointer had rested
    /// on it.
    #[cfg(feature = "mocks")]
    pub fn show_path(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(file) = self.changes(cx).uncommitted.get(index).cloned() else {
            return;
        };
        let key = FileKey {
            source: Source::Uncommitted,
            path: file.path,
        };
        self.hovered = Some(key.clone());
        self.tooltip = Some(key);
        cx.notify();
    }

    /// Shows the path of the `index`th uncommitted file as just copied.
    #[cfg(feature = "mocks")]
    pub fn show_copied(&mut self, index: usize, cx: &mut Context<Self>) {
        self.show_path(index, cx);
        self.copied = Some(Task::ready(()));
    }

    /// Shows the notice `control` answers with, as if it had just been used.
    #[cfg(feature = "mocks")]
    pub fn show_notice(&mut self, control: Control, cx: &mut Context<Self>) {
        self.notice = Some((control, control.notice(), Task::ready(())));
        cx.notify();
    }

    /// Picks the `index`th symbol of the selected file, as clicking it in the symbol layout does.
    #[cfg(feature = "mocks")]
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

    /// Stages all of the file at `path`, or unstages it once it's fully staged.
    fn toggle_staged(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(staging) = self.changes(cx).file(path).map(FileChange::staging) else {
            return;
        };
        let worktree = self.worktree.clone();
        self.projects.update(cx, |projects, cx| {
            projects.set_staged(&worktree, path, staging != Staging::Staged, cx)
        });
    }

    /// Stages everything, or unstages everything once all of it is staged.
    fn toggle_all_staged(&mut self, cx: &mut Context<Self>) {
        let all = self.changes(cx).staging() == Staging::Staged;
        self.set_all_staged(!all, cx);
    }

    /// Starts the delay before the path shows, or hides it when the pointer leaves.
    fn hover(&mut self, key: FileKey, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            if self.hovered.as_ref() == Some(&key) {
                self.hovered = None;
                self.tooltip = None;
                self.tooltip_delay = None;
                self.copied = None;
                cx.notify();
            }
            return;
        }
        // The new row can report hover before the old one reports leaving, so the old row's
        // path must go now, or Ctrl Shift C would copy it.
        if self.tooltip.as_ref().is_some_and(|tooltip| *tooltip != key) {
            self.tooltip = None;
            self.copied = None;
            cx.notify();
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

    /// Tracks the commit title under the pointer, whose fade must end in the hover color.
    fn hover_commit(&mut self, sha: SharedString, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.hovered_commit = Some(sha);
        } else if self.hovered_commit.as_ref() == Some(&sha) {
            self.hovered_commit = None;
        }
        cx.notify();
    }

    fn file(&self, key: &FileKey, cx: &App) -> Option<FileChange> {
        let changes = self.changes(cx);
        let files = match &key.source {
            Source::Uncommitted => &changes.uncommitted,
            Source::Commit(sha) => &changes.commit(sha)?.files,
        };
        files.iter().find(|file| file.path == key.path).cloned()
    }

    fn copy_path(&mut self, _: &CopyPath, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.tooltip.as_ref().and_then(|key| self.file(key, cx)) else {
            cx.propagate();
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(file.path.display().to_string()));
        self.copied = Some(clear_after(COPIED_FOR, |this| this.copied = None, cx));
        cx.notify();
    }

    fn use_control(&mut self, control: Control, cx: &mut Context<Self>) {
        let clear = clear_after(NOTICE_FOR, |this| this.notice = None, cx);
        self.notice = Some((control, control.notice(), clear));
        cx.notify();
    }

    fn commit_staged(&mut self, _: &CommitStaged, _window: &mut Window, cx: &mut Context<Self>) {
        self.use_control(Control::Commit, cx);
    }
}

/// Runs `clear` on the view once `duration` has passed, unless the task is dropped first.
fn clear_after(
    duration: Duration,
    clear: fn(&mut ProjectView),
    cx: &mut Context<ProjectView>,
) -> Task<()> {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(duration).await;
        this.update(cx, |this, cx| {
            clear(this);
            cx.notify();
        })
        .ok();
    })
}

impl Focusable for ProjectView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ProjectView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let changes = self.changes(cx);
        div()
            .id("project")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::copy_path))
            .on_action(cx.listener(Self::commit_staged))
            .flex()
            .flex_1()
            .min_h_0()
            .child(self.render_sidebar(&changes, cx))
            .child(self.render_main_view(cx))
    }
}

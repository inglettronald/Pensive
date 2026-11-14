//! A changed file open in the main view: its text, the committed text it's compared with, and,
//! for an uncommitted file, editing with undo and saving it back to the worktree.

use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use gpui::{Context, EventEmitter, Task};
use ropey::Rope;

use crate::{
    Backend, FileKey, FileVersions, GitBackend, LineDiff, Notice, Notices, Origin, Severity,
    Source, Version, diff::line_ranges, queue::Queue,
};

pub enum DocumentEvent {
    /// The text on disk now matches the document.
    Saved,
}

pub struct Document {
    worktree: PathBuf,
    key: FileKey,
    /// The committed text the diff compares with.
    base: Arc<str>,
    /// Where each line of `base` sits, for showing removed lines.
    base_lines: Vec<Range<usize>>,
    text: Rope,
    /// The file as last read or written, so a save can tell whether something else changed it.
    on_disk: Version,
    /// Counts edits, so a save knows which text it was for.
    revision: usize,
    saved_revision: usize,
    /// Counts changes to `on_disk`, so a read of the disk that started before one is dropped.
    disk_generation: usize,
    /// Set when the file changed on disk since it was read, which stops saving until the
    /// document is reloaded or the file is overwritten.
    conflict: Option<Version>,
    diff: LineDiff,
    /// The diff running. Replacing it cancels it, so only the newest one lands.
    diffing: Option<Task<()>>,
    undo: Vec<Vec<Edit>>,
    redo: Vec<Vec<Edit>>,
    /// Whether the next edit may join the last undo step.
    grouping: bool,
    /// Writes of the text, one at a time.
    saves: Queue,
    save_timer: Option<Task<()>>,
    git: Backend<dyn GitBackend>,
}

/// What a save wrote, or the file as it was on disk when it changed since it was read, so nothing
/// was written.
type Written = anyhow::Result<Result<String, Version>>;

/// One replacement, as applied: `old` at `start` became `new`.
#[derive(Clone, Debug)]
struct Edit {
    start: usize,
    old: String,
    new: String,
}

impl EventEmitter<DocumentEvent> for Document {}

/// A file's text and diff, prepared off the UI thread for a [`Document`] to open with.
pub(crate) struct Contents {
    base: String,
    base_lines: Vec<Range<usize>>,
    text: Rope,
    on_disk: Version,
    diff: LineDiff,
}

impl Contents {
    /// The file read as `versions`, or `None` when either version is binary, since there are no
    /// lines to show. A deleted file is empty. Diffs the whole file, so it's slow on a big one.
    pub(crate) fn new(versions: FileVersions) -> Option<Self> {
        let base = match versions.old {
            Version::Text(text) => text,
            Version::Missing => String::new(),
            Version::Binary => return None,
        };
        let (text, deleted) = match versions.new {
            Version::Text(text) => (text, false),
            Version::Missing => (String::new(), true),
            Version::Binary => return None,
        };
        Some(Self {
            base_lines: line_ranges(&base),
            diff: LineDiff::new(&base, &text),
            text: Rope::from_str(&text),
            on_disk: if deleted {
                Version::Missing
            } else {
                Version::Text(text)
            },
            base,
        })
    }
}

impl Document {
    pub(crate) fn new(
        worktree: PathBuf,
        key: FileKey,
        contents: Contents,
        git: Backend<dyn GitBackend>,
    ) -> Self {
        Self {
            worktree,
            key,
            base: contents.base.into(),
            base_lines: contents.base_lines,
            on_disk: contents.on_disk,
            text: contents.text,
            revision: 0,
            saved_revision: 0,
            disk_generation: 0,
            conflict: None,
            diff: contents.diff,
            diffing: None,
            undo: Vec::new(),
            redo: Vec::new(),
            grouping: false,
            saves: Queue::default(),
            save_timer: None,
            git,
        }
    }

    pub fn path(&self) -> &Path {
        &self.key.path
    }

    /// Whether it can change: an uncommitted file can, unless it's deleted, and a commit's can't.
    pub fn is_editable(&self) -> bool {
        self.key.source == Source::Uncommitted && self.on_disk != Version::Missing
    }

    pub fn text(&self) -> &Rope {
        &self.text
    }

    /// Which lines differ from the committed text. Recomputed off the UI thread after each edit,
    /// so it can lag the text by an edit or two.
    pub fn diff(&self) -> &LineDiff {
        &self.diff
    }

    /// The `index`th line of the committed text, without its line ending.
    pub fn base_line(&self, index: usize) -> &str {
        self.base_lines
            .get(index)
            .map_or("", |range| &self.base[range.clone()])
    }

    /// Whether it has edits that aren't on disk yet.
    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    /// Whether the file changed on disk while it had edits, so saving would overwrite someone
    /// else's change.
    pub fn has_conflict(&self) -> bool {
        self.conflict.is_some()
    }

    /// Replaces the bytes in `range` with `text`. Typing or deleting next to the last edit joins
    /// its undo step until [`Self::break_undo_group`].
    pub fn edit(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        if !self.is_editable() || (range.is_empty() && text.is_empty()) {
            return;
        }
        let edit = self.apply(range, text);
        self.redo.clear();
        match self.undo.last_mut() {
            Some(step)
                if self.grouping && joins(step.last().expect("steps aren't empty"), &edit) =>
            {
                step.push(edit)
            }
            _ => self.undo.push(vec![edit]),
        }
        self.grouping = true;
        self.changed(cx);
    }

    /// Starts a new undo step with the next edit, as moving the cursor does.
    pub fn break_undo_group(&mut self) {
        self.grouping = false;
    }

    /// Reverts the last undo step. Returns where the cursor goes: the end of what it restored.
    pub fn undo(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let step = self.undo.pop()?;
        let mut cursor: Option<usize> = None;
        for edit in step.iter().rev() {
            self.apply(edit.start..edit.start + edit.new.len(), &edit.old);
            let end = edit.start + edit.old.len();
            // Text restored at or before the cursor moves it along.
            let moved = cursor.map(|cursor| {
                if edit.start <= cursor {
                    (cursor + edit.old.len()).saturating_sub(edit.new.len())
                } else {
                    cursor
                }
            });
            cursor = Some(moved.map_or(end, |moved| moved.max(end)));
        }
        let cursor = cursor.unwrap_or(0);
        self.redo.push(step);
        self.grouping = false;
        self.changed(cx);
        Some(cursor)
    }

    /// Applies the last step undone again. Returns where the cursor goes.
    pub fn redo(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let step = self.redo.pop()?;
        let mut cursor = 0;
        for edit in &step {
            self.apply(edit.start..edit.start + edit.old.len(), &edit.new);
            cursor = edit.start + edit.new.len();
        }
        self.undo.push(step);
        self.grouping = false;
        self.changed(cx);
        Some(cursor)
    }

    fn apply(&mut self, range: Range<usize>, text: &str) -> Edit {
        let on_boundary = |byte| self.text.char_to_byte(self.text.byte_to_char(byte)) == byte;
        debug_assert!(
            on_boundary(range.start) && on_boundary(range.end),
            "edits start and end between characters"
        );
        let start = self.text.byte_to_char(range.start);
        let end = self.text.byte_to_char(range.end);
        let old = self.text.slice(start..end).to_string();
        self.text.remove(start..end);
        self.text.insert(start, text);
        Edit {
            start: range.start,
            old,
            new: text.to_owned(),
        }
    }

    /// Bumps the revision and diffs the new text.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.revision += 1;
        self.rediff(cx);
    }

    /// Diffs the text off the UI thread. A newer diff drops the one still running.
    fn rediff(&mut self, cx: &mut Context<Self>) {
        let (base, text) = (self.base.clone(), self.text.clone());
        self.diffing = Some(cx.spawn(async move |this, cx| {
            let diff = cx
                .background_executor()
                .spawn(async move { LineDiff::new(&base, &text.to_string()) })
                .await;
            this.update(cx, |this, cx| {
                this.diff = diff;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Saves once `delay` passes without another call, as auto-save does while typing.
    pub fn save_after(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.save_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |this, cx| this.save(cx)).ok();
        }));
    }

    /// Writes the text to the worktree, off the UI thread, unless the file changed on disk since
    /// it was read. One write runs at a time, and a save asked for meanwhile follows it.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        self.save_timer = None;
        if !self.is_dirty() || self.conflict.is_some() {
            return;
        }
        // A save checks again when it starts, so the ones queued behind a save that wrote
        // everything find nothing to do.
        self.saves.push(
            async move |this, cx| {
                let Ok(Some((revision, write))) = this.update(cx, |this, cx| this.write(cx)) else {
                    return;
                };
                let result = write.await;
                this.update(cx, |this, cx| this.written(revision, result, cx))
                    .ok();
            },
            cx,
        );
    }

    /// Starts writing the text, unless it's saved or the file changed on disk. Returns the
    /// revision it writes.
    fn write(&mut self, cx: &mut Context<Self>) -> Option<(usize, Task<Written>)> {
        if !self.is_dirty() || self.conflict.is_some() {
            return None;
        }
        let text = self.text.to_string();
        let expected = self.on_disk.clone();
        let (worktree, path) = (self.worktree.clone(), self.key.path.clone());
        let title = format!("Couldn't save {}", self.key.path.display());
        let write = self.git.call(
            title,
            move |git| {
                let current = git.read_file(&worktree, &path)?;
                if current != expected {
                    return Ok(Err(current));
                }
                git.write_file(&worktree, &path, &text)?;
                Ok(Ok(text))
            },
            cx,
        );
        Some((self.revision, write))
    }

    /// Follows a finished write of `revision`.
    fn written(&mut self, revision: usize, result: Written, cx: &mut Context<Self>) {
        match result {
            Ok(Ok(text)) => {
                self.set_on_disk(Version::Text(text));
                self.saved_revision = revision;
                cx.emit(DocumentEvent::Saved);
            }
            Ok(Err(current)) => {
                self.conflict = Some(current);
                Notices::post(self.conflict_notice(), cx);
            }
            // The backend reported it.
            Err(_) => {}
        }
        cx.notify();
    }

    fn conflict_notice(&self) -> Notice {
        Notice {
            severity: Severity::Error,
            origin: Origin::Git,
            title: format!(
                "{} changed on disk, so your edits weren't saved",
                self.key.path.display()
            )
            .into(),
            detail: None,
        }
    }

    /// Replaces the text with what's on disk, as one undo step, so the edits it drops can come
    /// back. Clears a conflict.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let (worktree, path) = (self.worktree.clone(), self.key.path.clone());
        let title = format!("Couldn't read {}", self.key.path.display());
        let read = self
            .git
            .call(title, move |git| git.read_file(&worktree, &path), cx);
        cx.spawn(async move |this, cx| {
            let Ok(on_disk) = read.await else {
                return;
            };
            this.update(cx, |this, cx| this.take_disk_text(on_disk, cx))
                .ok();
        })
        .detach();
    }

    /// Writes the text even though the file changed on disk.
    pub fn overwrite(&mut self, cx: &mut Context<Self>) {
        if let Some(current) = self.conflict.take() {
            self.set_on_disk(current);
            self.save(cx);
        }
    }

    /// Marks when the disk was last read or written, for [`Self::refresh`].
    pub(crate) fn disk_stamp(&self) -> usize {
        self.disk_generation
    }

    /// Follows a new committed text and the file on disk, as after the changes are read again.
    /// Edits that aren't saved are kept. `stamp` is [`Self::disk_stamp`] from before the read, so
    /// a read that a save or another read overtook doesn't roll the text back.
    pub(crate) fn refresh(&mut self, stamp: usize, versions: FileVersions, cx: &mut Context<Self>) {
        let base = match versions.old {
            Version::Text(text) => text,
            _ => String::new(),
        };
        let on_disk = versions.new;
        if *self.base != *base {
            self.base_lines = line_ranges(&base);
            self.base = base.into();
            self.rediff(cx);
        }
        let current = stamp == self.disk_generation && !self.saves.is_busy();
        if current && !self.is_dirty() && on_disk != self.on_disk {
            self.take_disk_text(on_disk, cx);
        }
    }

    fn set_on_disk(&mut self, on_disk: Version) {
        self.on_disk = on_disk;
        self.disk_generation += 1;
    }

    fn take_disk_text(&mut self, on_disk: Version, cx: &mut Context<Self>) {
        let Version::Text(text) = &on_disk else {
            return;
        };
        self.conflict = None;
        if self.text != text.as_str() {
            self.grouping = false;
            self.edit(0..self.text.len_bytes(), text, cx);
            self.grouping = false;
        }
        self.set_on_disk(on_disk);
        self.saved_revision = self.revision;
        cx.notify();
    }
}

/// Whether `next` continues typing or deleting where `last` left off.
fn joins(last: &Edit, next: &Edit) -> bool {
    let typing = last.old.is_empty()
        && next.old.is_empty()
        && next.start == last.start + last.new.len()
        && !next.new.contains('\n');
    let deleting = last.new.is_empty()
        && next.new.is_empty()
        && (next.start + next.old.len() == last.start || next.start == last.start);
    typing || deleting
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gpui::{AppContext as _, Entity, TestAppContext};

    use super::*;
    use crate::{
        Notices, Projects,
        fixtures::{FakeGit, PENSIVE_ROOT},
    };

    const PATH: &str = "crates/theme/src/colors.rs";

    /// The sample's colors file, open over a `FakeGit` the test can also read.
    async fn open(cx: &mut TestAppContext) -> (Entity<Document>, Arc<FakeGit>) {
        let git = Arc::new(FakeGit::default());
        let projects = cx.new(|_| Projects::new(git.clone()));
        let key = FileKey {
            source: Source::Uncommitted,
            path: PATH.into(),
        };
        let open = projects.update(cx, |projects, cx| {
            projects.open_document(PENSIVE_ROOT.into(), key.clone(), cx)
        });
        open.await.unwrap();
        let document = projects.read_with(cx, |projects, _| {
            projects
                .document(Path::new(PENSIVE_ROOT), &key)
                .unwrap()
                .clone()
        });
        (document, git)
    }

    fn on_disk(git: &FakeGit) -> String {
        match git
            .read_file(Path::new(PENSIVE_ROOT), Path::new(PATH))
            .unwrap()
        {
            Version::Text(text) => text,
            version => panic!("expected text, found {version:?}"),
        }
    }

    fn text(document: &Entity<Document>, cx: &mut TestAppContext) -> String {
        document.read_with(cx, |document, _| document.text().to_string())
    }

    #[gpui::test]
    async fn typing_undoes_as_one_step(cx: &mut TestAppContext) {
        let (document, _) = open(cx).await;
        let original = text(&document, cx);
        document.update(cx, |document, cx| {
            for (offset, letter) in ["a", "b", "c"].into_iter().enumerate() {
                document.edit(offset..offset, letter, cx);
            }
            document.break_undo_group();
            document.edit(0..0, "!", cx);
        });
        assert!(text(&document, cx).starts_with("!abc"));
        let cursor = document.update(cx, |document, cx| {
            document.undo(cx);
            document.undo(cx)
        });
        assert_eq!(cursor, Some(0));
        assert_eq!(text(&document, cx), original);
        document.update(cx, |document, cx| document.redo(cx));
        assert!(text(&document, cx).starts_with("abc"));
    }

    #[gpui::test]
    async fn undoing_a_forward_delete_puts_the_cursor_after_it(cx: &mut TestAppContext) {
        let (document, _) = open(cx).await;
        let cursor = document.update(cx, |document, cx| {
            document.edit(0..1, "", cx);
            document.edit(0..1, "", cx);
            document.undo(cx)
        });
        assert_eq!(cursor, Some(2));
    }

    #[gpui::test]
    async fn a_new_committed_text_leaves_the_document_clean(cx: &mut TestAppContext) {
        let (document, git) = open(cx).await;
        let disk = on_disk(&git);
        document.update(cx, |document, cx| {
            let stamp = document.disk_stamp();
            let versions = FileVersions {
                old: Version::Missing,
                new: Version::Text(disk),
            };
            document.refresh(stamp, versions, cx)
        });
        cx.run_until_parked();
        document.read_with(cx, |document, _| {
            assert!(!document.is_dirty());
            assert_eq!(document.diff().changes.len(), 1);
        });
    }

    #[gpui::test]
    async fn a_read_from_before_a_save_doesnt_roll_the_text_back(cx: &mut TestAppContext) {
        let (document, _) = open(cx).await;
        let stamp = document.read_with(cx, |document, _| document.disk_stamp());
        let before = text(&document, cx);
        document.update(cx, |document, cx| {
            document.edit(0..0, "saved\n", cx);
            document.save(cx);
        });
        cx.run_until_parked();
        document.update(cx, |document, cx| {
            let versions = FileVersions {
                old: Version::Missing,
                new: Version::Text(before),
            };
            document.refresh(stamp, versions, cx)
        });
        assert!(text(&document, cx).starts_with("saved\n"));
    }

    #[gpui::test]
    async fn the_diff_follows_edits(cx: &mut TestAppContext) {
        let (document, _) = open(cx).await;
        let before = document.read_with(cx, |document, _| document.diff().changes.len());
        document.update(cx, |document, cx| document.edit(0..0, "// new\n", cx));
        cx.run_until_parked();
        let changes = document.read_with(cx, |document, _| document.diff().changes.clone());
        assert_eq!(changes.len(), before + 1);
        assert_eq!(changes[0].new, 0..1);
    }

    #[gpui::test]
    async fn saving_writes_the_text_and_cleans_the_document(cx: &mut TestAppContext) {
        let (document, git) = open(cx).await;
        document.update(cx, |document, cx| {
            document.edit(0..0, "// first\n", cx);
            document.save(cx);
            // Asked for while the first save runs, so it follows.
            document.edit(0..0, "// second\n", cx);
            document.save(cx);
        });
        cx.run_until_parked();
        assert!(on_disk(&git).starts_with("// second\n// first\n"));
        assert!(!document.read_with(cx, |document, _| document.is_dirty()));
    }

    #[gpui::test]
    async fn a_file_changed_on_disk_isnt_overwritten(cx: &mut TestAppContext) {
        let (document, git) = open(cx).await;
        git.write_file(Path::new(PENSIVE_ROOT), Path::new(PATH), "theirs\n")
            .unwrap();
        document.update(cx, |document, cx| {
            document.edit(0..0, "mine\n", cx);
            document.save(cx);
        });
        cx.run_until_parked();
        assert_eq!(on_disk(&git), "theirs\n");
        assert!(document.read_with(cx, |document, _| document.has_conflict()));
        let notices = cx.update(Notices::global);
        notices.read_with(cx, |notices, _| {
            assert!(notices.held()[0].notice.title.contains("changed on disk"));
        });

        document.update(cx, |document, cx| document.reload(cx));
        cx.run_until_parked();
        assert_eq!(text(&document, cx), "theirs\n");
        assert!(!document.read_with(cx, |document, _| document.is_dirty()));
        // Reloading is one undo step, so the dropped edit can come back.
        document.update(cx, |document, cx| document.undo(cx));
        assert!(text(&document, cx).starts_with("mine\n"));
    }

    #[gpui::test]
    async fn auto_save_waits_for_typing_to_stop(cx: &mut TestAppContext) {
        let (document, git) = open(cx).await;
        let delay = Duration::from_secs(1);
        document.update(cx, |document, cx| {
            document.edit(0..0, "a", cx);
            document.save_after(delay, cx);
        });
        cx.executor().advance_clock(delay / 2);
        document.update(cx, |document, cx| {
            document.edit(1..1, "b", cx);
            document.save_after(delay, cx);
        });
        cx.executor().advance_clock(delay / 2);
        cx.run_until_parked();
        assert!(!on_disk(&git).starts_with("ab"));
        cx.executor().advance_clock(delay);
        cx.run_until_parked();
        assert!(on_disk(&git).starts_with("ab"));
    }
}

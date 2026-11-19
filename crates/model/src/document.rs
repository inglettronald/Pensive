//! A changed file open in the main view: its text, the committed text it's compared with, and,
//! for an uncommitted file, editing with undo and saving it back to the worktree.

use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use gpui::{Context, EventEmitter, Task};
use language::{Registry, Selection};
use text::{Rope, SourceId, SourceStamp, TextChange, TextEdit, TextSnapshot};

use crate::{
    Backend, FileKey, FileSource, FileVersions, GitBackend, LineDiff, Notice, Notices, Origin,
    Severity, Source, Version, diff::line_ranges, queue::Queue,
};

pub enum DocumentEvent {
    /// One ordered text transaction; consumers must match its `before` stamp.
    Changed(TextChange),
    /// The text on disk now matches the document.
    Saved,
}

pub struct Document {
    worktree: PathBuf,
    key: FileKey,
    base_source: FileSource,
    new_source: FileSource,
    base_present: bool,
    new_present: bool,
    languages: Arc<Registry>,
    /// The committed text the diff compares with.
    base: Arc<str>,
    /// Immutable old side, with an identity independent of the editable side.
    base_snapshot: TextSnapshot,
    /// Where each line of `base` sits, for showing removed lines.
    base_lines: Vec<Range<usize>>,
    text: Rope,
    text_source: SourceId,
    /// The file as last read or written, so a save can tell whether something else changed it.
    on_disk: Version,
    /// Counts edits, so a save knows which text it was for.
    revision: usize,
    saved_revision: usize,
    /// Counts updates to `on_disk`, so reads started before one are dropped.
    disk_generation: usize,
    refresh_generation: usize,
    /// Set when the file changed on disk since it was read, which stops saving until the
    /// document is reloaded or the file is overwritten.
    conflict: Option<Version>,
    diff: LineDiff,
    /// The diff running. Replacing it cancels it; stamps also guard publication.
    diffing: Option<Task<()>>,
    diff_generation: usize,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentSide {
    Base,
    New,
}

/// One immutable input for language work. Validate the text stamp, original source
/// context, presence and whole selection before publishing a future result.
#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    pub text: TextSnapshot,
    pub source: FileSource,
    pub present: bool,
    pub language: Selection,
}

impl DocumentSnapshot {
    pub fn same_input(&self, other: &Self) -> bool {
        self.text.stamp() == other.text.stamp()
            && self.source == other.source
            && self.present == other.present
            && self.language == other.language
    }
}

/// One replacement, as applied: `old` at `start` became `new`.
#[derive(Clone, Debug)]
struct Edit {
    start: usize,
    old: String,
    new: String,
}

/// Both inputs and the particular request must still match before a diff can land.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DiffStamp {
    base: SourceStamp,
    text: SourceStamp,
    generation: usize,
}

/// A disk generation plus the latest requested versions read. Unlike text stamps,
/// this also distinguishes overlapping reads of identical bytes and metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RefreshStamp {
    disk: usize,
    request: usize,
}

impl EventEmitter<DocumentEvent> for Document {}

/// A file's text and diff, prepared off the UI thread for a [`Document`] to open with.
pub(crate) struct Contents {
    base_source: FileSource,
    new_source: FileSource,
    base_present: bool,
    new_present: bool,
    base: String,
    base_rope: Rope,
    base_lines: Vec<Range<usize>>,
    text: Rope,
    on_disk: Version,
    diff: LineDiff,
}

impl Contents {
    /// The file read as `versions`, or `None` when either version is binary, since there are no
    /// lines to show. A deleted file is empty. Diffs the whole file, so it's slow on a big one.
    pub(crate) fn new(versions: FileVersions) -> Option<Self> {
        let base_present = versions.old != Version::Missing;
        let new_present = versions.new != Version::Missing;
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
            base_source: versions.old_source,
            new_source: versions.new_source,
            base_present,
            new_present,
            base_rope: Rope::from_str(&base),
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
            base_source: contents.base_source,
            new_source: contents.new_source,
            base_present: contents.base_present,
            new_present: contents.new_present,
            languages: Arc::new(Registry::default()),
            base: contents.base.into(),
            base_snapshot: TextSnapshot::new(
                SourceStamp {
                    source: SourceId::default(),
                    revision: 0,
                },
                &contents.base_rope,
            ),
            base_lines: contents.base_lines,
            on_disk: contents.on_disk,
            text: contents.text,
            text_source: SourceId::default(),
            revision: 0,
            saved_revision: 0,
            disk_generation: 0,
            refresh_generation: 0,
            conflict: None,
            diff: contents.diff,
            diffing: None,
            diff_generation: 0,
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

    /// A cheap immutable view of the existing editable/new rope, without a second buffer.
    pub fn snapshot(&self) -> TextSnapshot {
        TextSnapshot::new(self.text_stamp(), &self.text)
    }

    /// The immutable committed/old side. Base refreshes advance only this source's revision.
    pub fn base_snapshot(&self) -> TextSnapshot {
        self.base_snapshot.clone()
    }

    /// Resolves each original path independently, including missing/empty sides.
    /// No selection is reused solely because the text revision stayed unchanged.
    pub fn source_snapshot(&self, side: DocumentSide) -> DocumentSnapshot {
        let (text, source, present) = match side {
            DocumentSide::Base => (self.base_snapshot(), &self.base_source, self.base_present),
            DocumentSide::New => (self.snapshot(), &self.new_source, self.new_present),
        };
        DocumentSnapshot {
            text,
            language: self.languages.resolve(&source.path.to_string_lossy()),
            source: source.clone(),
            present,
        }
    }

    pub(crate) fn set_languages(&mut self, languages: Arc<Registry>, cx: &mut Context<Self>) {
        self.languages = languages;
        cx.notify();
    }

    fn text_stamp(&self) -> SourceStamp {
        SourceStamp {
            source: self.text_source.clone(),
            revision: self.revision,
        }
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
    ///
    /// Panics for reversed, out-of-bounds or non-UTF-8-boundary ranges. Validate untrusted
    /// ranges with `text::validate_range` before calling this editing boundary.
    pub fn edit(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        if !self.is_editable() {
            return;
        }
        self.replace(range, text, cx);
    }

    fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        text::validate_range(&self.text, &range).expect("edits use valid UTF-8 byte ranges");
        if range.is_empty() && text.is_empty() {
            return;
        }
        let (edit, change) = self.apply(range, text);
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
        self.changed(vec![change], cx);
    }

    /// Starts a new undo step with the next edit, as moving the cursor does.
    pub fn break_undo_group(&mut self) {
        self.grouping = false;
    }

    /// Reverts the last undo step. Returns where the cursor goes: the end of what it restored.
    pub fn undo(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let step = self.undo.pop()?;
        let mut cursor: Option<usize> = None;
        let mut changes = Vec::with_capacity(step.len());
        for edit in step.iter().rev() {
            let (_, change) = self.apply(edit.start..edit.start + edit.new.len(), &edit.old);
            changes.push(change);
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
        self.changed(changes, cx);
        Some(cursor)
    }

    /// Applies the last step undone again. Returns where the cursor goes.
    pub fn redo(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let step = self.redo.pop()?;
        let mut cursor = 0;
        let mut changes = Vec::with_capacity(step.len());
        for edit in &step {
            let (_, change) = self.apply(edit.start..edit.start + edit.old.len(), &edit.new);
            changes.push(change);
            cursor = edit.start + edit.new.len();
        }
        self.undo.push(step);
        self.grouping = false;
        self.changed(changes, cx);
        Some(cursor)
    }

    fn apply(&mut self, range: Range<usize>, text: &str) -> (Edit, TextEdit) {
        let change = TextEdit::replacement(&self.text, range.clone(), text)
            .expect("edits use valid UTF-8 byte ranges");
        let start = self.text.byte_to_char(range.start);
        let end = self.text.byte_to_char(range.end);
        let old = self.text.slice(start..end).to_string();
        self.text.remove(start..end);
        self.text.insert(start, text);
        (
            Edit {
                start: range.start,
                old,
                new: text.to_owned(),
            },
            change,
        )
    }

    /// Bumps the revision and diffs the new text.
    fn changed(&mut self, edits: Vec<TextEdit>, cx: &mut Context<Self>) {
        let before = self.text_stamp();
        let after = before.next();
        self.revision = after.revision;
        cx.emit(DocumentEvent::Changed(TextChange {
            before,
            after,
            edits,
        }));
        self.rediff(cx);
    }

    fn diff_stamp(&self) -> DiffStamp {
        DiffStamp {
            base: self.base_snapshot.stamp().clone(),
            text: self.text_stamp(),
            generation: self.diff_generation,
        }
    }

    fn accept_diff(&mut self, stamp: &DiffStamp, diff: LineDiff) -> bool {
        if *stamp != self.diff_stamp() {
            return false;
        }
        self.diff = diff;
        true
    }

    /// Diffs the text off the UI thread. A newer diff drops the one still running.
    fn rediff(&mut self, cx: &mut Context<Self>) {
        self.diff_generation = self
            .diff_generation
            .checked_add(1)
            .expect("diff generation exhausted");
        let stamp = self.diff_stamp();
        let (base, text) = (self.base.clone(), self.text.clone());
        self.diffing = Some(cx.spawn(async move |this, cx| {
            let diff = cx
                .background_executor()
                .spawn(async move { LineDiff::new(&base, &text.to_string()) })
                .await;
            this.update(cx, |this, cx| {
                if this.accept_diff(&stamp, diff) {
                    cx.notify();
                }
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
        let stamp = (self.text_stamp(), self.disk_stamp());
        let (worktree, path) = (self.worktree.clone(), self.key.path.clone());
        let title = format!("Couldn't read {}", self.key.path.display());
        let read = self
            .git
            .call(title, move |git| git.read_file(&worktree, &path), cx);
        cx.spawn(async move |this, cx| {
            let Ok(on_disk) = read.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if stamp == (this.text_stamp(), this.disk_stamp()) && !this.saves.is_busy() {
                    this.take_disk_text(on_disk, cx);
                }
            })
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

    fn disk_stamp(&self) -> usize {
        self.disk_generation
    }

    fn advance_refresh_generation(&mut self) {
        self.refresh_generation = self
            .refresh_generation
            .checked_add(1)
            .expect("refresh generation exhausted");
    }

    /// Marks the newest requested versions read. An older read must not restore
    /// its paths/commit origins even when all text revisions remain equal.
    pub(crate) fn start_refresh(&mut self) -> RefreshStamp {
        self.advance_refresh_generation();
        RefreshStamp {
            disk: self.disk_generation,
            request: self.refresh_generation,
        }
    }

    /// Follows a new committed text and the file on disk, as after the changes are read again.
    /// Edits that aren't saved are kept. `stamp` is [`Self::start_refresh`] from before the read, so
    /// a read that a save or another read overtook doesn't roll the text back.
    pub(crate) fn refresh(
        &mut self,
        stamp: RefreshStamp,
        versions: FileVersions,
        cx: &mut Context<Self>,
    ) {
        // A superseded read cannot restore old source metadata or base contents.
        if stamp.disk != self.disk_generation
            || stamp.request != self.refresh_generation
            || self.saves.is_busy()
        {
            return;
        }
        self.base_source = versions.old_source;
        self.new_source = versions.new_source;
        self.base_present = versions.old != Version::Missing;
        self.new_present = versions.new != Version::Missing;
        let base = match versions.old {
            Version::Text(text) => text,
            _ => String::new(),
        };
        let on_disk = versions.new;
        if *self.base != *base {
            self.base_lines = line_ranges(&base);
            self.base_snapshot =
                TextSnapshot::new(self.base_snapshot.stamp().next(), &Rope::from_str(&base));
            self.base = base.into();
            self.rediff(cx);
        }
        if !self.is_dirty() && on_disk != self.on_disk {
            self.take_disk_text(on_disk, cx);
        }
        // Retire the request even when it only updated base or source metadata.
        self.advance_refresh_generation();
        cx.notify();
    }

    fn set_on_disk(&mut self, on_disk: Version) {
        self.new_present = on_disk != Version::Missing;
        self.on_disk = on_disk;
        self.disk_generation = self
            .disk_generation
            .checked_add(1)
            .expect("disk generation exhausted");
    }

    fn take_disk_text(&mut self, on_disk: Version, cx: &mut Context<Self>) {
        let text = match &on_disk {
            Version::Text(text) => text.as_str(),
            Version::Missing => "",
            Version::Binary => return,
        };
        self.conflict = None;
        if self.text != text {
            // One precise replacement, leaving shared prefix/suffix bytes untouched. Compare
            // scalars so a shared leading UTF-8 byte cannot split a multibyte character.
            let start: usize = self
                .text
                .chars()
                .zip(text.chars())
                .take_while(|(old, new)| old == new)
                .map(|(char, _)| char.len_utf8())
                .sum();
            let tail = self.text.byte_slice(start..);
            let suffix: usize = tail
                .chars_at(tail.len_chars())
                .reversed()
                .zip(text[start..].chars().rev())
                .take_while(|(old, new)| old == new)
                .map(|(char, _)| char.len_utf8())
                .sum();
            self.grouping = false;
            self.replace(
                start..self.text.len_bytes() - suffix,
                &text[start..text.len() - suffix],
                cx,
            );
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
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use gpui::{AppContext as _, Entity, TestAppContext};

    use super::*;
    use crate::{
        Notices, Projects,
        fixtures::{FakeGit, PENSIVE_ROOT},
    };

    const PATH: &str = "crates/theme/src/colors.rs";

    fn versions(old: Version, new: Version) -> FileVersions {
        FileVersions {
            old,
            new,
            old_source: FileSource {
                path: PATH.into(),
                origin: crate::FileOrigin::Commit(Some("fixture-head".into())),
            },
            new_source: FileSource {
                path: PATH.into(),
                origin: crate::FileOrigin::Worktree,
            },
        }
    }

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

    fn sample(text: &str, cx: &mut TestAppContext) -> (Entity<Document>, Arc<FakeGit>) {
        let git = Arc::new(FakeGit::default());
        git.write_file(Path::new(PENSIVE_ROOT), Path::new(PATH), text)
            .unwrap();
        let contents = Contents::new(versions(
            Version::Text(text.into()),
            Version::Text(text.into()),
        ))
        .unwrap();
        let document = cx.new(|_| {
            Document::new(
                PENSIVE_ROOT.into(),
                FileKey {
                    source: Source::Uncommitted,
                    path: PATH.into(),
                },
                contents,
                Backend::new(git.clone(), Origin::Git),
            )
        });
        (document, git)
    }

    fn changes(
        document: &Entity<Document>,
        cx: &mut TestAppContext,
    ) -> Rc<RefCell<Vec<TextChange>>> {
        let changes = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            let changes = changes.clone();
            cx.subscribe(document, move |_, event: &DocumentEvent, _| {
                if let DocumentEvent::Changed(change) = event {
                    changes.borrow_mut().push(change.clone());
                }
            })
            .detach();
        });
        changes
    }

    #[gpui::test]
    fn cross_extension_rename_keeps_each_side_context_and_identity(cx: &mut TestAppContext) {
        let git = Arc::new(FakeGit::default());
        let mut versions = versions(
            Version::Text("const x = 1;\n".into()),
            Version::Text("const x = 1;\n".into()),
        );
        versions.old_source.path = "src/old.js".into();
        versions.new_source.path = "src/new.ts".into();
        let contents = Contents::new(versions).unwrap();
        let document = cx.new(|_| {
            Document::new(
                PENSIVE_ROOT.into(),
                FileKey {
                    source: Source::Uncommitted,
                    path: "src/new.ts".into(),
                },
                contents,
                Backend::new(git, Origin::Git),
            )
        });
        document.read_with(cx, |document, _| {
            let old = document.source_snapshot(DocumentSide::Base);
            let new = document.source_snapshot(DocumentSide::New);
            assert_eq!(old.source.path, Path::new("src/old.js"));
            assert_eq!(new.source.path, Path::new("src/new.ts"));
            assert_eq!(old.language.language.as_str(), "javascript");
            assert_eq!(new.language.language.as_str(), "typescript");
            assert_eq!(
                old.source.origin,
                crate::FileOrigin::Commit(Some("fixture-head".into()))
            );
            assert_eq!(new.source.origin, crate::FileOrigin::Worktree);
            assert_ne!(old.text.stamp().source, new.text.stamp().source);
            assert_eq!(old.text.stamp().revision, new.text.stamp().revision);
            assert!(!old.same_input(&new));
            assert!(new.same_input(&document.source_snapshot(DocumentSide::New)));
        });
    }

    #[gpui::test]
    fn missing_sides_retain_paths_and_committed_documents_stay_read_only(cx: &mut TestAppContext) {
        for (old, new, old_present, new_present) in [
            (Version::Missing, Version::Text(String::new()), false, true),
            (Version::Text("old".into()), Version::Missing, true, false),
        ] {
            for committed in [false, true] {
                let mut versions = versions(old.clone(), new.clone());
                versions.old_source.path = "file.js".into();
                versions.new_source.path = "file.ts".into();
                if committed {
                    versions.new_source.origin =
                        crate::FileOrigin::Commit(Some("new-commit".into()));
                }
                let document = cx.new(|_| {
                    Document::new(
                        PENSIVE_ROOT.into(),
                        FileKey {
                            source: if committed {
                                Source::Commit("new-commit".into())
                            } else {
                                Source::Uncommitted
                            },
                            path: "file.ts".into(),
                        },
                        Contents::new(versions).unwrap(),
                        Backend::new(Arc::new(FakeGit::default()), Origin::Git),
                    )
                });
                document.read_with(cx, |document, _| {
                    let old = document.source_snapshot(DocumentSide::Base);
                    let new = document.source_snapshot(DocumentSide::New);
                    assert_eq!(old.present, old_present);
                    assert_eq!(new.present, new_present);
                    assert_eq!(old.language.language.as_str(), "javascript");
                    assert_eq!(new.language.language.as_str(), "typescript");
                    assert_eq!(document.is_editable(), !committed && new_present);
                });
            }
        }
    }

    #[gpui::test]
    fn metadata_and_registration_changes_invalidate_equal_revision_inputs(cx: &mut TestAppContext) {
        let (document, _) = sample("unchanged", cx);
        document.update(cx, |document, cx| {
            let old = document.source_snapshot(DocumentSide::Base);
            let new = document.source_snapshot(DocumentSide::New);
            let stale_read = document.start_refresh();
            let mut incoming = versions(
                Version::Text("unchanged".into()),
                Version::Text("unchanged".into()),
            );
            incoming.old_source.path = "original.js".into();
            incoming.old_source.origin = crate::FileOrigin::Commit(Some("next-head".into()));
            document.refresh(stale_read, incoming, cx);
            let refreshed = document.source_snapshot(DocumentSide::Base);
            assert_eq!(refreshed.text.stamp(), old.text.stamp());
            assert_eq!(refreshed.language.language.as_str(), "javascript");
            assert!(!old.same_input(&refreshed));
            assert!(new.same_input(&document.source_snapshot(DocumentSide::New)));

            document.refresh(
                stale_read,
                versions(Version::Text("stale".into()), Version::Text("stale".into())),
                cx,
            );
            assert!(refreshed.same_input(&document.source_snapshot(DocumentSide::Base)));
            let mut registry = Registry::default();
            registry
                .associate(language::Association::Extension("rs".into()), "text")
                .unwrap();
            document.set_languages(Arc::new(registry), cx);
            let reassociated = document.source_snapshot(DocumentSide::New);
            assert_eq!(reassociated.text.stamp(), new.text.stamp());
            assert_eq!(reassociated.language.language.as_str(), "text");
            assert!(!new.same_input(&reassociated));
            let mut registry = Registry::default();
            let mut registration = language::LanguageRegistration::new("rust");
            registration.extensions = vec!["rs".into()];
            registration.providers.push(language::ProviderRegistration {
                id: "test-syntax".into(),
                capabilities: vec![language::Capability::Syntax],
                priority: 0,
            });
            registry.register(registration).unwrap();
            document.set_languages(Arc::new(registry), cx);
            let provider_changed = document.source_snapshot(DocumentSide::New);
            assert_eq!(provider_changed.text.stamp(), new.text.stamp());
            assert_eq!(
                provider_changed.language.syntax.as_deref(),
                Some("test-syntax")
            );
            assert!(!provider_changed.same_input(&new));
        });
    }

    #[gpui::test]
    fn newer_refresh_requests_win_and_presence_changes_keep_source_identity(
        cx: &mut TestAppContext,
    ) {
        let (document, _) = sample("original", cx);
        document.update(cx, |document, cx| {
            let before = document.source_snapshot(DocumentSide::New);
            let older = document.start_refresh();
            let newer = document.start_refresh();
            let mut stale = versions(Version::Text("stale".into()), Version::Text("stale".into()));
            stale.old_source.path = "stale.js".into();
            document.refresh(older, stale, cx);
            assert!(
                document
                    .source_snapshot(DocumentSide::New)
                    .same_input(&before)
            );
            document.refresh(
                newer,
                versions(Version::Text("original".into()), Version::Missing),
                cx,
            );
            let deleted = document.source_snapshot(DocumentSide::New);
            assert!(!deleted.present);
            assert_eq!(deleted.text.rope().len_bytes(), 0);
            assert_eq!(deleted.text.stamp().source, before.text.stamp().source);
            assert_ne!(deleted.text.stamp(), before.text.stamp());
            assert!(!document.is_editable());
            assert!(!document.is_dirty());
            assert_eq!(before.text.rope().to_string(), "original");
            let stamp = document.start_refresh();
            document.refresh(
                stamp,
                versions(
                    Version::Text("original".into()),
                    Version::Text(String::new()),
                ),
                cx,
            );
            let added = document.source_snapshot(DocumentSide::New);
            assert!(added.present);
            assert_eq!(added.text.stamp(), deleted.text.stamp());
            assert!(!added.same_input(&deleted));
            assert!(document.is_editable());
            assert!(added.same_input(&document.source_snapshot(DocumentSide::New)));
        });
    }

    #[gpui::test]
    fn metadata_refresh_keeps_dirty_text_and_source_identity(cx: &mut TestAppContext) {
        let (document, _) = sample("original", cx);
        document.update(cx, |document, cx| {
            document.edit(0..0, "edited ", cx);
            let new = document.source_snapshot(DocumentSide::New);
            let mut incoming = versions(
                Version::Text("new base".into()),
                Version::Text("disk changes".into()),
            );
            incoming.old_source.path = "original.js".into();
            let stamp = document.start_refresh();
            document.refresh(stamp, incoming, cx);
            let after = document.source_snapshot(DocumentSide::New);
            assert_eq!(after.text.rope().to_string(), "edited original");
            assert!(after.same_input(&new));
            assert!(document.is_dirty());
            assert_eq!(
                document
                    .source_snapshot(DocumentSide::Base)
                    .language
                    .language
                    .as_str(),
                "javascript"
            );
        });
    }

    #[gpui::test]
    fn snapshots_distinguish_documents_and_sides_and_survive_edits(cx: &mut TestAppContext) {
        let (document, _) = sample("é\r\n😀", cx);
        let (other, _) = sample("é\r\n😀", cx);
        let before = document.read_with(cx, |document, _| document.snapshot());
        let base = document.read_with(cx, |document, _| document.base_snapshot());
        let independent = other.read_with(cx, |document, _| document.snapshot());
        assert_ne!(before.stamp(), independent.stamp());
        assert_ne!(before.stamp(), base.stamp());
        assert_eq!(before.stamp().revision, 0);
        assert_eq!(independent.stamp().revision, 0);
        document.update(cx, |document, cx| document.edit(0..2, "中", cx));
        let after = document.read_with(cx, |document, _| document.snapshot());
        assert_eq!(after.stamp(), &before.stamp().next());
        assert_eq!(before.rope().to_string(), "é\r\n😀");
        assert_eq!(base.rope().to_string(), "é\r\n😀");
        assert_eq!(after.rope().to_string(), "中\r\n😀");
    }

    #[gpui::test]
    fn grouped_undo_and_redo_publish_ordered_byte_edits_with_one_revision(cx: &mut TestAppContext) {
        let (document, _) = sample("start\r\nend", cx);
        let changes = changes(&document, cx);
        document.update(cx, |document, cx| {
            document.edit(7..7, "é", cx);
            document.edit(9..9, "😀", cx);
            assert_eq!(document.undo(cx), Some(7));
            assert_eq!(document.text.to_string(), "start\r\nend");
            // Keep the existing revision-based dirty semantics even when undo restores bytes.
            assert!(document.is_dirty());
            assert_eq!(document.redo(cx), Some(13));
        });
        let changes = changes.borrow();
        assert_eq!(changes.len(), 4);
        for (index, change) in changes.iter().enumerate() {
            assert_eq!(change.before.revision, index);
            assert_eq!(change.after, change.before.next());
            assert_eq!(change.first_changed_line(), 1);
            if index > 0 {
                assert_eq!(changes[index - 1].after, change.before);
            }
        }
        let ranges = |change: &TextChange| {
            change
                .edits
                .iter()
                .map(|edit| (edit.old.clone(), edit.new.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(ranges(&changes[0]), [(7..7, 7..9)]);
        assert_eq!(ranges(&changes[1]), [(9..9, 9..13)]);
        assert_eq!(ranges(&changes[2]), [(9..13, 9..9), (7..9, 7..7)]);
        assert_eq!(ranges(&changes[3]), [(7..7, 7..9), (9..9, 9..13)]);
        assert_eq!(
            changes[2].edits[0].old_points.end,
            text::BytePoint { line: 1, column: 6 }
        );
        assert_eq!(
            changes[2].edits[1].new_points.end,
            text::BytePoint { line: 1, column: 0 }
        );
        assert_eq!(text(&document, cx), "start\r\né😀end");
    }

    #[gpui::test]
    fn replacement_deletion_and_inverse_records_track_line_changes(cx: &mut TestAppContext) {
        let (document, _) = sample("é\r\n😀x\nlast", cx);
        let changes = changes(&document, cx);
        document.update(cx, |document, cx| {
            document.edit(4..9, "中\r\n🙂", cx);
            document.undo(cx);
            document.redo(cx);
            document.break_undo_group();
            document.edit(2..4, "", cx);
        });
        let changes = changes.borrow();
        assert_eq!(changes.len(), 4);
        let replacement = &changes[0].edits[0];
        let undo = &changes[1].edits[0];
        assert_eq!(replacement.old, 4..9);
        assert_eq!(replacement.new, 4..13);
        assert_eq!(undo.old, replacement.new);
        assert_eq!(undo.new, replacement.old);
        assert_eq!(undo.old_points, replacement.new_points);
        assert_eq!(undo.new_points, replacement.old_points);
        assert_eq!(changes[2].edits[0], *replacement);
        assert_eq!(changes[3].edits[0].old, 2..4);
        assert_eq!(changes[3].edits[0].new, 2..2);
        assert_eq!(changes[3].first_changed_line(), 0);
        assert_eq!(text(&document, cx), "é中\r\n🙂\nlast");
    }

    #[gpui::test]
    async fn reload_is_a_precise_transaction_and_can_be_undone(cx: &mut TestAppContext) {
        let (document, git) = sample("same\r\né tail\r\n", cx);
        let changes = changes(&document, cx);
        document.update(cx, |document, cx| document.edit(8..13, " mine", cx));
        git.write_file(
            Path::new(PENSIVE_ROOT),
            Path::new(PATH),
            "same\r\nê changed\r\n",
        )
        .unwrap();
        document.update(cx, |document, cx| document.reload(cx));
        cx.run_until_parked();
        assert_eq!(text(&document, cx), "same\r\nê changed\r\n");
        assert!(!document.read_with(cx, |document, _| document.is_dirty()));
        {
            let changes = changes.borrow();
            assert_eq!(changes.len(), 2);
            let reload = &changes[1];
            assert_eq!(reload.before.revision, 1);
            assert_eq!(reload.after.revision, 2);
            assert_eq!(reload.first_changed_line(), 1);
            assert_eq!(reload.edits[0].old, 6..13);
            assert_eq!(reload.edits[0].new, 6..16);
            assert_eq!(
                reload.edits[0].new_points.end,
                text::BytePoint {
                    line: 1,
                    column: 10
                }
            );
        }
        document.update(cx, |document, cx| document.undo(cx));
        assert_eq!(text(&document, cx), "same\r\né mine\r\n");
        document.update(cx, |document, cx| document.redo(cx));
        assert_eq!(text(&document, cx), "same\r\nê changed\r\n");
        document.update(cx, |document, cx| document.reload(cx));
        cx.run_until_parked();
        assert_eq!(
            changes.borrow().len(),
            4,
            "unchanged reload publishes no edit"
        );
        assert!(!document.read_with(cx, |document, _| document.is_dirty()));
    }

    #[gpui::test]
    fn base_only_refresh_invalidates_old_source_and_diff_without_editing(cx: &mut TestAppContext) {
        let (document, _) = sample("same\r\n", cx);
        let changes = changes(&document, cx);
        document.update(cx, |document, cx| {
            let base = document.base_snapshot();
            let text = document.snapshot();
            let diff = document.diff_stamp();
            let versions = versions(
                Version::Text("new base\r\n".into()),
                document.on_disk.clone(),
            );
            let stamp = document.start_refresh();
            document.refresh(stamp, versions.clone(), cx);
            assert_eq!(document.base_snapshot.stamp(), &base.stamp().next());
            assert_eq!(document.snapshot().stamp(), text.stamp());
            assert_eq!(base.rope().to_string(), "same\r\n");
            assert_eq!(document.base_snapshot.rope().to_string(), "new base\r\n");
            assert!(!document.accept_diff(&diff, LineDiff::default()));
            assert!(!document.is_dirty());
            let refreshed = document.diff_stamp();
            let stamp = document.start_refresh();
            document.refresh(stamp, versions, cx);
            assert_eq!(
                document.diff_stamp(),
                refreshed,
                "unchanged base retains its stamp"
            );
        });
        cx.run_until_parked();
        assert!(changes.borrow().is_empty());
        document.read_with(cx, |document, _| {
            assert_eq!(*document.diff(), LineDiff::new("new base\r\n", "same\r\n"));
        });
    }

    #[gpui::test]
    fn diff_publication_rejects_other_sources_revisions_and_generations(cx: &mut TestAppContext) {
        let (document, _) = sample("text\n", cx);
        let (other, _) = sample("text\n", cx);
        let unrelated = other.read_with(cx, |document, _| document.diff_stamp());
        document.update(cx, |document, cx| {
            assert!(!document.accept_diff(&unrelated, LineDiff::default()));
            let before_edit = document.diff_stamp();
            document.edit(0..0, "!", cx);
            let current = document.diff_stamp();
            let mut stale = current.clone();
            stale.text = before_edit.text.clone();
            assert!(!document.accept_diff(&stale, LineDiff::default()));
            let mut stale = current.clone();
            stale.base.source = SourceId::default();
            assert!(!document.accept_diff(&stale, LineDiff::default()));
            document.rediff(cx);
            assert!(
                !document.accept_diff(&current, LineDiff::default()),
                "same inputs, superseded request"
            );
            let expected = LineDiff::new("text\n", "!text\n");
            assert!(document.accept_diff(&document.diff_stamp(), expected.clone()));
            assert_eq!(*document.diff(), expected);
            assert!(!document.accept_diff(&before_edit, LineDiff::default()));
            assert_eq!(
                *document.diff(),
                expected,
                "rejection preserves the published result"
            );
        });
    }

    #[gpui::test]
    fn a_refresh_that_keeps_dirty_text_does_not_cancel_reload(cx: &mut TestAppContext) {
        let (document, git) = sample("original", cx);
        git.write_file(Path::new(PENSIVE_ROOT), Path::new(PATH), "changed on disk")
            .unwrap();

        document.update(cx, |document, cx| {
            document.edit(0..0, "unsaved ", cx);
            document.reload(cx);
            let stamp = document.start_refresh();
            let refreshed = versions(
                Version::Text("original".into()),
                Version::Text("changed on disk".into()),
            );
            document.refresh(stamp, refreshed, cx);
            assert_eq!(document.text().to_string(), "unsaved original");
        });

        cx.run_until_parked();
        assert_eq!(text(&document, cx), "changed on disk");
        assert!(!document.read_with(cx, |document, _| document.is_dirty()));
    }

    #[gpui::test]
    fn edits_overtake_a_pending_reload(cx: &mut TestAppContext) {
        let (document, git) = sample("original\n", cx);
        git.write_file(Path::new(PENSIVE_ROOT), Path::new(PATH), "disk\n")
            .unwrap();
        document.update(cx, |document, cx| {
            document.reload(cx);
            document.edit(0..0, "new edit\n", cx);
        });
        cx.run_until_parked();
        assert_eq!(text(&document, cx), "new edit\noriginal\n");
        assert!(document.read_with(cx, |document, _| document.is_dirty()));
    }

    #[gpui::test]
    fn saving_keeps_original_unicode_and_crlf_bytes_and_snapshot_stamp(cx: &mut TestAppContext) {
        let (document, git) = sample("é\r\n😀\r\n", cx);
        document.update(cx, |document, cx| {
            document.edit(4..8, "中", cx);
            document.save(cx);
        });
        let snapshot = document.read_with(cx, |document, _| document.snapshot());
        cx.run_until_parked();
        assert_eq!(on_disk(&git).as_bytes(), "é\r\n中\r\n".as_bytes());
        document.read_with(cx, |document, _| {
            assert_eq!(document.snapshot().stamp(), snapshot.stamp());
            assert!(!document.is_dirty());
        });
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
            let stamp = document.start_refresh();
            let versions = versions(Version::Missing, Version::Text(disk));
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
        let stamp = document.update(cx, |document, _| document.start_refresh());
        let before = text(&document, cx);
        document.update(cx, |document, cx| {
            document.edit(0..0, "saved\n", cx);
            document.save(cx);
        });
        cx.run_until_parked();
        document.update(cx, |document, cx| {
            let versions = versions(Version::Missing, Version::Text(before));
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

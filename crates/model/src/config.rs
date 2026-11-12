//! The trait that config folder backends implement, and the settings and theme files read through
//! it.

use std::{fmt::Debug, sync::Arc, time::Duration};

use anyhow::Result;
use futures::{StreamExt, channel::mpsc};
use gpui::{Context, EventEmitter};

/// Calls block, so models run them off the UI thread.
pub trait ConfigBackend: Send + Sync + Debug {
    /// The text of `settings.toml`, empty when there's no file.
    fn read_settings(&self) -> Result<String>;
    /// Replaces `settings.toml` with `text`.
    fn write_settings(&self, text: &str) -> Result<()>;
    /// Each theme file, as its name without `.toml` and its text.
    fn read_themes(&self) -> Result<Vec<(String, String)>>;
    /// Calls `changed` whenever a file might have changed, for as long as the returned value is
    /// kept.
    fn watch(&self, changed: Box<dyn Fn() + Send + Sync>) -> Result<Box<dyn Send>>;
}

pub enum ConfigEvent {
    /// `settings` holds new text from disk.
    SettingsRead,
    /// `themes` holds new files from disk.
    ThemesRead,
    /// A file couldn't be read, written or watched.
    Problem(String),
}

/// The settings and theme files as last read from the config folder or written to it.
pub struct ConfigFiles {
    pub settings: String,
    pub themes: Vec<(String, String)>,
    backend: Arc<dyn ConfigBackend>,
    /// Settings text waiting to be written. One write runs at a time.
    unsaved: Option<String>,
    saving: bool,
    watching: Option<Box<dyn Send>>,
}

impl EventEmitter<ConfigEvent> for ConfigFiles {}

impl ConfigFiles {
    /// No files yet, reading the config folder through `backend` once `load` is called.
    pub fn new(backend: Arc<dyn ConfigBackend>) -> Self {
        Self {
            settings: String::new(),
            themes: Vec::new(),
            backend,
            unsaved: None,
            saving: false,
            watching: None,
        }
    }

    /// Reads the files, and reads them again whenever they change. Subscribe before calling it,
    /// so no event is missed.
    pub fn load(&mut self, cx: &mut Context<Self>) {
        let (changed, mut changes) = mpsc::unbounded();
        match self.backend.watch(Box::new(move || {
            changed.unbounded_send(()).ok();
        })) {
            Ok(watching) => self.watching = Some(watching),
            Err(error) => cx.emit(ConfigEvent::Problem(format!(
                "Settings won't reload while Pensive runs: {error:#}"
            ))),
        }
        cx.spawn(async move |this, cx| {
            while changes.next().await.is_some() {
                // One save can be several events. Wait for the rest, then read once.
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                while changes.try_recv().is_ok() {}
                if this.update(cx, |this, cx| this.reload(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        self.reload(cx);
    }

    /// Reads the files again, off the UI thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let read = cx
            .background_executor()
            .spawn(async move { (backend.read_settings(), backend.read_themes()) });
        cx.spawn(async move |this, cx| {
            let (settings, themes) = read.await;
            this.update(cx, |this, cx| this.read(settings, themes, cx))
                .ok();
        })
        .detach();
    }

    fn read(
        &mut self,
        settings: Result<String>,
        themes: Result<Vec<(String, String)>>,
        cx: &mut Context<Self>,
    ) {
        match settings {
            Ok(settings) if settings != self.settings => {
                self.settings = settings;
                cx.emit(ConfigEvent::SettingsRead);
            }
            Ok(_) => {}
            Err(error) => cx.emit(ConfigEvent::Problem(format!(
                "Couldn't read settings.toml: {error:#}"
            ))),
        }
        match themes {
            Ok(themes) if themes != self.themes => {
                self.themes = themes;
                cx.emit(ConfigEvent::ThemesRead);
            }
            Ok(_) => {}
            Err(error) => cx.emit(ConfigEvent::Problem(format!(
                "Couldn't read the themes: {error:#}"
            ))),
        }
    }

    /// Writes `text` as `settings.toml`, off the UI thread. If the file changed on disk since it
    /// was last read, the user's edit wins: it's read instead and `text` is dropped.
    pub fn save_settings(&mut self, text: String, cx: &mut Context<Self>) {
        if text == self.settings {
            self.unsaved = None;
            return;
        }
        self.unsaved = Some(text);
        if !self.saving {
            self.saving = true;
            self.write_unsaved(cx);
        }
    }

    fn write_unsaved(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            while let Ok(Some((text, last_read, backend))) = this.update(cx, |this, _| {
                let text = this.unsaved.take();
                if text.is_none() {
                    this.saving = false;
                }
                Some((text?, this.settings.clone(), this.backend.clone()))
            }) {
                let written = text.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move { write_if_unchanged(&*backend, &last_read, &written) })
                    .await;
                let updated = this.update(cx, |this, cx| match result {
                    Ok(None) => this.settings = text,
                    Ok(Some(on_disk)) => {
                        this.unsaved = None;
                        this.settings = on_disk;
                        cx.emit(ConfigEvent::Problem(
                            "settings.toml changed on disk, so the change made in Pensive \
                             wasn't saved"
                                .into(),
                        ));
                        cx.emit(ConfigEvent::SettingsRead);
                    }
                    Err(error) => cx.emit(ConfigEvent::Problem(format!(
                        "Couldn't save settings.toml: {error:#}"
                    ))),
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}

/// Writes `text` unless the file no longer holds `last_read`. Returns the file's text when it
/// changed.
fn write_if_unchanged(
    backend: &dyn ConfigBackend,
    last_read: &str,
    text: &str,
) -> Result<Option<String>> {
    let on_disk = backend.read_settings()?;
    if on_disk != last_read {
        return Ok(Some(on_disk));
    }
    backend.write_settings(text)?;
    Ok(None)
}

#[cfg(all(test, feature = "fixtures"))]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{AppContext as _, Entity, TestAppContext};

    use super::*;
    use crate::fixtures::FakeConfig;

    /// The files loaded through `fake`, and a record of the events they sent.
    fn load(
        fake: &Arc<FakeConfig>,
        cx: &mut TestAppContext,
    ) -> (Entity<ConfigFiles>, Rc<RefCell<Vec<String>>>) {
        let files = cx.new(|_| ConfigFiles::new(fake.clone()));
        let events = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            let events = events.clone();
            cx.subscribe(&files, move |_, event: &ConfigEvent, _| {
                events.borrow_mut().push(match event {
                    ConfigEvent::SettingsRead => "settings read".into(),
                    ConfigEvent::ThemesRead => "themes read".into(),
                    ConfigEvent::Problem(problem) => problem.clone(),
                })
            })
            .detach();
        });
        files.update(cx, |files, cx| files.load(cx));
        cx.run_until_parked();
        (files, events)
    }

    #[gpui::test]
    fn load_reads_settings_and_themes(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeConfig::new("density = \"dense\"", &[("nord", "")]));
        let (files, events) = load(&fake, cx);
        files.read_with(cx, |files, _| {
            assert_eq!(files.settings, "density = \"dense\"");
            assert_eq!(files.themes.len(), 1);
        });
        assert_eq!(*events.borrow(), ["settings read", "themes read"]);
    }

    #[gpui::test]
    fn saving_writes_the_settings(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeConfig::new("", &[]));
        let (files, events) = load(&fake, cx);
        files.update(cx, |files, cx| {
            files.save_settings("density = \"dense\"".into(), cx)
        });
        cx.run_until_parked();
        assert_eq!(fake.settings(), "density = \"dense\"");
        assert!(events.borrow().is_empty());
    }

    #[gpui::test]
    fn a_hand_edit_since_the_last_read_wins_over_a_save(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeConfig::new("", &[]));
        let (files, events) = load(&fake, cx);
        fake.edit_settings("theme = \"nord\"");
        files.update(cx, |files, cx| {
            files.save_settings("density = \"dense\"".into(), cx)
        });
        cx.run_until_parked();
        assert_eq!(fake.settings(), "theme = \"nord\"");
        files.read_with(cx, |files, _| {
            assert_eq!(files.settings, "theme = \"nord\"")
        });
        assert_eq!(events.borrow().last().unwrap(), "settings read");
    }
}

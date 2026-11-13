//! How a model reaches an I/O backend: each call runs off the UI thread, and a failure is reported
//! to the notices before the model sees it.

use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use gpui::{App, SharedString, Task};

use crate::{Notice, Notices, Origin};

/// A model holds this instead of the backend itself, so no call can skip the report.
#[derive(Debug)]
pub struct Backend<B: ?Sized> {
    backend: Arc<B>,
    /// What a failure is blamed on, unless it's a [`crate::FixtureGap`].
    origin: Origin,
}

impl<B: ?Sized> Clone for Backend<B> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            origin: self.origin,
        }
    }
}

impl<B: ?Sized + Send + Sync + 'static> Backend<B> {
    pub fn new(backend: Arc<B>, origin: Origin) -> Self {
        Self { backend, origin }
    }

    /// Runs `work` on the background executor. A failure is reported under `title`, such as
    /// "Couldn't open ~/code/harbor", and still returned, for a view that also shows it on the
    /// control that started the call.
    pub fn call<T: Send + 'static>(
        &self,
        title: impl Into<SharedString>,
        work: impl FnOnce(&B) -> Result<T> + Send + 'static,
        cx: &mut App,
    ) -> Task<Result<T>> {
        let backend = self.backend.clone();
        let origin = self.origin;
        let title = title.into();
        let running = cx
            .background_executor()
            .spawn(async move { work(&backend) });
        cx.spawn(async move |cx| {
            let result = running.await;
            if let Err(error) = &result {
                let notice = Notice::failed(title, origin, error);
                cx.update(|cx| Notices::post(notice, cx));
            }
            result
        })
    }
}

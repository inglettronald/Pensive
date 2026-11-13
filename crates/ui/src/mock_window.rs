//! The window view mocks start from: Pensive on the sample projects, set up as the app sets it up.

use gpui::{App, AppContext, Entity, Window};
use model::{ProjectId, fixtures};

use crate::root::Pensive;

/// Pensive on the sample projects with `project` open, after declaring every view's settings as
/// the app does, so the palette and Settings list the same settings in a mock as in the app.
pub(crate) fn pensive(
    project: Option<ProjectId>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Pensive> {
    crate::declare_settings(cx);
    let projects = cx.new(|_| fixtures::projects());
    cx.new(|cx| Pensive::new(projects, project, window, cx))
}

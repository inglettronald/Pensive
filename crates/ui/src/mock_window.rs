//! The window view mocks start from: Pensive on the sample projects, set up as the app sets it up,
//! at the size of a maximized window on a 1080p screen.

use gpui::{AnyView, App, AppContext, Entity, Window};
use mock::Mock;
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

/// A mock of a whole window, at 1920 × 1080, that the matrix also renders at other sizes.
pub(crate) fn mock(
    subject: &'static str,
    name: &'static str,
    build: fn(&mut Window, &mut App) -> AnyView,
) -> Mock {
    fixed(subject, name, build).in_matrix()
}

/// A mock of a whole window that the matrix leaves out, because it sets its own density or zoom
/// or isn't a screen people use.
pub(crate) fn fixed(
    subject: &'static str,
    name: &'static str,
    build: fn(&mut Window, &mut App) -> AnyView,
) -> Mock {
    Mock::new(subject, name, build).size(1920., 1080.)
}

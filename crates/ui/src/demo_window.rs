//! Opens a demo's window at the size asked for, on a mock when one is named.

use gpui::{App, AppContext, Entity, Pixels, Size};
use mock::Mock;
use model::Projects;

use crate::root::{Pensive, WINDOW_SIZE, window_options};

/// A window `size` big on the landing screen, or on the mock whose id is `mock`, as if opened
/// from the mock browser. Fails, naming the id, when no mock has it.
pub fn open_demo_window(
    projects: Entity<Projects>,
    mock: Option<&str>,
    size: Option<Size<Pixels>>,
    cx: &mut App,
) -> Result<(), String> {
    let mock = mock.map(find).transpose()?;
    cx.open_window(
        window_options(size.unwrap_or(WINDOW_SIZE), cx),
        |window, cx| {
            cx.new(|cx| {
                let mut pensive = Pensive::new(projects, None, window, cx);
                if let Some((mock, component)) = mock {
                    pensive.view_mocks(window, cx);
                    pensive.open_mock(mock, component, window, cx);
                }
                pensive
            })
        },
    )
    .expect("failed to open a window");
    Ok(())
}

/// The mock with `id`, and whether it's a component's.
fn find(id: &str) -> Result<(Mock, bool), String> {
    let views = crate::mocks().into_iter().map(|mock| (mock, false));
    let components = components::mocks().into_iter().map(|mock| (mock, true));
    views
        .chain(components)
        .find(|(mock, _)| mock.id() == id)
        .ok_or_else(|| format!("no mock is called `{id}`. Ctrl Shift M in the demo lists them."))
}

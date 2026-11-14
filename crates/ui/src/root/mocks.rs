//! The root view's mocks: the landing screen, a project, and the command palette over them.

use std::sync::Arc;

use gpui::{AnyView, App, AppContext as _, Context, Window};
use mock::Mock;
use model::{
    ProjectId, Projects,
    fixtures::{FakeGit, HARBOR, PENSIVE},
};

use super::Pensive;
use crate::{
    mock_window,
    palette::{Keyword, PaletteView, Tab},
};

/// A window on the sample projects, with the palette open on `tab` and set up by `setup`.
fn window(
    project: Option<ProjectId>,
    tab: Option<Tab>,
    setup: fn(&mut PaletteView, &mut Context<PaletteView>),
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    let view = mock_window::pensive(project, window, cx);
    view.update(cx, |pensive, cx| {
        if let Some(tab) = tab {
            pensive.open_palette(Some(tab), window, cx);
        }
        pensive.palette.update(cx, setup);
    });
    view.into()
}

fn untouched(_: &mut PaletteView, _: &mut Context<PaletteView>) {}

pub(crate) fn mocks() -> Vec<Mock> {
    vec![
        Mock::new("root", "empty", |window, cx| {
            crate::declare_settings(cx);
            let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
            cx.new(|cx| Pensive::new(projects, None, window, cx)).into()
        }),
        Mock::new("root", "landing", |w, cx| {
            window(None, None, untouched, w, cx)
        }),
        Mock::new("root", "landing_typing", |w, cx| {
            window(
                None,
                None,
                |palette, cx| palette.set_query("gui", cx),
                w,
                cx,
            )
        }),
        Mock::new("root", "project", |w, cx| {
            window(Some(PENSIVE), None, untouched, w, cx)
        }),
        Mock::new("root", "palette", |w, cx| {
            window(Some(PENSIVE), Some(Tab::All), untouched, w, cx)
        }),
        Mock::new("root", "active", |w, cx| {
            window(Some(PENSIVE), Some(Tab::Active), untouched, w, cx)
        }),
        Mock::new("root", "commands", |w, cx| {
            window(Some(PENSIVE), Some(Tab::Commands), untouched, w, cx)
        }),
        Mock::new("root", "scoped", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| {
                    palette.set_keyword(Some(Keyword::Project(Some(HARBOR))), cx);
                    palette.set_query("fix", cx);
                },
                w,
                cx,
            )
        }),
        Mock::new("root", "no_results", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| {
                    palette.set_keyword(Some(Keyword::Project(Some(HARBOR))), cx);
                    palette.set_query("qwerty", cx);
                },
                w,
                cx,
            )
        }),
        Mock::new("root", "clone", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| palette.set_query("https://github.com/zed-industries/zed.git", cx),
                w,
                cx,
            )
        }),
        Mock::new("root", "settings", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| palette.set_keyword(Some(Keyword::Setting), cx),
                w,
                cx,
            )
        }),
        Mock::new("root", "find_settings", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| palette.set_query("sett", cx),
                w,
                cx,
            )
        }),
    ]
}

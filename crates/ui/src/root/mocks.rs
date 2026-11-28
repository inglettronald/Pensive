//! The root view's mocks: the landing screen, a project, and the command palette over them.

use std::sync::Arc;

use gpui::{AnyView, App, AppContext as _, Context, Window};
use mock::Mock;
use model::{
    ProjectId, Projects,
    fixtures::{FakeGit, HARBOR, PENSIVE},
};

use theme::Theme;

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
        mock_window::mock("root", "empty", |window, cx| {
            crate::declare_settings(cx);
            let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
            cx.new(|cx| Pensive::new(projects, None, window, cx)).into()
        }),
        mock_window::mock("root", "landing", |w, cx| {
            window(None, None, untouched, w, cx)
        }),
        mock_window::mock("root", "landing_typing", |w, cx| {
            window(
                None,
                None,
                |palette, cx| palette.set_query("gui", cx),
                w,
                cx,
            )
        }),
        mock_window::mock("root", "project", |w, cx| {
            window(Some(PENSIVE), None, untouched, w, cx)
        }),
        mock_window::fixed("root", "project_light", |w, cx| {
            cx.set_global(Theme::pensive_light());
            window(Some(PENSIVE), None, untouched, w, cx)
        }),
        mock_window::fixed("root", "project_paper", |w, cx| {
            cx.set_global(Theme::pensive_paper());
            window(Some(PENSIVE), None, untouched, w, cx)
        }),
        mock_window::mock("root", "run_not_wired", |w, cx| {
            let view = mock_window::pensive(Some(PENSIVE), w, cx);
            view.update(cx, |pensive, cx| {
                let clear = cx.spawn(async |_, _| {});
                pensive.run_notice = Some((model::Notice::not_wired("Run"), clear));
            });
            view.into()
        }),
        mock_window::mock("root", "palette", |w, cx| {
            window(Some(PENSIVE), Some(Tab::All), untouched, w, cx)
        }),
        mock_window::fixed("root", "palette_light", |w, cx| {
            cx.set_global(Theme::pensive_light());
            window(Some(PENSIVE), Some(Tab::All), untouched, w, cx)
        }),
        mock_window::fixed("root", "palette_paper", |w, cx| {
            cx.set_global(Theme::pensive_paper());
            window(Some(PENSIVE), Some(Tab::All), untouched, w, cx)
        }),
        mock_window::mock("root", "active", |w, cx| {
            window(Some(PENSIVE), Some(Tab::Active), untouched, w, cx)
        }),
        mock_window::mock("root", "commands", |w, cx| {
            window(Some(PENSIVE), Some(Tab::Commands), untouched, w, cx)
        }),
        mock_window::mock("root", "scoped", |w, cx| {
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
        mock_window::mock("root", "no_results", |w, cx| {
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
        mock_window::mock("root", "clone", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| palette.set_query("https://github.com/zed-industries/zed.git", cx),
                w,
                cx,
            )
        }),
        mock_window::mock("root", "settings", |w, cx| {
            window(
                Some(PENSIVE),
                Some(Tab::All),
                |palette, cx| palette.set_keyword(Some(Keyword::Setting), cx),
                w,
                cx,
            )
        }),
        mock_window::mock("root", "find_settings", |w, cx| {
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

//! One mock per board on the app shell canvas: the pensive project's window in each state,
//! built from the sample changes, then harbor's window partway through a merge.

use gpui::{AnyView, App, AppContext, Context, Window};
use mock::Mock;
use model::fixtures::{self, HARBOR, PENSIVE};
use theme::DensityPreset;

use super::{Control, FileLayout, Panel, ProjectView};
use crate::{
    editor::{Mode, SAVE, SaveMode},
    mock_window,
    root::Pensive,
};

pub(crate) fn mocks() -> Vec<Mock> {
    vec![
        mock_window::mock("project", "some_staged", |window, cx| {
            framed(untouched, window, cx)
        }),
        mock_window::mock("project", "all_staged", |window, cx| {
            framed(|view, _, cx| view.set_all_staged(true, cx), window, cx)
        }),
        mock_window::mock("project", "nothing_staged", |window, cx| {
            framed(|view, _, cx| view.set_all_staged(false, cx), window, cx)
        }),
        mock_window::mock("project", "uncommitted_folded", |window, cx| {
            framed(
                |view, _, cx| view.set_uncommitted_open(false, cx),
                window,
                cx,
            )
        }),
        mock_window::mock("project", "commit_open", |window, cx| {
            framed(
                |view, _, cx| view.toggle_commit("5e35ea9".into(), cx),
                window,
                cx,
            )
        }),
        mock_window::mock("project", "hovering_a_file", |window, cx| {
            framed(|view, _, cx| view.show_path(1, cx), window, cx)
        }),
        mock_window::mock("project", "path_copied", |window, cx| {
            framed(|view, _, cx| view.show_copied(1, cx), window, cx)
        }),
        mock_window::mock("project", "push_not_wired", |window, cx| {
            framed(
                |view, _, cx| view.show_notice(Control::Push, cx),
                window,
                cx,
            )
        }),
        mock_window::mock("project", "pull_requests_not_wired", |window, cx| {
            framed(
                |view, _, cx| view.show_notice(Control::Panel(Panel::PullRequests), cx),
                window,
                cx,
            )
        }),
        mock_window::mock("project", "commit_not_wired", |window, cx| {
            framed(
                |view, _, cx| view.show_notice(Control::Commit, cx),
                window,
                cx,
            )
        }),
        mock_window::mock("project", "tree_layout", |window, cx| {
            framed(
                |view, _, cx| view.set_layout(FileLayout::Tree, cx),
                window,
                cx,
            )
        }),
        mock_window::mock("project", "symbol_layout", |window, cx| {
            framed(
                |view, _, cx| {
                    view.set_layout(FileLayout::Symbols, cx);
                    view.select_symbol(1, cx);
                },
                window,
                cx,
            )
        }),
        mock_window::mock("project", "no_changes", |window, cx| {
            crate::declare_settings(cx);
            let projects = cx.new(|_| fixtures::clean_projects());
            cx.new(|cx| Pensive::new(projects, Some(PENSIVE), window, cx))
                .into()
        }),
        mock_window::mock("project", "changes_folded", |window, cx| {
            framed(|view, _, cx| view.select_uncommitted(5, cx), window, cx)
        }),
        mock_window::mock("project", "file_mode", |window, cx| {
            framed(
                |view, _, cx| {
                    view.select_uncommitted(5, cx);
                    view.set_editor_mode(Mode::File, cx);
                },
                window,
                cx,
            )
        }),
        mock_window::mock("project", "plain_mode", |window, cx| {
            framed(
                |view, _, cx| view.set_editor_mode(Mode::Plain, cx),
                window,
                cx,
            )
        }),
        mock_window::mock("project", "unsaved", |window, cx| {
            SAVE.set(SaveMode::Manual, cx);
            framed(
                |view, window, cx| view.type_when_open("// TODO: ", window, cx),
                window,
                cx,
            )
        }),
        mock_window::mock("project", "renamed_and_conflicted", |window, cx| {
            mock_window::pensive(Some(HARBOR), window, cx).into()
        }),
        mock_window::fixed("project", "some_staged_dense", |window, cx| {
            cx.set_global(DensityPreset::Dense.density());
            framed(untouched, window, cx)
        }),
        mock_window::mock("files_panel", "calm", |window, cx| {
            framed(|view, _, cx| view.show_panel(Panel::Files, cx), window, cx)
        }),
        mock_window::mock("files_panel", "filtered", |window, cx| {
            framed(
                |view, _, cx| {
                    view.show_panel(Panel::Files, cx);
                    view.set_file_filter("palette", cx);
                },
                window,
                cx,
            )
        }),
        mock_window::fixed("files_panel", "dense", |window, cx| {
            cx.set_global(DensityPreset::Dense.density());
            framed(|view, _, cx| view.show_panel(Panel::Files, cx), window, cx)
        }),
    ]
}

fn untouched(_: &mut ProjectView, _: &mut Window, _: &mut Context<ProjectView>) {}

/// The pensive project's window on the sample changes, with its view set up by `setup`.
fn framed(
    setup: fn(&mut ProjectView, &mut Window, &mut Context<ProjectView>),
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    let pensive = mock_window::pensive(Some(PENSIVE), window, cx);
    let view = pensive
        .read(cx)
        .project_view()
        .expect("the window opens on a project");
    view.update(cx, |view, cx| setup(view, window, cx));
    pensive.into()
}

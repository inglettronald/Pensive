//! One mock per board on the app shell canvas: the pensive project's window in each state,
//! built from the sample changes.

use std::sync::Arc;

use components::{TitleBar, TitleBarProject};
use gpui::{AnyView, App, Context, Entity, Window, div, prelude::*};
use mock::Mock;
use model::{
    Projects,
    fixtures::{self, FakeGit, PENSIVE},
};
use theme::{ActiveTheme, DensityPreset};

use super::{Control, FileLayout, ProjectView};
use crate::editor::{Mode, SAVE, SaveMode};

pub(crate) fn mocks() -> Vec<Mock> {
    vec![
        Mock::new("project", "some_staged", |window, cx| {
            framed(untouched, window, cx)
        }),
        Mock::new("project", "all_staged", |window, cx| {
            framed(|view, _, cx| view.set_all_staged(true, cx), window, cx)
        }),
        Mock::new("project", "nothing_staged", |window, cx| {
            framed(|view, _, cx| view.set_all_staged(false, cx), window, cx)
        }),
        Mock::new("project", "uncommitted_folded", |window, cx| {
            framed(
                |view, _, cx| view.set_uncommitted_open(false, cx),
                window,
                cx,
            )
        }),
        Mock::new("project", "commit_open", |window, cx| {
            framed(
                |view, _, cx| view.toggle_commit("5e35ea9".into(), cx),
                window,
                cx,
            )
        }),
        Mock::new("project", "hovering_a_file", |window, cx| {
            framed(|view, _, cx| view.show_path(1, cx), window, cx)
        }),
        Mock::new("project", "path_copied", |window, cx| {
            framed(|view, _, cx| view.show_copied(1, cx), window, cx)
        }),
        Mock::new("project", "push_not_wired", |window, cx| {
            framed(
                |view, _, cx| view.show_notice(Control::Push, cx),
                window,
                cx,
            )
        }),
        Mock::new("project", "commit_not_wired", |window, cx| {
            framed(
                |view, _, cx| view.show_notice(Control::Commit, cx),
                window,
                cx,
            )
        }),
        Mock::new("project", "tree_layout", |window, cx| {
            framed(
                |view, _, cx| view.set_layout(FileLayout::Tree, cx),
                window,
                cx,
            )
        }),
        Mock::new("project", "symbol_layout", |window, cx| {
            framed(
                |view, _, cx| {
                    view.set_layout(FileLayout::Symbols, cx);
                    view.select_symbol(1, cx);
                },
                window,
                cx,
            )
        }),
        Mock::new("project", "no_changes", |window, cx| {
            let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
            let view = cx.new(|cx| ProjectView::new(projects, PENSIVE, window, cx));
            cx.new(|_| Framed(view)).into()
        }),
        Mock::new("project", "changes_folded", |window, cx| {
            framed(|view, _, cx| view.select_uncommitted(5, cx), window, cx)
        }),
        Mock::new("project", "file_mode", |window, cx| {
            framed(
                |view, _, cx| {
                    view.select_uncommitted(5, cx);
                    view.set_editor_mode(Mode::File, cx);
                },
                window,
                cx,
            )
        }),
        Mock::new("project", "plain_mode", |window, cx| {
            framed(
                |view, _, cx| view.set_editor_mode(Mode::Plain, cx),
                window,
                cx,
            )
        }),
        Mock::new("project", "unsaved", |window, cx| {
            SAVE.set(SaveMode::Manual, cx);
            framed(
                |view, window, cx| view.type_when_open("// TODO: ", window, cx),
                window,
                cx,
            )
        }),
        Mock::new("project", "some_staged_dense", |window, cx| {
            cx.set_global(DensityPreset::Dense.density());
            framed(untouched, window, cx)
        }),
    ]
}

fn untouched(_: &mut ProjectView, _: &mut Window, _: &mut Context<ProjectView>) {}

/// The view on the sample changes, set up by `setup`, under the title bar it has in a window.
fn framed(
    setup: fn(&mut ProjectView, &mut Window, &mut Context<ProjectView>),
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    crate::declare_settings(cx);
    let projects = cx.new(|_| fixtures::projects());
    let view = cx.new(|cx| ProjectView::new(projects, PENSIVE, window, cx));
    view.update(cx, |view, cx| setup(view, window, cx));
    cx.new(|_| Framed(view)).into()
}

struct Framed(Entity<ProjectView>);

impl Render for Framed {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.colors.surface_app)
            .text_color(theme.colors.text)
            .font_family(theme.fonts.ui.clone())
            .child(
                TitleBar::new(Some(TitleBarProject {
                    initial: "P".into(),
                    name: "pensive".into(),
                    branch: Some("feat/command-palette".into()),
                }))
                .on_palette(|_, _| {}),
            )
            .child(self.0.clone())
    }
}

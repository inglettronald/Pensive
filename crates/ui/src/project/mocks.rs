//! One mock per board on the app shell canvas: the pensive project's window in each state,
//! built from the sample changes.

use components::{TitleBar, TitleBarProject};
use gpui::{AnyView, App, Context, Entity, Window, div, prelude::*};
use mock::Mock;
use model::{
    ProjectId, Projects,
    fixtures::{self, HARBOR, PENSIVE},
};
use theme::{ActiveTheme, DensityPreset};

use super::{Control, FileLayout, ProjectView};

pub(crate) fn mocks() -> Vec<Mock> {
    vec![
        Mock::new("project", "some_staged", |window, cx| {
            framed(untouched, window, cx)
        }),
        Mock::new("project", "all_staged", |window, cx| {
            framed(|view, cx| view.set_all_staged(true, cx), window, cx)
        }),
        Mock::new("project", "nothing_staged", |window, cx| {
            framed(|view, cx| view.set_all_staged(false, cx), window, cx)
        }),
        Mock::new("project", "uncommitted_folded", |window, cx| {
            framed(|view, cx| view.set_uncommitted_open(false, cx), window, cx)
        }),
        Mock::new("project", "commit_open", |window, cx| {
            framed(
                |view, cx| view.toggle_commit("5e35ea9".into(), cx),
                window,
                cx,
            )
        }),
        Mock::new("project", "hovering_a_file", |window, cx| {
            framed(|view, cx| view.show_path(1, cx), window, cx)
        }),
        Mock::new("project", "path_copied", |window, cx| {
            framed(|view, cx| view.show_copied(1, cx), window, cx)
        }),
        Mock::new("project", "push_not_wired", |window, cx| {
            framed(|view, cx| view.show_notice(Control::Push, cx), window, cx)
        }),
        Mock::new("project", "commit_not_wired", |window, cx| {
            framed(|view, cx| view.show_notice(Control::Commit, cx), window, cx)
        }),
        Mock::new("project", "tree_layout", |window, cx| {
            framed(|view, cx| view.set_layout(FileLayout::Tree, cx), window, cx)
        }),
        Mock::new("project", "symbol_layout", |window, cx| {
            framed(
                |view, cx| {
                    view.set_layout(FileLayout::Symbols, cx);
                    view.select_symbol(1, cx);
                },
                window,
                cx,
            )
        }),
        Mock::new("project", "no_changes", |_, cx| {
            // Harbor's sample worktree is clean, so FakeGit reads it as having no changes.
            framed_project(HARBOR, untouched, cx)
        }),
        Mock::new("project", "some_staged_dense", |window, cx| {
            cx.set_global(DensityPreset::Dense.density());
            framed(untouched, window, cx)
        }),
    ]
}

fn untouched(_: &mut ProjectView, _: &mut Context<ProjectView>) {}

/// The view on the sample changes, set up by `setup`, under the title bar it has in a window.
fn framed(
    setup: fn(&mut ProjectView, &mut Context<ProjectView>),
    _window: &mut Window,
    cx: &mut App,
) -> AnyView {
    framed_project(PENSIVE, setup, cx)
}

/// [`framed`] for any sample project.
fn framed_project(
    project: ProjectId,
    setup: fn(&mut ProjectView, &mut Context<ProjectView>),
    cx: &mut App,
) -> AnyView {
    let projects = cx.new(|_| fixtures::projects());
    let view = cx.new(|cx| ProjectView::new(projects.clone(), project, cx));
    view.update(cx, setup);
    cx.new(|_| Framed {
        view,
        projects,
        project,
    })
    .into()
}

struct Framed {
    view: Entity<ProjectView>,
    projects: Entity<Projects>,
    project: ProjectId,
}

impl Render for Framed {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let projects = self.projects.read(cx);
        let title = projects
            .project(self.project)
            .map(|project| TitleBarProject {
                initial: project.initial(),
                name: project.name.clone(),
                branch: projects.current_branch(self.project).cloned(),
            });
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.colors.surface_app)
            .text_color(theme.colors.text)
            .font_family(theme.fonts.ui.clone())
            .child(TitleBar::new(title).on_palette(|_, _| {}))
            .child(self.view.clone())
    }
}

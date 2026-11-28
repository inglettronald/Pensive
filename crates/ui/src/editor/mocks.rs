use std::{path::Path, sync::Arc};

use gpui::{
    AnyView, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window,
    div,
};
use mock::Mock;
use model::{
    FileKey, Projects, Source,
    fixtures::{FakeGit, PENSIVE_ROOT, SYNTAX_SAMPLE},
};
use theme::ActiveTheme;

use super::{Editor, Mode};

pub(crate) fn mocks() -> Vec<Mock> {
    vec![
        Mock::new("editor", "syntax_changes", |window, cx| {
            editor(Mode::Changes, Source::Uncommitted, window, cx)
        })
        .size(760., 390.),
        Mock::new("editor", "syntax_commit", |window, cx| {
            editor(Mode::File, Source::Commit("5e35ea9".into()), window, cx)
        })
        .size(760., 290.),
        Mock::new("editor", "syntax_plain", |window, cx| {
            editor(Mode::Plain, Source::Uncommitted, window, cx)
        })
        .size(760., 290.),
    ]
}

fn editor(mode: Mode, source: Source, window: &mut Window, cx: &mut App) -> AnyView {
    crate::declare_settings(cx);
    let projects = cx.new(|_| Projects::new(Arc::new(FakeGit::default())));
    let key = FileKey {
        source,
        path: SYNTAX_SAMPLE.into(),
    };
    let open = projects.update(cx, |projects, cx| {
        projects.open_document(PENSIVE_ROOT.into(), key.clone(), cx)
    });
    let editor = cx.new(|cx| {
        cx.spawn(async move |editor, cx| {
            open.await.expect("the sample opens");
            editor
                .update(cx, |editor: &mut Editor, cx| {
                    let document = projects
                        .read(cx)
                        .document(Path::new(PENSIVE_ROOT), &key)
                        .cloned();
                    editor.set_mode(mode, cx);
                    editor.set_document(document, cx);
                })
                .ok();
        })
        .detach();
        Editor::new(window, cx)
    });
    cx.new(|_| Frame(editor)).into()
}

struct Frame(Entity<Editor>);

impl Render for Frame {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().colors.surface_app)
            .child(self.0.clone())
    }
}

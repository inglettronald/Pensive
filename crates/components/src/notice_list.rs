//! The notices that are held, errors first, each dismissable, with Clear all at the top.

use gpui::{App, ElementId, SharedString, Window, div, prelude::*};
use theme::{ActiveTheme, Dp, Shadow, Typeset, dp};

use crate::{Callback, IconButton, IconName, Tag, Tone, callback};

/// Wider than a toast, since a row has its age and dismiss button too.
const WIDTH: Dp = dp(380.);

#[derive(IntoElement)]
pub struct NoticeList {
    rows: Vec<NoticeRow>,
    on_clear: Option<Callback<()>>,
}

impl NoticeList {
    /// The rows in the order given.
    pub fn new(rows: impl IntoIterator<Item = NoticeRow>) -> Self {
        Self {
            rows: rows.into_iter().collect(),
            on_clear: None,
        }
    }

    pub fn on_clear(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(callback(handler));
        self
    }
}

pub struct NoticeRow {
    id: ElementId,
    tone: Tone,
    title: SharedString,
    tag: Option<Tag>,
    count: usize,
    detail: Option<SharedString>,
    age: SharedString,
    on_dismiss: Option<Callback<()>>,
}

impl NoticeRow {
    pub fn new(id: impl Into<ElementId>, tone: Tone, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            tone,
            title: title.into(),
            tag: None,
            count: 1,
            detail: None,
            age: SharedString::default(),
            on_dismiss: None,
        }
    }

    pub fn tag(mut self, tag: Tag) -> Self {
        self.tag = Some(tag);
        self
    }

    /// How many times it happened. Above one, it shows as `×3`.
    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// How long ago it last happened, such as `4m`.
    pub fn age(mut self, age: impl Into<SharedString>) -> Self {
        self.age = age.into();
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(callback(handler));
        self
    }
}

impl RenderOnce for NoticeList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let density = cx.density();
        let rows = self.rows.into_iter().map(|row| {
            let group = SharedString::from(format!("notice-{:?}", row.id));
            div()
                .id(row.id)
                .group(group.clone())
                .flex()
                .gap_2p5()
                .py_2p5()
                .pl_3()
                .pr_2p5()
                .border_t_1()
                .border_color(colors.border_subtle)
                .hover(|row| row.bg(colors.surface_selected))
                .children(
                    row.tone
                        .icon(16., colors)
                        .map(|icon| div().mt_0p5().child(icon)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .typeset(density.body)
                                .text_color(colors.text_strong)
                                .child(row.title)
                                .children(row.tag)
                                .when(row.count > 1, |line| {
                                    line.child(
                                        div()
                                            .typeset(density.label)
                                            .text_color(colors.text_faint)
                                            .child(format!("×{}", row.count)),
                                    )
                                }),
                        )
                        .children(row.detail.map(|detail| {
                            div()
                                .font_family(theme.fonts.code.clone())
                                .typeset(density.small)
                                .text_color(colors.text_muted)
                                .child(detail)
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap_1()
                        .typeset(density.small)
                        .text_color(colors.text_faint)
                        .child(
                            div()
                                .invisible()
                                .group_hover(group, |dismiss| dismiss.visible())
                                .child(
                                    IconButton::new("dismiss", IconName::X)
                                        .size(dp(20.))
                                        .color(|colors| colors.text_faint)
                                        .when_some(row.on_dismiss, |button, handler| {
                                            button
                                                .on_click(move |window, cx| handler((), window, cx))
                                        }),
                                ),
                        )
                        .child(row.age),
                )
        });
        div()
            .flex()
            .flex_col()
            .w(WIDTH)
            .overflow_hidden()
            .rounded(density.radius_lg)
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface_raised)
            .shadow(Shadow::Medium.layers())
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(density.row_height + dp(8.))
                    .pl_3()
                    .pr_2()
                    .typeset(density.body)
                    .text_color(colors.text_strong)
                    .child(div().flex_1().child("Notices"))
                    .child(
                        div()
                            .id("clear")
                            .px_1p5()
                            .py_0p5()
                            .rounded(density.radius_sm)
                            .typeset(density.small)
                            .text_color(colors.text_muted)
                            .hover(|clear| clear.bg(colors.surface_selected))
                            .child("Clear all")
                            .when_some(self.on_clear, |clear, handler| {
                                clear.on_click(move |_, window, cx| handler((), window, cx))
                            }),
                    ),
            )
            .children(rows)
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    vec![
        mock::Mock::component("notice_list", "gallery", || {
            crate::Gallery::new("Notice list")
                .section(
                    "Errors, then warnings",
                    [(
                        "Three held",
                        NoticeList::new([
                            NoticeRow::new("read", Tone::Error, "Couldn't read changes")
                                .tag(Tag::new("Fixture").accent())
                                .detail(
                                    "~/code/harbor-wt/tooltip-clip is not in a sample repository",
                                )
                                .age("1m"),
                            NoticeRow::new("config", Tone::Error, "Couldn't read the settings")
                                .tag(Tag::new("Config"))
                                .detail("settings.toml:14: expected a boolean")
                                .age("4m"),
                            NoticeRow::new("unknown", Tone::Warning, "Setting ignored")
                                .tag(Tag::new("Config"))
                                .count(3)
                                .detail("`palette.blur` isn't a setting")
                                .age("4m"),
                        ])
                        .into_any_element(),
                    )],
                )
                .into_any_element()
        })
        .size(640., 420.),
    ]
}

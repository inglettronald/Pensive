//! A labeled page of a component's states, which its gallery mock renders.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Styled, Window,
    div, px,
};
use theme::{ActiveTheme, Typeset};

#[derive(IntoElement)]
pub struct Gallery {
    title: SharedString,
    sections: Vec<Section>,
}

enum Section {
    Items {
        title: SharedString,
        items: Vec<(SharedString, AnyElement)>,
    },
    Matrix {
        title: SharedString,
        columns: Vec<&'static str>,
        rows: Vec<(&'static str, Vec<AnyElement>)>,
    },
}

impl Gallery {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            sections: Vec::new(),
        }
    }

    /// A row of items, each labeled underneath. Wraps when it runs out of width.
    pub fn section(
        mut self,
        title: impl Into<SharedString>,
        items: impl IntoIterator<Item = (impl Into<SharedString>, AnyElement)>,
    ) -> Self {
        self.sections.push(Section::Items {
            title: title.into(),
            items: items
                .into_iter()
                .map(|(label, item)| (label.into(), item))
                .collect(),
        });
        self
    }

    /// One cell for every combination of `rows` and `columns`, labeled with their names. Pass
    /// an enum's full list of values, so a new value shows up without changing the gallery.
    pub fn matrix<R, C>(
        mut self,
        title: impl Into<SharedString>,
        rows: &[R],
        columns: &[C],
        cell: impl Fn(R, C) -> AnyElement,
    ) -> Self
    where
        R: Copy + Into<&'static str>,
        C: Copy + Into<&'static str>,
    {
        self.sections.push(Section::Matrix {
            title: title.into(),
            columns: columns.iter().map(|&column| column.into()).collect(),
            rows: rows
                .iter()
                .map(|&row| {
                    let cells = columns.iter().map(|&column| cell(row, column)).collect();
                    (row.into(), cells)
                })
                .collect(),
        });
        self
    }
}

impl RenderOnce for Gallery {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let label = |text: SharedString| {
            div()
                .typeset(density.label)
                .text_color(colors.text_faint)
                .child(text)
        };
        const ROW_LABEL_WIDTH: f32 = 96.;

        div()
            .flex()
            .flex_col()
            .gap_8()
            .p_8()
            .font_family(cx.theme().fonts.ui.clone())
            .typeset(density.body)
            .text_color(colors.text)
            .child(
                div()
                    .typeset(density.heading)
                    .text_color(colors.text_strong)
                    .child(self.title),
            )
            .children(self.sections.into_iter().map(|section| {
                match section {
                    Section::Items { title, items } => {
                        div().flex().flex_col().gap_3().child(label(title)).child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_4()
                                .children(items.into_iter().map(|(name, item)| {
                                    div().flex().flex_col().gap_1p5().child(item).child(
                                        div()
                                            .typeset(density.small)
                                            .text_color(colors.text_muted)
                                            .child(name),
                                    )
                                })),
                        )
                    }
                    Section::Matrix {
                        title,
                        columns,
                        rows,
                    } => div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(label(title))
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .child(div().w(px(ROW_LABEL_WIDTH)).flex_none())
                                .children(columns.into_iter().map(|column| {
                                    div().flex_1().min_w_0().child(label(column.into()))
                                })),
                        )
                        .children(rows.into_iter().map(|(row, cells)| {
                            div()
                                .flex()
                                .items_center()
                                .gap_4()
                                .child(
                                    div()
                                        .w(px(ROW_LABEL_WIDTH))
                                        .flex_none()
                                        .child(label(row.into())),
                                )
                                .children(
                                    cells
                                        .into_iter()
                                        .map(|cell| div().flex().flex_1().min_w_0().child(cell)),
                                )
                        })),
                }
            }))
    }
}

/// A sidebar-colored box to show rows on, as they sit in the app.
#[derive(IntoElement)]
pub(crate) struct Panel {
    width: Pixels,
    children: Vec<AnyElement>,
}

impl Panel {
    pub(crate) fn new(width: Pixels, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        Self {
            width,
            children: children
                .into_iter()
                .map(IntoElement::into_any_element)
                .collect(),
        }
    }
}

impl RenderOnce for Panel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .w(self.width)
            .p_1()
            .rounded(cx.density().radius_lg)
            .bg(cx.theme().colors.surface_panel)
            .children(self.children)
    }
}

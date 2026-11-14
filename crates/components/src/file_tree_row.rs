//! One row of the Files panel's tree. Every level steps in by one arrow and its gap, and a child
//! starts where its parent's name starts, folder or file, so files carry no empty arrow slot.

use std::rc::Rc;

use gpui::{App, ElementId, Hsla, SharedString, Window, div, prelude::*};
use theme::{ActiveTheme, Density, Dp, HAIRLINE, ThemeColors, Typeset, dp};

use crate::{Callback, Icon, IconName};

/// How a row's Git status shows.
#[derive(Clone, Copy)]
pub enum TreeStatus {
    /// A file's name in the status color, or a dot after a folder's name.
    Color(fn(&ThemeColors) -> Hsla),
    /// A letter after the name, such as M, in the status color.
    Letter(char, fn(&ThemeColors) -> Hsla),
}

#[derive(IntoElement)]
pub struct FileTreeRow {
    id: ElementId,
    name: SharedString,
    depth: usize,
    /// Whether a folder is open. `None` for a file.
    folder: Option<bool>,
    status: Option<TreeStatus>,
    ignored: bool,
    guides: bool,
    selected: bool,
    hovered: bool,
    on_click: Option<Callback<usize>>,
    on_hover: Option<Callback<bool>>,
}

impl FileTreeRow {
    pub fn file(id: impl Into<ElementId>, name: impl Into<SharedString>, depth: usize) -> Self {
        Self::new(id, name, depth, None)
    }

    /// A folder, or a chain of single-child folders joined with `/`, open or closed.
    pub fn folder(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        depth: usize,
        open: bool,
    ) -> Self {
        Self::new(id, name, depth, Some(open))
    }

    fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        depth: usize,
        folder: Option<bool>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            depth,
            folder,
            status: None,
            ignored: false,
            guides: true,
            selected: false,
            hovered: false,
            on_click: None,
            on_hover: None,
        }
    }

    pub fn status(mut self, status: Option<TreeStatus>) -> Self {
        self.status = status;
        self
    }

    /// Dims a row that `.gitignore` leaves out.
    pub fn ignored(mut self, ignored: bool) -> Self {
        self.ignored = ignored;
        self
    }

    /// A line under each ancestor's arrow. On by default.
    pub fn guides(mut self, guides: bool) -> Self {
        self.guides = guides;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Shows the hover state without a pointer, for mocks.
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// Called with the click count, so a view can open a file on the second click.
    pub fn on_click(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn on_hover(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }

    /// A folder's arrow: half a row, so it shrinks with the density.
    fn arrow(density: &Density) -> Dp {
        density.row_height / 2.
    }

    /// How far each level steps in: an arrow and its gap, 20 dp in Calm and 16 in Dense.
    pub fn step(density: &Density) -> Dp {
        density.row_height - dp(8.)
    }
}

impl RenderOnce for FileTreeRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = &cx.theme().colors;
        let density = cx.density();
        let (arrow, step) = (Self::arrow(density), Self::step(density));
        let name_color = match self.status {
            _ if self.ignored => colors.text_disabled,
            Some(TreeStatus::Color(color)) if self.folder.is_none() => color(colors),
            _ if self.selected => colors.text_strong,
            _ if self.folder.is_some() => colors.text_muted,
            _ => colors.text,
        };
        let marker = match self.status {
            Some(TreeStatus::Color(color)) if self.folder.is_some() => Some(
                div()
                    .flex_none()
                    .size(dp(6.))
                    .mr_1()
                    .rounded_full()
                    .bg(color(colors))
                    .into_any_element(),
            ),
            Some(TreeStatus::Letter(letter, color)) => Some(
                div()
                    .flex_none()
                    .mr_1()
                    .font_family(cx.theme().fonts.code.clone())
                    .typeset(density.label)
                    .text_color(color(colors))
                    .child(SharedString::from(letter.to_string()))
                    .into_any_element(),
            ),
            _ => None,
        };
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .h(density.row_height - dp(4.))
            .px_2()
            .rounded(density.radius_sm)
            .typeset(density.body)
            .map(|row| {
                if self.selected {
                    row.bg(colors.surface_selected)
                } else if self.hovered {
                    row.bg(colors.surface_raised)
                } else {
                    row.hover(|row| row.bg(colors.surface_raised))
                }
            })
            .child(
                div()
                    .relative()
                    .flex_none()
                    .self_stretch()
                    .w(step * self.depth as f32)
                    .when(self.guides, |indent| {
                        indent.children((0..self.depth).map(|level| {
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(step * level as f32 + arrow / 2.)
                                .w(HAIRLINE)
                                .bg(colors.border_subtle)
                        }))
                    }),
            )
            .children(self.folder.map(|open| {
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(arrow)
                    .mr(step - arrow)
                    .child(
                        Icon::new(if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .size(arrow)
                        .color(if self.ignored {
                            colors.text_disabled
                        } else {
                            colors.text_faint
                        }),
                    )
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(name_color)
                    .child(self.name),
            )
            .children(marker)
            .when_some(self.on_click, |row, handler| {
                row.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event.click_count(), window, cx)
                })
            })
            .when_some(self.on_hover, |row, handler| {
                row.on_hover(move |hovered, window, cx| handler(*hovered, window, cx))
            })
    }
}

#[cfg(feature = "mocks")]
pub(crate) fn mocks() -> Vec<mock::Mock> {
    use crate::gallery::Panel;

    fn rows() -> Vec<FileTreeRow> {
        let modified = TreeStatus::Color(|colors| colors.diff_modified);
        let added = TreeStatus::Color(|colors| colors.diff_added);
        vec![
            FileTreeRow::folder("assets", "assets", 0, false),
            FileTreeRow::folder("crates", "crates", 0, true).status(Some(modified)),
            FileTreeRow::folder("src", "components/src", 1, true).status(Some(modified)),
            FileTreeRow::file("icon", "icon.rs", 2),
            FileTreeRow::file("palette", "palette.rs", 2)
                .status(Some(modified))
                .selected(true),
            FileTreeRow::file("stepper", "stepper.rs", 2).status(Some(added)),
            FileTreeRow::file("toggle", "toggle.rs", 2).hovered(true),
            FileTreeRow::folder("theme", "theme", 1, false),
            FileTreeRow::file("cargo", "Cargo.toml", 1),
            FileTreeRow::folder("target", "target", 0, false).ignored(true),
            FileTreeRow::file("lock", "Cargo.lock", 0)
                .status(Some(TreeStatus::Letter('M', |colors| colors.diff_modified))),
            FileTreeRow::file("readme", "README.md", 0),
        ]
    }
    vec![
        mock::Mock::component("file_tree_row", "gallery", || {
            let column = |rows: Vec<FileTreeRow>| Panel::new(dp(232.), rows).into_any_element();
            crate::Gallery::new("FileTreeRow")
                .section(
                    "Rows",
                    [
                        ("Status as color, a letter on Cargo.lock", column(rows())),
                        (
                            "Without guides",
                            column(rows().into_iter().map(|row| row.guides(false)).collect()),
                        ),
                    ],
                )
                .into_any_element()
        })
        .size(640., 520.),
    ]
}

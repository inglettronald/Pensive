//! A raised list of options that opens under the control that owns it, with a check on the
//! current option, and closes on a click outside it.

use gpui::{
    AnyElement, App, Div, ElementId, SharedString, Stateful, anchored, deferred, div, prelude::*,
};
use theme::{ActiveTheme, Dp, Shadow, Typeset, dp};

use crate::{Icon, IconName};

/// Wide enough for a dropdown's options and the controls that open it to line up.
pub(crate) const MIN_WIDTH: Dp = dp(160.);
/// Past this, an option's trailing text fades out.
const MAX_WIDTH: Dp = dp(320.);
const CHECK: Dp = dp(14.);

/// The menu under its control, which must be `relative`. `on_dismiss` runs on a click outside.
pub(crate) fn menu(
    rows: impl IntoIterator<Item = AnyElement>,
    on_dismiss: impl Fn(&mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let density = cx.density();
    div().absolute().top_full().left_0().mt_1().child(
        deferred(
            anchored().snap_to_window().child(
                div()
                    .id("menu")
                    .occlude()
                    .min_w(MIN_WIDTH)
                    .max_w(MAX_WIDTH)
                    .p_1()
                    .rounded(density.radius_lg)
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.surface_raised)
                    .shadow(Shadow::Medium.layers())
                    .typeset(density.body)
                    .children(rows)
                    .on_mouse_down_out(move |_, _, cx| on_dismiss(cx)),
            ),
        )
        .priority(1),
    )
}

/// What leads an option, in a column of its own so the labels line up.
pub(crate) enum Leading {
    /// The current option, drawn in `text_strong`.
    Check,
    Blank,
    /// An action rather than a value, such as editing the options.
    Icon(IconName),
}

/// One option: its leading mark, then whatever the caller adds after it.
pub(crate) fn menu_row(
    id: impl Into<ElementId>,
    leading: Leading,
    hovered: bool,
    cx: &App,
) -> Stateful<Div> {
    let colors = &cx.theme().colors;
    let density = cx.density();
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .h(density.row_height)
        .px_2()
        .rounded(density.radius_sm)
        .whitespace_nowrap()
        .text_color(match leading {
            Leading::Check => colors.text_strong,
            _ => colors.text,
        })
        .when(hovered, |row| row.bg(colors.surface_selected))
        .hover(|row| row.bg(colors.surface_selected))
        .child(match leading {
            Leading::Check => Icon::new(IconName::Check)
                .size(CHECK)
                .color(colors.text_strong)
                .into_any_element(),
            Leading::Blank => div().flex_none().size(CHECK).into_any_element(),
            Leading::Icon(icon) => Icon::new(icon)
                .size(CHECK)
                .color(colors.text_faint)
                .into_any_element(),
        })
}

/// Names the options under it, such as Saved.
pub(crate) fn menu_section(name: impl Into<SharedString>, cx: &App) -> AnyElement {
    let colors = &cx.theme().colors;
    let density = cx.density();
    div()
        .pt_1p5()
        .pb_0p5()
        .px_2()
        .typeset(density.label)
        .text_color(colors.text_faint)
        .child(name.into())
        .into_any_element()
}

/// A line between groups of options, edge to edge.
pub(crate) fn menu_divider(cx: &App) -> AnyElement {
    div()
        .h(theme::HAIRLINE)
        .my_1()
        .mx_neg_1()
        .bg(cx.theme().colors.border_subtle)
        .into_any_element()
}

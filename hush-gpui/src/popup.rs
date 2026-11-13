//! The completion and history menu, drawn under (or above) the command.

use gpui::{
    anchored, div, point, prelude::*, px, rgb, svg, AnyElement, Bounds, Context, FontWeight,
    MouseButton, Pixels, SharedString,
};

use crate::colors::Palette;
use crate::prompt::{Popup, PopupRow};
use crate::ui::{self, UI_FONT};
use crate::view::TerminalView;

const PADDING: f32 = 4.0;
const TITLE_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 26.0;
const HINTS_HEIGHT: f32 = 27.0;
const GAP: f32 = 4.0;
const MARGIN: f32 = 8.0;

/// Draws `popup` next to `anchor` (the cell it hangs from), below it when it fits
/// inside `area` and above it otherwise.
pub fn render(
    popup: Popup,
    anchor: Bounds<Pixels>,
    area: Bounds<Pixels>,
    palette: &Palette,
    font: SharedString,
    cx: &mut Context<TerminalView>,
) -> AnyElement {
    let rows = popup.rows.len().max(1) as f32;
    let height = 2.0 * PADDING
        + 2.0
        + rows * ROW_HEIGHT
        + if popup.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        }
        + if popup.hints.is_empty() {
            0.0
        } else {
            HINTS_HEIGHT
        };
    let height = px(height);
    let below = anchor.bottom() + px(GAP);
    let top = if below + height <= area.bottom() - px(MARGIN) {
        below
    } else {
        (anchor.top() - height - px(GAP)).max(area.top() + px(MARGIN))
    };
    let left = (anchor.left() - px(MARGIN)).max(area.left() + px(MARGIN));

    let mut menu = div()
        .flex()
        .flex_col()
        .min_w(px(280.))
        .max_w(px(520.))
        .p(px(PADDING))
        .border_1()
        .border_color(rgb(palette.line))
        .rounded(px(9.))
        .bg(rgb(palette.menu))
        .shadow(ui::shadow(palette))
        .font_family(font)
        .text_size(px(12.5))
        .text_color(rgb(palette.foreground))
        // Keep the click from reaching the terminal (which would start a selection).
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

    if let Some(title) = popup.title {
        menu = menu.child(
            div()
                .h(px(TITLE_HEIGHT))
                .px(px(8.))
                .pt(px(6.))
                .font_family(UI_FONT)
                .text_size(px(10.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.muted))
                .whitespace_nowrap()
                .text_ellipsis()
                .child(title.to_uppercase()),
        );
    }

    if popup.rows.is_empty() {
        menu = menu.child(
            div()
                .h(px(ROW_HEIGHT))
                .flex()
                .items_center()
                .px(px(8.))
                .font_family(UI_FONT)
                .text_size(px(12.))
                .text_color(rgb(palette.muted))
                .child(popup.empty.to_owned()),
        );
    }
    for row in popup.rows {
        menu = menu.child(render_row(row, palette, cx));
    }

    if !popup.hints.is_empty() {
        menu = menu.child(
            div()
                .h(px(HINTS_HEIGHT))
                .mt(px(GAP))
                .px(px(8.))
                .pt(px(6.))
                .flex()
                .gap(px(12.))
                .border_t_1()
                .border_color(rgb(palette.line))
                .font_family(UI_FONT)
                .text_size(px(11.))
                .text_color(rgb(palette.muted))
                .children(popup.hints.iter().copied()),
        );
    }

    anchored()
        .position(point(left, top))
        .snap_to_window_with_margin(px(MARGIN))
        .child(menu)
        .into_any_element()
}

fn render_row(row: PopupRow, palette: &Palette, cx: &mut Context<TerminalView>) -> AnyElement {
    let index = row.index;
    let matched = row.matched.min(row.label.len());
    let (head, rest) = row
        .label
        .split_at_checked(matched)
        .unwrap_or(("", row.label));
    let label = div()
        .flex()
        .overflow_hidden()
        .whitespace_nowrap()
        .when(!head.is_empty(), |label| {
            label.child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.accent))
                    .child(head.to_owned()),
            )
        })
        .child(rest.to_owned());
    div()
        .h(px(ROW_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .rounded(px(5.))
        .when(row.selected, |el| el.bg(rgb(palette.menu_selected)))
        .child(
            svg()
                .path(row.icon.path())
                .size(px(14.))
                .flex_none()
                .text_color(rgb(palette.muted)),
        )
        .child(label)
        .when_some(row.detail, |el, detail| {
            el.child(
                div()
                    .ml_auto()
                    .pl(px(18.))
                    .font_family(UI_FONT)
                    .text_size(px(11.5))
                    .text_color(rgb(palette.muted))
                    .whitespace_nowrap()
                    .child(detail.to_owned()),
            )
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, _, _, cx| {
                view.pick(index);
                cx.stop_propagation();
                cx.notify();
            }),
        )
        .into_any_element()
}

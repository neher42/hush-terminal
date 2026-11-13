//! Small building blocks shared by the window's panels.

use gpui::{
    div, hsla, point, prelude::*, px, rgb, svg, AnyView, App, BoxShadow, Context, Div, ElementId,
    FontWeight, MouseButton, SharedString, Stateful, Window,
};

use crate::colors::Palette;
use crate::icons::Icon;

pub const UI_FONT: &str = ".SystemUIFont";

/// Soft two-layer shadow for floating panels.
pub fn shadow(palette: &Palette) -> Vec<BoxShadow> {
    let strength = if palette.dark { 3.0 } else { 1.0 };
    vec![
        BoxShadow {
            color: hsla(0.6, 0.3, 0.07, 0.08 * strength),
            offset: point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(0.),
        },
        BoxShadow {
            color: hsla(0.6, 0.5, 0.12, 0.16 * strength),
            offset: point(px(0.), px(10.)),
            blur_radius: px(28.),
            spread_radius: px(0.),
        },
    ]
}

/// Small dark label, for tooltips and short confirmations.
pub fn label(text: SharedString, palette: &Palette) -> Div {
    div()
        .px(px(8.))
        .py(px(5.))
        .rounded(px(6.))
        .bg(rgb(palette.tooltip))
        .text_color(rgb(palette.tooltip_text))
        .font_family(UI_FONT)
        .text_size(px(11.5))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(text)
}

pub struct Tooltip {
    text: SharedString,
    palette: &'static Palette,
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        label(self.text.clone(), self.palette)
    }
}

/// Builds a tooltip for `.tooltip(...)`.
pub fn tooltip(
    text: impl Into<SharedString>,
    palette: &'static Palette,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| {
        let text = text.clone();
        cx.new(|_| Tooltip { text, palette }).into()
    }
}

/// Uppercase section heading, like "CURRENT DIRECTORY".
pub fn section_label(text: &str, palette: &Palette) -> Div {
    div()
        .font_family(UI_FONT)
        .text_size(px(10.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(palette.muted))
        .child(tracked(&text.to_uppercase()))
}

/// gpui has no letter spacing, so small caps labels get hair spaces between letters
/// (about the 0.07em of the web version).
fn tracked(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 4);
    for (i, ch) in text.chars().enumerate() {
        if i > 0 {
            out.push('\u{200A}');
        }
        out.push(ch);
    }
    out
}

/// A borderless square button with an icon. Clicks don't take focus from the terminal.
pub fn icon_button(
    id: impl Into<ElementId>,
    icon: Icon,
    active: bool,
    palette: &'static Palette,
) -> Stateful<Div> {
    let color = if active {
        palette.accent
    } else {
        palette.muted
    };
    div()
        .id(id)
        .flex_none()
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .text_color(rgb(color))
        .hover(|style| style.bg(rgb(palette.line)))
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .child(svg().path(icon.path()).size(px(15.)).text_color(rgb(color)))
}

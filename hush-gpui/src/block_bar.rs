//! Buttons shown on the command block under the mouse.

use gpui::{
    div, prelude::*, px, rgb, svg, AnyElement, Context, MouseButton, Pixels, SharedString, Window,
};

use crate::colors::Palette;
use crate::icons::Icon;
use crate::popup::UI_FONT;
use crate::view::TerminalView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockAction {
    CopyOutput,
    CopyCommand,
    RunAgain,
    SendToClaude,
}

impl BlockAction {
    fn icon(self) -> Icon {
        match self {
            BlockAction::CopyOutput => Icon::Copy,
            BlockAction::CopyCommand => Icon::Command,
            BlockAction::RunAgain => Icon::Rerun,
            BlockAction::SendToClaude => Icon::Send,
        }
    }

    fn label(self) -> &'static str {
        match self {
            BlockAction::CopyOutput => "Copy output",
            BlockAction::CopyCommand => "Copy command",
            BlockAction::RunAgain => "Run again",
            BlockAction::SendToClaude => "Send to Claude",
        }
    }
}

/// The toolbar, pinned to the right of the block's first visible line.
pub fn render(
    top: Pixels,
    has_output: bool,
    palette: &'static Palette,
    cx: &mut Context<TerminalView>,
) -> AnyElement {
    let actions = [
        BlockAction::CopyOutput,
        BlockAction::CopyCommand,
        BlockAction::RunAgain,
        BlockAction::SendToClaude,
    ];
    div()
        .absolute()
        .top(top)
        .right(px(12.))
        .flex()
        .gap(px(4.))
        .children(
            actions
                .into_iter()
                .filter(|&action| has_output || action != BlockAction::CopyOutput)
                .map(|action| button(action, palette, cx)),
        )
        .into_any_element()
}

fn button(
    action: BlockAction,
    palette: &'static Palette,
    cx: &mut Context<TerminalView>,
) -> AnyElement {
    let group = SharedString::from(action.label());
    div()
        .id(action.label())
        .group(group.clone())
        .w(px(28.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(rgb(palette.line))
        .rounded(px(6.))
        .bg(rgb(palette.background))
        .hover(|style| style.border_color(rgb(palette.muted)))
        .child(
            svg()
                .path(action.icon().path())
                .size(px(15.))
                .text_color(rgb(palette.muted))
                .group_hover(group, |style| style.text_color(rgb(palette.foreground))),
        )
        // Keep focus and any selection in the terminal.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |view, _, _, cx| view.block_action(action, cx)))
        .tooltip(move |_, cx| {
            cx.new(|_| Tooltip {
                text: action.label().into(),
                palette,
            })
            .into()
        })
        .into_any_element()
}

pub struct Tooltip {
    pub text: SharedString,
    pub palette: &'static Palette,
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        label(self.text.clone(), self.palette)
    }
}

/// Small dark label, for tooltips and short confirmations.
pub fn label(text: SharedString, palette: &Palette) -> gpui::Div {
    div()
        .px(px(8.))
        .py(px(4.))
        .rounded(px(5.))
        .bg(rgb(palette.tooltip))
        .text_color(rgb(palette.tooltip_text))
        .font_family(UI_FONT)
        .text_size(px(11.5))
        .whitespace_nowrap()
        .child(text)
}

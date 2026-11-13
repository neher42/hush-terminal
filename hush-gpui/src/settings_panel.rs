//! The settings dialog (⌘,).

use gpui::{
    div, hsla, prelude::*, px, rgb, AnyElement, Context, Div, ElementId, FontWeight, Hsla,
    MouseButton, SharedString, Stateful,
};
use hush_complete::paths::{base_name, display_path, parent_path};

use crate::colors::Palette;
use crate::settings::{clamp_font_size, Appearance, FontChoice, Settings};
use crate::ui::{self, UI_FONT};
use crate::workspace::Workspace;

/// What the dialog shows besides the settings themselves.
pub struct PanelState<'a> {
    pub settings: &'a Settings,
    pub home: &'a str,
    /// Whether Hush may use accessibility (for Send to Claude); `None` while checking.
    pub permission: Option<bool>,
    pub palette: &'static Palette,
}

pub fn render(state: PanelState, cx: &mut Context<Workspace>) -> AnyElement {
    let p = state.palette;
    let s = state.settings;
    let body = div()
        .id("settings-body")
        .flex()
        .flex_col()
        .overflow_y_scroll()
        .px(px(20.))
        .pt(px(6.))
        .pb(px(20.))
        .child(section(
            "Appearance",
            false,
            p,
            vec![segmented(
                "appearance",
                Appearance::ALL.map(|a| (a, a.label(), None)),
                s.appearance,
                p,
                cx,
                |s, appearance| s.appearance = appearance,
            )],
        ))
        .child(section(
            "Text",
            false,
            p,
            vec![
                row(
                    "Size",
                    None,
                    p,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(stepper("font-smaller", "−", p, cx, |s| {
                            s.font_size = clamp_font_size(s.font_size - 1.0)
                        }))
                        .child(
                            div()
                                .min_w(px(52.))
                                .flex()
                                .justify_center()
                                .child(format!("{} pt", s.font_size)),
                        )
                        .child(stepper("font-bigger", "+", p, cx, |s| {
                            s.font_size = clamp_font_size(s.font_size + 1.0)
                        })),
                ),
                row(
                    "Font",
                    None,
                    p,
                    segmented(
                        "font",
                        FontChoice::ALL.map(|f| (f, f.label(), Some(f.family()))),
                        s.font,
                        p,
                        cx,
                        |s, font| s.font = font,
                    ),
                ),
            ],
        ))
        .child(section(
            "Sidebar",
            false,
            p,
            vec![
                row(
                    "Show folder tree",
                    Some("⌘B"),
                    p,
                    switch("sidebar", s.sidebar_visible, p, cx, |s| {
                        s.sidebar_visible = !s.sidebar_visible
                    }),
                ),
                row(
                    "Show git status",
                    None,
                    p,
                    switch("git", s.show_git_status, p, cx, |s| {
                        s.show_git_status = !s.show_git_status
                    }),
                ),
                pins(s, state.home, p, cx),
            ],
        ))
        .child(section(
            "Send to Claude",
            true,
            p,
            vec![
                row(
                    "Accessibility permission",
                    None,
                    p,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(match state.permission {
                            None => div().text_color(rgb(p.muted)).child("Checking…"),
                            Some(true) => div().text_color(rgb(p.ansi[2])).child("Allowed"),
                            Some(false) => div().text_color(rgb(p.ansi[1])).child("Not allowed"),
                        })
                        .child(
                            button("permission", p)
                                .px(px(10.))
                                .py(px(5.))
                                .child("Open Accessibility Settings")
                                .on_click(|_, _, _| hush_core::claude::open_permission_settings()),
                        ),
                ),
                note(
                    "Still not allowed after enabling it? Remove Hush from the list and add it again.",
                    p,
                ),
            ],
        ));

    let panel = div()
        .id("settings")
        .w(px(520.))
        .max_h_full()
        .flex()
        .flex_col()
        .border_1()
        .border_color(rgb(p.line))
        .rounded(px(12.))
        .bg(rgb(p.background))
        .shadow(ui::shadow(p))
        .text_color(rgb(p.foreground))
        .font_family(UI_FONT)
        .text_size(px(13.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt(px(14.))
                .pr(px(14.))
                .pb(px(10.))
                .pl(px(20.))
                .border_b_1()
                .border_color(rgb(p.line))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Settings"),
                )
                .child(
                    div()
                        .id("settings-close")
                        .size(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .text_size(px(18.))
                        .text_color(rgb(p.muted))
                        .hover(|s| s.bg(rgb(p.line)).text_color(rgb(p.foreground)))
                        .child("×")
                        .on_click(cx.listener(|ws, _, window, cx| ws.close_settings(window, cx))),
                ),
        )
        .child(body);

    div()
        .id("settings-backdrop")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p(px(24.))
        .bg(Hsla {
            a: 0.18,
            ..rgb(p.foreground).into()
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|ws, _, window, cx| ws.close_settings(window, cx)),
        )
        .child(panel)
        .into_any_element()
}

fn section(title: &str, last: bool, p: &Palette, children: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .py(px(14.))
        .when(!last, |el| el.border_b_1().border_color(rgb(p.line)))
        .child(ui::section_label(title, p))
        .children(children)
        .into_any_element()
}

fn row(label: &str, hint: Option<&str>, p: &Palette, control: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .min_h(px(28.))
        .child(
            div()
                .flex()
                .gap(px(6.))
                .child(label.to_owned())
                .children(hint.map(|hint| {
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(p.muted))
                        .child(hint.to_owned())
                })),
        )
        .child(control)
        .into_any_element()
}

fn note(text: &str, p: &Palette) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(rgb(p.muted))
        .child(text.to_owned())
        .into_any_element()
}

fn button(id: impl Into<ElementId>, p: &'static Palette) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(rgb(p.line))
        .rounded(px(6.))
        .bg(rgb(p.background))
        .hover(|s| s.bg(rgb(p.hover)).border_color(rgb(p.faint)))
}

fn stepper(
    id: &'static str,
    text: &'static str,
    p: &'static Palette,
    cx: &mut Context<Workspace>,
    change: fn(&mut Settings),
) -> impl IntoElement {
    button(id, p)
        .w(px(28.))
        .h(px(26.))
        .child(text)
        .on_click(cx.listener(move |ws, _, window, cx| ws.update_settings(change, window, cx)))
}

fn segmented<T: Copy + PartialEq + 'static, const N: usize>(
    id: &'static str,
    choices: [(T, &'static str, Option<&'static str>); N],
    current: T,
    p: &'static Palette,
    cx: &mut Context<Workspace>,
    set: fn(&mut Settings, T),
) -> AnyElement {
    let track: Hsla = Hsla {
        a: 0.07,
        ..rgb(p.foreground).into()
    };
    let track = div().flex().p(px(2.)).rounded(px(8.)).bg(track).children(
        choices
            .into_iter()
            .enumerate()
            .map(|(i, (value, label, font))| {
                let selected = value == current;
                div()
                    .id(SharedString::from(format!("{id}-{i}")))
                    .px(px(12.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .text_size(px(12.5))
                    .text_color(rgb(if selected { p.foreground } else { p.secondary }))
                    .when_some(font, |el, font| el.font_family(font))
                    .when(selected, |el| {
                        el.bg(rgb(p.background))
                            .border_1()
                            .border_color(rgb(p.line))
                    })
                    .when(!selected, |el| {
                        el.border_1().border_color(hsla(0., 0., 0., 0.))
                    })
                    .child(label)
                    .on_click(cx.listener(move |ws, _, window, cx| {
                        ws.update_settings(|s| set(s, value), window, cx)
                    }))
            }),
    );
    // In a row of its own, the track stays as wide as its choices.
    div().flex().child(track).into_any_element()
}

fn switch(
    id: &'static str,
    on: bool,
    p: &'static Palette,
    cx: &mut Context<Workspace>,
    toggle: fn(&mut Settings),
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(34.))
        .h(px(20.))
        .p(px(2.))
        .flex()
        .when(on, |el| el.justify_end())
        .rounded_full()
        .bg(rgb(if on { p.accent } else { p.faint }))
        .child(div().size(px(16.)).rounded_full().bg(rgb(0xffffff)))
        .on_click(cx.listener(move |ws, _, window, cx| ws.update_settings(toggle, window, cx)))
}

fn pins(s: &Settings, home: &str, p: &'static Palette, cx: &mut Context<Workspace>) -> AnyElement {
    let heading = div().child("Pinned folders");
    if s.pinned_folders.is_empty() {
        return div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(heading)
            .child(note(
                "Nothing pinned yet. Use the pin button next to the current folder.",
                p,
            ))
            .into_any_element();
    }
    let last = s.pinned_folders.len() - 1;
    let rows = s.pinned_folders.iter().enumerate().map(|(i, path)| {
        let name = if path == home {
            "~".to_owned()
        } else {
            base_name(path).to_owned()
        };
        let action = |key: &str, text: &'static str, enabled: bool| {
            button(SharedString::from(format!("{key}-{i}")), p)
                .w(px(26.))
                .h(px(24.))
                .child(text)
                .when(!enabled, |el| el.opacity(0.4))
        };
        let remove = path.clone();
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .font_family(FontChoice::Menlo.family())
                    .text_size(px(12.5))
                    .child(name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .child(display_path(parent_path(path), home)),
            )
            .child(
                action("up", "↑", i > 0).on_click(cx.listener(move |ws, _, window, cx| {
                    ws.update_settings(|s| s.move_pin(i, false), window, cx)
                })),
            )
            .child(
                action("down", "↓", i < last).on_click(cx.listener(move |ws, _, window, cx| {
                    ws.update_settings(|s| s.move_pin(i, true), window, cx)
                })),
            )
            .child(
                action("remove", "×", true).on_click(cx.listener(move |ws, _, window, cx| {
                    let path = remove.clone();
                    ws.update_settings(move |s| s.toggle_pin(&path), window, cx)
                })),
            )
    });
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(heading)
        .children(rows)
        .into_any_element()
}

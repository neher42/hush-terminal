//! The window: title bar with tabs, the sidebar and the active tab's terminals.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    div, point, prelude::*, px, rgb, size, App, Axis, Bounds, ClipboardItem, Context, DisplayId,
    Entity, FocusHandle, FontWeight, KeyDownEvent, MouseButton, Pixels, Point, SharedString,
    Subscription, Task, TitlebarOptions, Window, WindowAppearance, WindowBounds, WindowOptions,
};
use hush_complete::paths::display_path;
use hush_complete::HistoryStore;
use hush_core::pty::PtyManager;

use crate::actions::*;
use crate::colors::{Palette, DARK, LIGHT};
use crate::icons::Icon;
use crate::panes::Pane;
use crate::settings::{Appearance, CurrentSettings, Settings, SettingsFile, DEFAULT_FONT_SIZE};
use crate::settings_panel::{self, PanelState};
use crate::sidebar::{self, ContextMenu, PathCommand, Sidebar, SidebarEvent};
use crate::ui::{self, UI_FONT};
use crate::view::{Look, TerminalEvent, TerminalView};

const TITLEBAR_HEIGHT: f32 = 40.;
/// Room for the traffic lights.
const TRAFFIC_LIGHTS: f32 = 78.;
const TOAST_TIME: Duration = Duration::from_millis(1600);

struct Tab {
    panes: Pane,
    /// The terminal that gets the keyboard, and whose folder the window shows.
    focused: Entity<TerminalView>,
}

/// What all windows share. The settings themselves are the `CurrentSettings` global.
#[derive(Clone)]
pub struct Shared {
    pub pty: Arc<PtyManager>,
    pub settings_file: Rc<SettingsFile>,
    /// So a command typed in one window is suggested in the others.
    pub history: Rc<RefCell<HistoryStore>>,
}

/// A window's top left corner on a display, relative to that display.
pub struct Place {
    display: Option<DisplayId>,
    origin: Point<Pixels>,
}

/// Opens a window at `place`, or centered on the main display. Its first tab runs a
/// shell in `cwd`, running `run` at the first prompt.
pub fn open_window(
    shared: Shared,
    cwd: String,
    run: Option<String>,
    place: Option<Place>,
    cx: &mut App,
) {
    let extent = size(px(1100.), px(700.));
    let (display, bounds) = match place {
        Some(Place { display, origin }) => (display, Bounds::new(origin, extent)),
        None => (None, Bounds::centered(None, extent, cx)),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: display,
        window_min_size: Some(size(px(520.), px(320.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Hush".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(13.))),
        }),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| Workspace::new(shared, cwd, run, window, cx))
    })
    .expect("failed to open the window");
}

pub struct Workspace {
    shared: Shared,
    home: String,
    tabs: Vec<Tab>,
    active: usize,
    sidebar: Entity<Sidebar>,
    /// This window's copy of `CurrentSettings`, to see what changed.
    settings: Settings,
    settings_focus: Option<FocusHandle>,
    /// Accessibility permission, checked when the settings open.
    permission: Option<bool>,
    menu: Option<ContextMenu>,
    toast: Option<(SharedString, Task<()>)>,
    system_dark: bool,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    fn new(
        shared: Shared,
        cwd: String,
        run: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let home = dirs::home_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".to_owned());
        let settings = cx.global::<CurrentSettings>().0.clone();
        let system_dark = is_dark(window.appearance());
        let palette = palette_for(settings.appearance, system_dark);
        let font = gpui::font(settings.font.family());
        let sidebar = cx.new(|cx| {
            let mut sidebar = Sidebar::new(&home, palette, font, settings.show_git_status, cx);
            sidebar.set_pinned(&settings.pinned_folders, cx);
            sidebar
        });
        let subscriptions = vec![
            cx.subscribe_in(&sidebar, window, Self::on_sidebar_event),
            cx.observe_window_appearance(window, |ws, window, cx| {
                ws.system_dark = is_dark(window.appearance());
                ws.apply_look(cx);
            }),
            cx.observe_window_activation(window, |ws, window, cx| {
                if window.is_window_active() && ws.settings_focus.is_some() {
                    ws.check_permission(cx);
                }
            }),
            cx.observe_global::<CurrentSettings>(|ws, cx| {
                let settings = cx.global::<CurrentSettings>().0.clone();
                ws.apply_settings(settings, cx);
            }),
        ];
        let mut workspace = Self {
            shared,
            home,
            tabs: Vec::new(),
            active: 0,
            sidebar,
            settings,
            settings_focus: None,
            permission: None,
            menu: None,
            toast: None,
            system_dark,
            _subscriptions: subscriptions,
        };
        workspace.open_tab(&cwd, run, window, cx);
        workspace
    }

    fn palette(&self) -> &'static Palette {
        palette_for(self.settings.appearance, self.system_dark)
    }

    fn look(&self) -> Look {
        Look {
            font: gpui::font(self.settings.font.family()),
            font_size: px(self.settings.font_size),
            palette: self.palette(),
        }
    }

    fn active_view(&self) -> &Entity<TerminalView> {
        &self.tabs[self.active].focused
    }

    fn new_terminal(
        &mut self,
        cwd: &str,
        run: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<TerminalView>, Pane) {
        let (pty, history) = (self.shared.pty.clone(), self.shared.history.clone());
        let look = self.look();
        let view = cx.new(|cx| TerminalView::new(pty, cwd, history, look, run, cx));
        let events = cx.subscribe_in(&view, window, Self::on_terminal_event);
        (view.clone(), Pane::terminal(view, events))
    }

    fn open_tab(
        &mut self,
        cwd: &str,
        run: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (focused, panes) = self.new_terminal(cwd, run, window, cx);
        self.tabs.push(Tab { panes, focused });
        self.activate(self.tabs.len() - 1, window, cx);
    }

    /// Opens a terminal in the same folder next to the focused one.
    fn split(&mut self, axis: Axis, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.active_view().clone();
        let cwd = target.read(cx).cwd().to_owned();
        let (view, pane) = self.new_terminal(&cwd, None, window, cx);
        let tab = &mut self.tabs[self.active];
        tab.panes.split(&target, axis, pane);
        self.focus_pane(view, window, cx);
    }

    /// Closes one terminal, and its tab when it was the last one there.
    fn close_pane(
        &mut self,
        view: &Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.tabs.iter().position(|tab| tab.panes.contains(view)) else {
            return;
        };
        let tab = &mut self.tabs[index];
        let order = tab.panes.views();
        if !tab.panes.remove(view) {
            self.close_tab(index, window, cx);
            return;
        }
        if &tab.focused == view {
            // The terminal before it takes the focus, or the next one when it was first.
            let at = order.iter().position(|v| v == view).unwrap_or(0);
            tab.focused = order[if at > 0 { at - 1 } else { 1 }].clone();
        }
        if index == self.active {
            self.activate(index, window, cx);
        } else {
            cx.notify();
        }
    }

    fn focus_pane(
        &mut self,
        view: Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tabs[self.active].focused = view;
        self.activate(self.active, window, cx);
    }

    fn cycle_panes(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let views = self.tabs[self.active].panes.views();
        let len = views.len();
        if len > 1 {
            let at = views
                .iter()
                .position(|v| v == self.active_view())
                .unwrap_or(0);
            let next = if forward { at + 1 } else { at + len - 1 };
            self.focus_pane(views[next % len].clone(), window, cx);
        }
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            // Closing the last tab closes the window, as in a browser; the app quits
            // with its last window.
            window.remove_window();
            return;
        }
        // Like browsers: the tab to the right takes its place, or the new last tab.
        let active = if index < self.active {
            self.active - 1
        } else {
            self.active.min(self.tabs.len() - 1)
        };
        self.activate(active, window, cx);
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.active = index;
        window.focus(self.active_view().read(cx).focus_handle());
        self.location_changed(window, cx);
    }

    /// The active tab moved to another folder, or a different tab became active.
    fn location_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.active_view().read(cx);
        let cwd = view.cwd().to_owned();
        window.set_window_title(&format!("{} — Hush", view.title()));
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_cwd(&cwd, cx));
        cx.notify();
    }

    fn on_terminal_event(
        &mut self,
        view: &Entity<TerminalView>,
        event: &TerminalEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_active = view == self.active_view();
        match event {
            TerminalEvent::Changed if is_active => self.location_changed(window, cx),
            TerminalEvent::Changed => cx.notify(),
            TerminalEvent::CommandFinished if is_active => {
                self.sidebar.update(cx, |sidebar, cx| sidebar.refresh(cx))
            }
            TerminalEvent::CommandFinished => {}
            TerminalEvent::Exited => self.close_pane(view, window, cx),
            TerminalEvent::Notify(text) => self.show_toast(text.clone(), cx),
        }
    }

    fn on_sidebar_event(
        &mut self,
        _: &Entity<Sidebar>,
        event: &SidebarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SidebarEvent::Command(command) => self.run_path_command(command, window, cx),
            SidebarEvent::Menu(menu) => {
                self.menu = Some(menu.clone());
                cx.notify();
            }
            SidebarEvent::Notify(text) => self.show_toast(text.clone(), cx),
        }
    }

    fn run_path_command(
        &mut self,
        command: &PathCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        let view = self.active_view().clone();
        match command {
            PathCommand::Insert { path, is_dir } => {
                view.update(cx, |view, cx| view.insert_path(path, *is_dir, cx))
            }
            PathCommand::Open(path) => view.update(cx, |view, cx| view.open_folder(path, cx)),
            PathCommand::OpenInNewTab(path) => self.open_tab(path, None, window, cx),
            PathCommand::Copy(path) => self.copy(path.clone(), format!("Copied {path}"), cx),
            PathCommand::TogglePin(path) => {
                let path = path.clone();
                self.update_settings(move |s| s.toggle_pin(&path), window, cx);
            }
            PathCommand::Reveal(path) => cx.reveal_path(Path::new(path)),
        }
        window.focus(self.active_view().read(cx).focus_handle());
        cx.notify();
    }

    /// Every settings change goes through here (menu, sidebar, settings dialog), so
    /// applying and saving stay in one place.
    pub fn update_settings(
        &mut self,
        change: impl FnOnce(&mut Settings),
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut settings = self.settings.clone();
        change(&mut settings);
        if settings == self.settings {
            return;
        }
        self.shared.settings_file.save(&settings);
        // Every window, this one included, picks it up from there.
        cx.set_global(CurrentSettings(settings));
    }

    fn apply_settings(&mut self, after: Settings, cx: &mut Context<Self>) {
        let before = std::mem::replace(&mut self.settings, after.clone());
        if after == before {
            return;
        }
        if (after.appearance, after.font, after.font_size)
            != (before.appearance, before.font, before.font_size)
        {
            self.apply_look(cx);
        }
        if after.pinned_folders != before.pinned_folders {
            self.sidebar.update(cx, |sidebar, cx| {
                sidebar.set_pinned(&after.pinned_folders, cx)
            });
        }
        if after.show_git_status != before.show_git_status {
            self.sidebar.update(cx, |sidebar, cx| {
                sidebar.set_show_git(after.show_git_status, cx)
            });
        }
        cx.notify();
    }

    fn apply_look(&mut self, cx: &mut Context<Self>) {
        let look = self.look();
        for view in self.tabs.iter().flat_map(|tab| tab.panes.views()) {
            view.update(cx, |view, cx| view.set_look(look.clone(), cx));
        }
        self.sidebar.update(cx, |sidebar, cx| {
            sidebar.set_look(look.palette, look.font.clone(), cx)
        });
        cx.notify();
    }

    fn copy(&mut self, text: String, confirmation: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_toast(confirmation.into(), cx);
    }

    fn show_toast(&mut self, text: SharedString, cx: &mut Context<Self>) {
        let hide = cx.spawn(async move |ws, cx| {
            cx.background_executor().timer(TOAST_TIME).await;
            let _ = ws.update(cx, |ws, cx| {
                ws.toast = None;
                cx.notify();
            });
        });
        self.toast = Some((text, hide));
        cx.notify();
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        window.focus(&focus);
        self.settings_focus = Some(focus);
        self.check_permission(cx);
        cx.notify();
    }

    pub fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_focus = None;
        window.focus(self.active_view().read(cx).focus_handle());
        cx.notify();
    }

    fn check_permission(&mut self, cx: &mut Context<Self>) {
        let check = cx
            .background_executor()
            .spawn(async { hush_core::claude::has_permission() });
        cx.spawn(async move |ws, cx| {
            let granted = check.await;
            let _ = ws.update(cx, |ws, cx| {
                ws.permission = Some(granted);
                cx.notify();
            });
        })
        .detach();
    }

    fn cycle(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let len = self.tabs.len();
        if len > 1 {
            let next = if forward {
                self.active + 1
            } else {
                self.active + len - 1
            };
            self.activate(next % len, window, cx);
        }
    }

    /// ⌘1–⌘8 pick a tab, ⌘9 the last one, as in browsers.
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if self.menu.is_some() && keystroke.key == "escape" {
            self.menu = None;
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if self.settings_focus.is_some() && keystroke.key == "escape" {
            self.close_settings(window, cx);
            cx.stop_propagation();
            return;
        }
        let m = &keystroke.modifiers;
        if !m.platform || m.control || m.alt || m.shift {
            return;
        }
        if let Some(n) = keystroke
            .key
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=9).contains(n))
        {
            let index = if n == 9 { self.tabs.len() - 1 } else { n - 1 };
            self.activate(index, window, cx);
            cx.stop_propagation();
        }
    }

    fn render_titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let middle = if self.tabs.len() > 1 {
            self.render_tabs(cx).into_any_element()
        } else {
            let view = self.active_view().read(cx);
            let title = view.title();
            let place = display_path(view.cwd(), &self.home);
            div()
                .id("title")
                .flex_1()
                .min_w_0()
                .flex()
                .justify_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(p.secondary))
                        .child(title.clone()),
                )
                .when(place != title, |el| el.child(format!(" — {place}")))
                .on_click(|event, window, _| {
                    if event.click_count() == 2 {
                        window.titlebar_double_click();
                    }
                })
                .into_any_element()
        };
        div()
            .flex_none()
            .h(px(TITLEBAR_HEIGHT))
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(TRAFFIC_LIGHTS))
            .pr(px(8.))
            .bg(rgb(p.chrome))
            .border_b_1()
            .border_color(rgb(p.line))
            .font_family(UI_FONT)
            .text_size(px(12.5))
            .text_color(rgb(p.muted))
            .child(
                ui::icon_button("toggle-sidebar", Icon::Sidebar, false, p)
                    .tooltip(ui::tooltip("Folder tree  ⌘B", p))
                    .on_click(cx.listener(|ws, _, window, cx| {
                        ws.update_settings(|s| s.sidebar_visible = !s.sidebar_visible, window, cx)
                    })),
            )
            .child(middle)
            .child(
                ui::icon_button("new-tab", Icon::Plus, false, p)
                    .tooltip(ui::tooltip("New tab  ⌘T", p))
                    .on_click(cx.listener(|ws, _, window, cx| ws.new_tab(&NewTab, window, cx))),
            )
            .child(
                ui::icon_button("split", Icon::Split, false, p)
                    .tooltip(ui::tooltip("Split  ⌘D", p))
                    .on_click(
                        cx.listener(|ws, _, window, cx| ws.split(Axis::Horizontal, window, cx)),
                    ),
            )
            .child(
                ui::icon_button("copy-path", Icon::Copy, false, p)
                    .tooltip(ui::tooltip("Copy path  ⌘⇧C", p))
                    .on_click(cx.listener(|ws, _, window, cx| ws.copy_path(&CopyPath, window, cx))),
            )
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let wash = |alpha: f32| gpui::Hsla {
            a: alpha,
            ..rgb(p.foreground).into()
        };
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(4.))
            .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                let active = i == self.active;
                let title = tab.focused.read(cx).title();
                let group = SharedString::from(format!("tab-{i}"));
                div()
                    .id(("tab", i))
                    .group(group.clone())
                    .flex_1()
                    .min_w(px(64.))
                    .max_w(px(220.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .pl(px(4.))
                    .pr(px(10.))
                    .rounded(px(7.))
                    .when(active, |el| {
                        el.bg(rgb(p.background))
                            .text_color(rgb(p.foreground))
                            .border_1()
                            .border_color(rgb(p.line))
                    })
                    .when(!active, |el| el.bg(wash(0.05)).hover(|s| s.bg(wash(0.09))))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(cx.listener(move |ws, _, window, cx| ws.activate(i, window, cx)))
                    .on_mouse_up(
                        MouseButton::Middle,
                        cx.listener(move |ws, _, window, cx| ws.close_tab(i, window, cx)),
                    )
                    .child(
                        div()
                            .id(("close-tab", i))
                            .flex_none()
                            .size(px(18.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .text_size(px(15.))
                            .when(!active, |el| {
                                el.invisible().group_hover(group, |s| s.visible())
                            })
                            .hover(|s| s.bg(rgb(p.line)).text_color(rgb(p.foreground)))
                            .child("×")
                            .on_click(cx.listener(move |ws, _, window, cx| {
                                cx.stop_propagation();
                                ws.close_tab(i, window, cx);
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .justify_center()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(title),
                    )
            }))
    }

    // Menu actions.

    fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.active_view().read(cx).cwd().to_owned();
        self.open_tab(&cwd, None, window, cx);
    }

    /// A window in the same folder, a little below and right of this one.
    fn new_window(&mut self, _: &NewWindow, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.active_view().read(cx).cwd().to_owned();
        let place = place_from(window, point(px(30.), px(30.)), cx);
        open_window(self.shared.clone(), cwd, None, place, cx);
    }

    /// ⌘W closes the settings, the focused terminal, or the tab with its last terminal.
    fn close(&mut self, _: &Close, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_focus.is_some() {
            self.close_settings(window, cx);
        } else {
            let view = self.active_view().clone();
            self.close_pane(&view, window, cx);
        }
    }

    fn copy_path(&mut self, _: &CopyPath, _window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.active_view().read(cx).cwd().to_owned();
        self.copy(cwd.clone(), format!("Copied {cwd}"), cx);
    }

    fn appearance(&mut self, appearance: Appearance, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.appearance = appearance, window, cx);
    }

    fn font_size(
        &mut self,
        size: impl FnOnce(f32) -> f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update_settings(
            |s| s.font_size = crate::settings::clamp_font_size(size(s.font_size)),
            window,
            cx,
        );
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        // Clicking a terminal focuses it; the window follows along.
        let panes = self.tabs[self.active].panes.render(p, &|view| {
            let clicked = view.clone();
            div()
                .size_full()
                .capture_any_mouse_down(cx.listener(move |ws, _, window, cx| {
                    if ws.active_view() != &clicked {
                        ws.focus_pane(clicked.clone(), window, cx);
                    }
                }))
        });
        let settings = self.settings_focus.clone().map(|focus| {
            div()
                .track_focus(&focus)
                .absolute()
                .inset_0()
                .child(settings_panel::render(
                    PanelState {
                        settings: &self.settings,
                        home: &self.home,
                        permission: self.permission,
                        palette: p,
                    },
                    cx,
                ))
        });
        let toast = self.toast.as_ref().map(|(text, _)| {
            div()
                .absolute()
                .bottom(px(16.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(ui::label(text.clone(), p))
        });
        let menu = self.menu.as_ref().map(|menu| {
            let ws = cx.entity().downgrade();
            sidebar::render_menu(menu, p, move |command, window, cx| {
                let _ = ws.update(cx, |ws, cx| ws.run_path_command(command, window, cx));
            })
        });
        div()
            .id("workspace")
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(p.background))
            .text_color(rgb(p.foreground))
            .capture_any_mouse_down(cx.listener(|ws, _, _, cx| {
                if ws.menu.take().is_some() {
                    cx.notify();
                }
            }))
            .capture_key_down(cx.listener(Self::key_down))
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::new_window))
            .on_action(cx.listener(Self::close))
            .on_action(
                cx.listener(|ws, _: &SplitRight, window, cx| {
                    ws.split(Axis::Horizontal, window, cx)
                }),
            )
            .on_action(
                cx.listener(|ws, _: &SplitDown, window, cx| ws.split(Axis::Vertical, window, cx)),
            )
            .on_action(cx.listener(Self::copy_path))
            .on_action(cx.listener(|ws, _: &NextTab, window, cx| ws.cycle(true, window, cx)))
            .on_action(cx.listener(|ws, _: &PreviousTab, window, cx| ws.cycle(false, window, cx)))
            .on_action(cx.listener(|ws, _: &NextPane, window, cx| ws.cycle_panes(true, window, cx)))
            .on_action(
                cx.listener(|ws, _: &PreviousPane, window, cx| ws.cycle_panes(false, window, cx)),
            )
            .on_action(cx.listener(|ws, _: &OpenSettings, window, cx| ws.open_settings(window, cx)))
            .on_action(cx.listener(|ws, _: &ToggleSidebar, window, cx| {
                ws.update_settings(|s| s.sidebar_visible = !s.sidebar_visible, window, cx)
            }))
            .on_action(cx.listener(|ws, _: &AppearanceSystem, window, cx| {
                ws.appearance(Appearance::System, window, cx)
            }))
            .on_action(cx.listener(|ws, _: &AppearanceLight, window, cx| {
                ws.appearance(Appearance::Light, window, cx)
            }))
            .on_action(cx.listener(|ws, _: &AppearanceDark, window, cx| {
                ws.appearance(Appearance::Dark, window, cx)
            }))
            .on_action(cx.listener(|ws, _: &BiggerText, window, cx| {
                ws.font_size(|size| size + 1.0, window, cx)
            }))
            .on_action(cx.listener(|ws, _: &SmallerText, window, cx| {
                ws.font_size(|size| size - 1.0, window, cx)
            }))
            .on_action(cx.listener(|ws, _: &ActualSize, window, cx| {
                ws.font_size(|_| DEFAULT_FONT_SIZE, window, cx)
            }))
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Zoom, window, _| window.zoom_window())
            .child(self.render_titlebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(self.settings.sidebar_visible, |el| {
                        el.child(self.sidebar.clone())
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .relative()
                            .child(panes)
                            .children(toast),
                    ),
            )
            .children(settings)
            .children(menu)
    }
}

/// The place at `offset` from the window's top left corner, on the window's display.
fn place_from(window: &Window, offset: Point<Pixels>, cx: &App) -> Option<Place> {
    match window.window_bounds() {
        WindowBounds::Windowed(bounds) => Some(Place {
            display: window.display(cx).map(|display| display.id()),
            origin: bounds.origin + offset,
        }),
        _ => None,
    }
}

fn is_dark(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

fn palette_for(appearance: Appearance, system_dark: bool) -> &'static Palette {
    match appearance {
        Appearance::Light => &LIGHT,
        Appearance::Dark => &DARK,
        Appearance::System if system_dark => &DARK,
        Appearance::System => &LIGHT,
    }
}

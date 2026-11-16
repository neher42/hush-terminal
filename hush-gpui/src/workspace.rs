//! The window: title bar with tabs, the sidebar and the active terminal.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    div, point, prelude::*, px, rgb, size, App, Bounds, ClipboardItem, Context, DragMoveEvent,
    Entity, FocusHandle, FontWeight, KeyDownEvent, MouseButton, MouseUpEvent, Pixels, Point,
    SharedString, Subscription, Task, TitlebarOptions, Window, WindowAppearance, WindowBounds,
    WindowOptions,
};
use hush_complete::paths::display_path;
use hush_complete::HistoryStore;
use hush_core::pty::PtyManager;

use crate::actions::*;
use crate::colors::{Palette, DARK, LIGHT};
use crate::icons::Icon;
use crate::settings::{Appearance, Settings, SettingsFile, DEFAULT_FONT_SIZE};
use crate::settings_panel::{self, PanelState};
use crate::sidebar::{self, ContextMenu, PathCommand, Sidebar, SidebarEvent};
use crate::ui::{self, UI_FONT};
use crate::view::{Look, TerminalEvent, TerminalView};

const TITLEBAR_HEIGHT: f32 = 40.;
/// Room for the traffic lights.
const TRAFFIC_LIGHTS: f32 = 78.;
const TOAST_TIME: Duration = Duration::from_millis(1600);

struct Tab {
    view: Entity<TerminalView>,
    _events: Subscription,
}

/// A tab being dragged: dropped on another tab it reorders, dropped outside the
/// window it becomes a window of its own.
#[derive(Clone)]
struct DraggedTab {
    index: usize,
    title: SharedString,
    palette: &'static Palette,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .rounded(px(7.))
            .bg(rgb(self.palette.background))
            .border_1()
            .border_color(rgb(self.palette.line))
            .text_color(rgb(self.palette.foreground))
            .font_family(UI_FONT)
            .text_size(px(12.))
            .whitespace_nowrap()
            .child(self.title.clone())
    }
}

/// What a window starts with.
pub enum WindowContent {
    /// A new shell in `cwd` (the home folder by default), running `run` at the first prompt.
    Shell {
        cwd: Option<String>,
        run: Option<String>,
    },
    /// A tab dragged out of another window, with its shell still running.
    Tab(Entity<TerminalView>),
}

/// Opens a window. Windows keep their own tabs but share the ptys and the settings file.
pub fn open_window(
    pty: Arc<PtyManager>,
    settings_file: SettingsFile,
    content: WindowContent,
    origin: Option<Point<Pixels>>,
    cx: &mut App,
) {
    let extent = size(px(1100.), px(700.));
    let bounds = match origin {
        Some(origin) => Bounds::new(origin, extent),
        None => Bounds::centered(None, extent, cx),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(520.), px(320.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Hush".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(13.))),
        }),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| Workspace::new(pty, settings_file, content, window, cx))
    })
    .expect("failed to open the window");
}

pub struct Workspace {
    pty: Arc<PtyManager>,
    home: String,
    history: Rc<RefCell<HistoryStore>>,
    tabs: Vec<Tab>,
    active: usize,
    /// The tab being dragged, while a drag is going on.
    dragging: Option<usize>,
    sidebar: Entity<Sidebar>,
    settings: Settings,
    settings_file: SettingsFile,
    settings_focus: Option<FocusHandle>,
    /// Accessibility permission, checked when the settings open.
    permission: Option<bool>,
    menu: Option<ContextMenu>,
    toast: Option<(SharedString, Task<()>)>,
    system_dark: bool,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(
        pty: Arc<PtyManager>,
        settings_file: SettingsFile,
        content: WindowContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let home = dirs::home_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".to_owned());
        let settings = settings_file.load();
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
        ];
        let mut workspace = Self {
            pty,
            home: home.clone(),
            history: Rc::default(),
            tabs: Vec::new(),
            active: 0,
            dragging: None,
            sidebar,
            settings,
            settings_file,
            settings_focus: None,
            permission: None,
            menu: None,
            toast: None,
            system_dark,
            _subscriptions: subscriptions,
        };
        match content {
            WindowContent::Shell { cwd, run } => {
                let cwd = cwd.unwrap_or_else(|| home.clone());
                workspace.open_tab(&cwd, run, window, cx);
            }
            WindowContent::Tab(view) => workspace.adopt_tab(view, window, cx),
        }
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
        &self.tabs[self.active].view
    }

    fn open_tab(
        &mut self,
        cwd: &str,
        run: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (pty, history, look) = (self.pty.clone(), self.history.clone(), self.look());
        let view = cx.new(|cx| TerminalView::new(pty, cwd, history, look, run, cx));
        let events = cx.subscribe_in(&view, window, Self::on_terminal_event);
        self.tabs.push(Tab {
            view,
            _events: events,
        });
        self.activate(self.tabs.len() - 1, window, cx);
    }

    /// Takes in a tab dragged out of another window; its shell keeps running.
    fn adopt_tab(
        &mut self,
        view: Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let look = self.look();
        view.update(cx, |view, cx| view.set_look(look, cx));
        let events = cx.subscribe_in(&view, window, Self::on_terminal_event);
        self.tabs.push(Tab {
            view,
            _events: events,
        });
        self.activate(self.tabs.len() - 1, window, cx);
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            // Closing the last tab closes the window, as in a browser.
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

    /// Moves a dragged tab to where the tab at `to` sits.
    fn move_tab(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.dragging = None;
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.activate(to, window, cx);
    }

    /// Opens the tab at `index` in a window of its own, keeping its shell running.
    /// The last tab stays put: a window always has one.
    fn detach_tab(
        &mut self,
        index: usize,
        mouse: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tabs.len() < 2 || index >= self.tabs.len() {
            return;
        }
        let view = self.tabs.remove(index).view;
        // As in close_tab: the tab to the right takes its place, or the new last tab.
        let active = if index < self.active {
            self.active - 1
        } else {
            self.active.min(self.tabs.len() - 1)
        };
        self.activate(active, window, cx);
        let origin = offset_origin(window, mouse - point(px(80.), px(20.)));
        open_window(
            self.pty.clone(),
            self.settings_file.clone(),
            WindowContent::Tab(view),
            origin,
            cx,
        );
    }

    /// A tab let go past the edge of the window becomes its own window.
    fn tab_released_outside(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.dragging.take() else {
            return;
        };
        if cx.has_active_drag() {
            self.detach_tab(index, event.position, window, cx);
        }
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
            TerminalEvent::Exited => {
                if let Some(index) = self.tabs.iter().position(|tab| &tab.view == view) {
                    self.close_tab(index, window, cx);
                }
            }
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
        let before = self.settings.clone();
        change(&mut self.settings);
        let after = self.settings.clone();
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
        self.settings_file.save(&self.settings);
        cx.notify();
    }

    fn apply_look(&mut self, cx: &mut Context<Self>) {
        let look = self.look();
        for tab in &self.tabs {
            tab.view
                .update(cx, |view, cx| view.set_look(look.clone(), cx));
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
                let title = tab.view.read(cx).title();
                let group = SharedString::from(format!("tab-{i}"));
                let dragged = DraggedTab {
                    index: i,
                    title: SharedString::from(title.clone()),
                    palette: p,
                };
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
                    .on_drag(dragged, |tab, _, _, cx| cx.new(|_| tab.clone()))
                    .on_drop(cx.listener(move |ws, tab: &DraggedTab, window, cx| {
                        ws.move_tab(tab.index, i, window, cx)
                    }))
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

    fn new_window(&mut self, _: &NewWindow, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.active_view().read(cx).cwd().to_owned();
        let origin = offset_origin(window, point(px(30.), px(30.)));
        open_window(
            self.pty.clone(),
            self.settings_file.clone(),
            WindowContent::Shell {
                cwd: Some(cwd),
                run: None,
            },
            origin,
            cx,
        );
    }

    fn close_active_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_focus.is_some() {
            self.close_settings(window, cx);
        } else {
            self.close_tab(self.active, window, cx);
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
            .on_drag_move(cx.listener(|ws, event: &DragMoveEvent<DraggedTab>, _, cx| {
                ws.dragging = Some(event.drag(cx).index);
            }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::tab_released_outside))
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::new_window))
            .on_action(cx.listener(Self::close_active_tab))
            .on_action(cx.listener(Self::copy_path))
            .on_action(cx.listener(|ws, _: &NextTab, window, cx| ws.cycle(true, window, cx)))
            .on_action(cx.listener(|ws, _: &PreviousTab, window, cx| ws.cycle(false, window, cx)))
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
                            .child(self.active_view().clone())
                            .children(toast),
                    ),
            )
            .children(settings)
            .children(menu)
    }
}

/// The screen position `delta` away from the window's top left corner, if it has one.
fn offset_origin(window: &Window, delta: Point<Pixels>) -> Option<Point<Pixels>> {
    match window.window_bounds() {
        WindowBounds::Windowed(bounds) => Some(bounds.origin + delta),
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

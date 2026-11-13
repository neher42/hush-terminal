use std::sync::Arc;
use std::time::Duration;

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use futures::StreamExt;
use gpui::{
    div, point, prelude::*, px, size, Bounds, ClipboardItem, Context, FocusHandle, Font,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    ScrollWheelEvent, SharedString, Task, Window,
};
use hush_complete::{CommandCatalog, FolderCheck, HistoryStore, Place};
use hush_core::claude::{self, Delivery};
use hush_core::events::ShellEvent;
use hush_core::executables::list_executables;
use hush_core::history::read_zsh_history;
use hush_core::pty::PtyManager;
use hush_editor::{resolve_key, Key};

use crate::block_bar::{self, BlockAction};
use crate::blocks::{self, Block, BlockLog};
use crate::colors::{Palette, LIGHT};
use crate::element::{Geometry, TerminalElement};
use crate::keys::{self, Mods};
use crate::popup;
use crate::prompt::{Env, PromptEditor, Submitted};
use crate::terminal::{GridSize, Marker, Output, Terminal};

const PADDING: f32 = 8.0;
/// How long to wait for zsh to redraw its prompt after a resize before drawing the
/// command again ourselves.
const REDRAW_FALLBACK: Duration = Duration::from_millis(250);
const NOTICE_TIME: Duration = Duration::from_millis(1600);

pub struct TerminalView {
    pub(crate) terminal: Terminal,
    prompt: PromptEditor,
    blocks: BlockLog,
    history: HistoryStore,
    catalog: CommandCatalog,
    folders: FolderCheck,
    cwd: String,
    home: String,
    focus: FocusHandle,
    pub(crate) font: Font,
    pub(crate) font_size: Pixels,
    pub(crate) palette: &'static Palette,
    geometry: Option<Geometry>,
    /// The mouse went down on the terminal itself (not on a button or menu).
    pressed: bool,
    /// Where a mouse drag started; the selection begins once the mouse moves.
    drag_start: Option<(Point, Side)>,
    mouse: Option<gpui::Point<Pixels>>,
    /// The block under the mouse, which shows its buttons.
    pub(crate) hovered: Option<Block>,
    toast: Option<(SharedString, Task<()>)>,
    notice_timer: Option<(u64, Task<()>)>,
    /// Fractional lines left over from smooth scrolling.
    scroll_remainder: f32,
    redraw_fallback: Option<Task<()>>,
    /// Runs once the first prompt is ready (`hush-gpui --run "command"`).
    pending_run: Option<String>,
}

impl TerminalView {
    pub fn new(
        pty: Arc<PtyManager>,
        run: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial = GridSize {
            cols: 100,
            rows: 30,
            cell_width: 8,
            cell_height: 17,
        };
        let home = dirs::home_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".to_owned());
        let (terminal, mut output) =
            Terminal::spawn(pty, Some(home.as_ref()), initial).expect("failed to start the shell");

        // Output arrives in bursts; handle everything queued, then redraw once.
        cx.spawn(async move |this, cx| {
            while let Some(first) = output.next().await {
                let mut batch = vec![first];
                while let Ok(next) = output.try_recv() {
                    batch.push(next);
                }
                let alive = this.update(cx, |view, cx| {
                    for item in batch {
                        view.handle_output(item, cx);
                    }
                    view.update_hover();
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        let focus = cx.focus_handle();
        window.focus(&focus);
        Self {
            terminal,
            prompt: PromptEditor::default(),
            blocks: BlockLog::default(),
            history: HistoryStore::default(),
            catalog: CommandCatalog::default(),
            folders: FolderCheck::default(),
            cwd: home.clone(),
            home,
            focus,
            font: gpui::font("Menlo"),
            font_size: px(13.5),
            palette: &LIGHT,
            geometry: None,
            pressed: false,
            drag_start: None,
            mouse: None,
            hovered: None,
            toast: None,
            notice_timer: None,
            scroll_remainder: 0.0,
            redraw_fallback: None,
            pending_run: run,
        }
    }

    /// Runs `f` with the prompt editor and everything it works with.
    fn with_prompt<R>(&mut self, f: impl FnOnce(&mut PromptEditor, &mut Env) -> R) -> R {
        let mut env = Env {
            terminal: &mut self.terminal,
            history: &self.history,
            catalog: &self.catalog,
            folders: &self.folders,
            place: Place {
                cwd: &self.cwd,
                home: &self.home,
            },
        };
        f(&mut self.prompt, &mut env)
    }

    fn handle_output(&mut self, output: Output, cx: &mut Context<Self>) {
        match output {
            Output::Wakeup => self.terminal.woke(),
            Output::Shell(marker) => self.handle_shell_event(marker, cx),
            Output::Term(Event::PtyWrite(text)) => self.terminal.write(text.as_bytes()),
            Output::Term(_) => {}
            Output::Exit => cx.quit(),
        }
    }

    fn handle_shell_event(&mut self, marker: Marker, cx: &mut Context<Self>) {
        let Marker {
            event,
            cursor,
            block,
        } = marker;
        match event {
            ShellEvent::CommandInputStart => {
                self.redraw_fallback = None;
                self.with_prompt(|prompt, env| prompt.begin_input(cursor, block, env));
                if let Some(command) = self.pending_run.take() {
                    self.run(&command);
                }
            }
            ShellEvent::CommandStart => self.prompt.deactivate(),
            ShellEvent::CommandEnd { exit_code } => {
                self.blocks.finished(block, exit_code);
                self.folders.clear();
            }
            ShellEvent::Cwd { path, .. } => self.cwd = path,
            ShellEvent::Meta { key, value } => match key.as_str() {
                "histfile" => {
                    let entries = cx
                        .background_executor()
                        .spawn(async move { read_zsh_history(value.as_ref()) });
                    cx.spawn(async move |this, cx| {
                        let entries = entries.await;
                        let _ = this.update(cx, |view, _| view.history.load(entries));
                    })
                    .detach();
                }
                "path" => {
                    let names = cx
                        .background_executor()
                        .spawn(async move { list_executables(&value) });
                    cx.spawn(async move |this, cx| {
                        let names = names.await;
                        let _ = this.update(cx, |view, _| view.catalog.set_executables(names));
                    })
                    .detach();
                }
                "names" => self
                    .catalog
                    .set_shell_names(value.split_whitespace().map(str::to_owned).collect()),
                _ => {}
            },
            ShellEvent::PromptStart => {}
        }
    }

    fn run(&mut self, command: &str) {
        let submitted = self.with_prompt(|prompt, env| prompt.run_command(command, env));
        self.submitted(submitted);
    }

    fn submitted(&mut self, submitted: Option<Submitted>) {
        if let Some(Submitted { block, command }) = submitted {
            self.history.add(&command);
            self.blocks.ran(block, command);
        }
    }

    /// A menu item was clicked.
    pub(crate) fn pick(&mut self, index: usize) {
        self.with_prompt(|prompt, env| prompt.pick(index, env));
    }

    pub(crate) fn block_action(&mut self, action: BlockAction, cx: &mut Context<Self>) {
        let Some(block) = self.hovered else {
            return;
        };
        let Some(ran) = self.blocks.get(block.id).cloned() else {
            return;
        };
        let output = || blocks::output_text(&self.terminal.term.lock(), &block);
        match action {
            BlockAction::CopyOutput => self.copy(output(), "Copied output", cx),
            BlockAction::CopyCommand => self.copy(ran.command, "Copied command", cx),
            BlockAction::RunAgain => self.run(&ran.command),
            BlockAction::SendToClaude => {
                let text = blocks::transcript(&ran, &output());
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                let delivery = cx.background_executor().spawn(async { claude::deliver() });
                cx.spawn(async move |this, cx| {
                    if let Delivery::Copied { message } = delivery.await {
                        let _ = this.update(cx, |view, cx| view.show_toast(message, cx));
                    }
                })
                .detach();
            }
        }
        cx.notify();
    }

    fn copy(&mut self, text: String, confirmation: &'static str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_toast(confirmation, cx);
    }

    fn show_toast(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let hide = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(NOTICE_TIME).await;
            let _ = this.update(cx, |view, cx| {
                view.toast = None;
                cx.notify();
            });
        });
        self.toast = Some((text.into(), hide));
        cx.notify();
    }

    /// Hides a "No matches" notice after a moment.
    fn watch_notice(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.prompt.notice_id() else {
            return;
        };
        if self
            .notice_timer
            .as_ref()
            .is_some_and(|(shown, _)| *shown == id)
        {
            return;
        }
        let hide = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(NOTICE_TIME).await;
            let _ = this.update(cx, |view, cx| {
                view.prompt.dismiss_notice(id);
                cx.notify();
            });
        });
        self.notice_timer = Some((id, hide));
    }

    /// Finds the block under the mouse. Returns whether it changed.
    fn update_hover(&mut self) -> bool {
        let hovered = self.mouse.and_then(|position| {
            let (point, _) = self.point_at(position)?;
            let term = self.terminal.term.lock();
            if term.mode().contains(TermMode::ALT_SCREEN) {
                return None;
            }
            let block = blocks::block_at(&term, point.line)?;
            let ran = self.blocks.get(block.id)?;
            (!ran.command.is_empty()).then_some(block)
        });
        let changed = hovered != self.hovered;
        self.hovered = hovered;
        changed
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = &keystroke.modifiers;
        if m.platform && self.app_shortcut(&keystroke.key, cx) {
            cx.stop_propagation();
            cx.notify();
            return;
        }

        if self.prompt.is_active() {
            let key = Key {
                key: &keystroke.key,
                key_char: keystroke.key_char.as_deref(),
                ctrl: m.control,
                alt: m.alt,
                shift: m.shift,
                cmd: m.platform,
            };
            if let Some(command) = resolve_key(&key, self.prompt.menu_open()) {
                let submitted = self.with_prompt(|prompt, env| prompt.execute(command, env));
                self.submitted(submitted);
            } else if let Some(text) = keystroke.key_char.as_deref() {
                if !m.control && !m.platform {
                    let text = text.to_owned();
                    self.with_prompt(|prompt, env| prompt.insert(&text, env));
                }
            }
            self.watch_notice(cx);
            cx.stop_propagation();
            cx.notify();
            return;
        }

        if m.platform {
            return;
        }
        let app_cursor = self.mode().contains(TermMode::APP_CURSOR);
        let mods = Mods {
            ctrl: m.control,
            alt: m.alt,
            shift: m.shift,
        };
        if let Some(bytes) = keys::to_bytes(
            &keystroke.key,
            keystroke.key_char.as_deref(),
            mods,
            app_cursor,
        ) {
            self.terminal.term.lock().scroll_display(Scroll::Bottom);
            self.terminal.write(&bytes);
            cx.stop_propagation();
        }
    }

    /// ⌘ shortcuts. These move to the app menu once there is one.
    fn app_shortcut(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        match key {
            "q" => cx.quit(),
            "c" => {
                if let Some(text) = self.terminal.term.lock().selection_to_string() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "v" => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.paste(&text);
                }
            }
            "x" => {
                if let Some(text) = self.with_prompt(|prompt, env| prompt.cut(env)) {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "a" => {
                if !self.with_prompt(|prompt, env| prompt.select_all(env)) {
                    self.select_everything();
                }
            }
            "k" => self.with_prompt(|prompt, env| prompt.clear_screen(env)),
            _ => return false,
        }
        true
    }

    fn paste(&mut self, text: &str) {
        if self.prompt.is_active() {
            let text = text.trim_end_matches(['\n', '\r']).to_owned();
            self.with_prompt(|prompt, env| prompt.insert(&text, env));
            return;
        }
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        let data = if self.mode().contains(TermMode::BRACKETED_PASTE) {
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text
        };
        self.terminal.write(data.as_bytes());
    }

    fn select_everything(&mut self) {
        let mut term = self.terminal.term.lock();
        let start = Point::new(term.topmost_line(), Column(0));
        let end = Point::new(term.bottommost_line(), term.last_column());
        let mut selection = Selection::new(SelectionType::Lines, start, Side::Left);
        selection.update(end, Side::Right);
        term.selection = Some(selection);
    }

    fn mode(&self) -> TermMode {
        *self.terminal.term.lock().mode()
    }

    /// Called while painting with the grid's position and cell size.
    pub(crate) fn set_geometry(&mut self, geometry: Geometry, cx: &mut Context<Self>) {
        self.geometry = Some(geometry);
        let size = geometry.grid;
        if size == self.terminal.size() {
            return;
        }
        self.terminal.resize(size);
        self.with_prompt(|prompt, env| prompt.after_resize(env));
        if self.prompt.is_active() {
            self.redraw_fallback = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(REDRAW_FALLBACK).await;
                let _ = this.update(cx, |view, cx| {
                    view.with_prompt(|prompt, env| prompt.render(env));
                    cx.notify();
                });
            }));
        }
    }

    /// The grid cell under a window position, and which half of it.
    fn point_at(&self, position: gpui::Point<Pixels>) -> Option<(Point, Side)> {
        let g = self.geometry?;
        let x = f32::from(position.x - g.origin.x) / f32::from(g.cell_width);
        let y = f32::from(position.y - g.origin.y) / f32::from(g.line_height);
        let col = x.max(0.0).min(f32::from(g.grid.cols) - 1.0);
        let row = y.max(0.0).min(f32::from(g.grid.rows) - 1.0) as i32;
        let side = if x - col.floor() > 0.5 {
            Side::Right
        } else {
            Side::Left
        };
        let offset = self.terminal.term.lock().grid().display_offset() as i32;
        Some((Point::new(Line(row - offset), Column(col as usize)), side))
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.pressed = true;
        if self.prompt.menu_open() {
            self.with_prompt(|prompt, env| {
                prompt.close_menu();
                prompt.render(env);
            });
        }
        let Some((point, side)) = self.point_at(event.position) else {
            return;
        };
        let kind = match event.click_count {
            1 => None,
            2 => Some(SelectionType::Semantic),
            _ => Some(SelectionType::Lines),
        };
        let mut term = self.terminal.term.lock();
        term.selection = kind.map(|kind| Selection::new(kind, point, side));
        drop(term);
        self.drag_start = kind.is_none().then_some((point, side));
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.mouse = Some(event.position);
        if self.update_hover() {
            cx.notify();
        }
        if event.pressed_button != Some(MouseButton::Left) || !self.pressed {
            return;
        }
        let Some((point, side)) = self.point_at(event.position) else {
            return;
        };
        let mut term = self.terminal.term.lock();
        if term.selection.is_none() {
            let Some((start, start_side)) = self.drag_start else {
                return;
            };
            term.selection = Some(Selection::new(SelectionType::Simple, start, start_side));
        }
        if let Some(selection) = term.selection.as_mut() {
            selection.update(point, side);
        }
        cx.notify();
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.pressed) {
            return;
        }
        self.drag_start = None;
        let has_selection = self.terminal.term.lock().selection.is_some();
        if has_selection {
            self.with_prompt(|prompt, env| prompt.terminal_selection_changed(env));
        } else if let Some((point, _)) = self.point_at(event.position) {
            self.with_prompt(|prompt, env| {
                prompt.terminal_selection_changed(env);
                prompt.click(point, env);
            });
        }
        cx.notify();
    }

    fn scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(g) = self.geometry else {
            return;
        };
        let delta = f32::from(event.delta.pixel_delta(g.line_height).y);
        let lines = delta / f32::from(g.line_height) + self.scroll_remainder;
        self.scroll_remainder = lines.fract();
        let lines = lines.trunc() as i32;
        if lines == 0 {
            return;
        }
        let mode = self.mode();
        if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            // Full screen programs like less scroll with the arrow keys.
            let key = if lines > 0 { "up" } else { "down" };
            let app_cursor = mode.contains(TermMode::APP_CURSOR);
            if let Some(bytes) = keys::to_bytes(key, None, Mods::default(), app_cursor) {
                self.terminal
                    .write(&bytes.repeat(lines.unsigned_abs() as usize));
            }
        } else {
            self.terminal
                .term
                .lock()
                .scroll_display(Scroll::Delta(lines));
        }
        self.update_hover();
        cx.notify();
    }

    fn hover_changed(&mut self, hovered: &bool, _window: &mut Window, cx: &mut Context<Self>) {
        if !hovered {
            self.mouse = None;
            self.update_hover();
            cx.notify();
        }
    }

    /// The grid's area in the window, and the bounds of one of its cells.
    fn cell_bounds(&self, cell: Point) -> Option<(Bounds<Pixels>, Bounds<Pixels>)> {
        let g = self.geometry?;
        let offset = self.terminal.term.lock().grid().display_offset() as i32;
        let area = Bounds::new(
            g.origin,
            size(
                g.cell_width * f32::from(g.grid.cols),
                g.line_height * f32::from(g.grid.rows),
            ),
        );
        let cell = Bounds::new(
            point(
                g.origin.x + g.cell_width * cell.column.0 as f32,
                g.origin.y + g.line_height * (cell.line.0 + offset) as f32,
            ),
            size(g.cell_width, g.line_height),
        );
        Some((area, cell))
    }

    fn render_popup(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let popup = self.prompt.popup(&self.terminal)?;
        let (area, anchor) = self.cell_bounds(popup.anchor)?;
        let font = self.font.family.clone();
        Some(popup::render(popup, anchor, area, self.palette, font, cx))
    }

    fn render_block_bar(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let block = self.hovered?;
        let g = self.geometry?;
        let offset = self.terminal.term.lock().grid().display_offset() as i32;
        let row = (block.start.0 + offset).max(0);
        let top = g.line_height * row as f32 + px(1.);
        Some(block_bar::render(
            top,
            block.end > block.head_end,
            self.palette,
            cx,
        ))
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toast = self.toast.as_ref().map(|(text, _)| {
            div()
                .absolute()
                .bottom(px(16.))
                .w_full()
                .flex()
                .justify_center()
                .child(block_bar::label(text.clone(), self.palette))
        });
        div()
            .id("terminal")
            .size_full()
            .p(px(PADDING))
            .bg(self.palette.background())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_scroll_wheel(cx.listener(Self::scroll))
            .on_hover(cx.listener(Self::hover_changed))
            .child(
                div()
                    .relative()
                    .size_full()
                    .child(TerminalElement { view: cx.entity() })
                    .children(self.render_block_bar(cx))
                    .children(toast),
            )
            .children(self.render_popup(cx))
    }
}

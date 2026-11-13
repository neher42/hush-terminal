use std::sync::Arc;

use alacritty_terminal::event::Event;
use alacritty_terminal::term::TermMode;
use futures::StreamExt;
use gpui::{div, prelude::*, px, Context, FocusHandle, Font, KeyDownEvent, Pixels, Window};
use hush_core::events::ShellEvent;
use hush_core::pty::{CommandAction, PtyManager};

use crate::colors::{Palette, LIGHT};
use crate::element::TerminalElement;
use crate::keys::{self, Mods};
use crate::terminal::{GridSize, Output, Terminal};

const PADDING: f32 = 8.0;

pub struct TerminalView {
    pub(crate) terminal: Terminal,
    focus: FocusHandle,
    pub(crate) font: Font,
    pub(crate) font_size: Pixels,
    pub(crate) palette: &'static Palette,
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
        let (terminal, mut output) = Terminal::spawn(pty, dirs::home_dir().as_deref(), initial)
            .expect("failed to start the shell");

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
            focus,
            font: gpui::font("Menlo"),
            font_size: px(13.5),
            palette: &LIGHT,
            pending_run: run,
        }
    }

    fn handle_output(&mut self, output: Output, cx: &mut Context<Self>) {
        match output {
            Output::Wakeup => {}
            Output::Shell(event, _cursor) => {
                if event == ShellEvent::CommandInputStart {
                    if let Some(command) = self.pending_run.take() {
                        self.terminal.submit(&command, "", CommandAction::Run);
                    }
                }
            }
            Output::Term(Event::PtyWrite(text)) => self.terminal.write(text.as_bytes()),
            Output::Term(_) => {}
            Output::Exit => cx.quit(),
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = &keystroke.modifiers;
        if m.platform {
            match keystroke.key.as_str() {
                "q" => cx.quit(),
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.paste(&text);
                    }
                }
                _ => {}
            }
            return;
        }
        let app_cursor = self
            .terminal
            .term
            .lock()
            .mode()
            .contains(TermMode::APP_CURSOR);
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
            self.terminal.write(&bytes);
            cx.stop_propagation();
        }
    }

    fn paste(&self, text: &str) {
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        let bracketed = self
            .terminal
            .term
            .lock()
            .mode()
            .contains(TermMode::BRACKETED_PASTE);
        let data = if bracketed {
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text
        };
        self.terminal.write(data.as_bytes());
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p(px(PADDING))
            .bg(self.palette.background())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .child(TerminalElement { view: cx.entity() })
    }
}

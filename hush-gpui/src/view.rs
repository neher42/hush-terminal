use alacritty_terminal::event::Event;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
use futures::StreamExt;
use gpui::{
    div, fill, point, prelude::*, px, relative, size, App, Bounds, Context, Element, ElementId, Entity, FocusHandle,
    Font, FontWeight, GlobalElementId, Hsla, InspectorElementId, KeyDownEvent, LayoutId, Pixels, ShapedLine, Style,
    TextRun, Window,
};

use crate::colors::{Palette, LIGHT};
use crate::keys::{self, Mods};
use crate::terminal::{GridSize, Terminal};

const PADDING: f32 = 8.0;

pub struct TerminalView {
    terminal: Terminal,
    focus: FocusHandle,
    font: Font,
    font_size: Pixels,
    palette: &'static Palette,
}

impl TerminalView {
    pub fn new(command: Option<(String, Vec<String>)>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial = GridSize { cols: 100, rows: 30, cell_width: 8, cell_height: 17 };
        let (terminal, mut events) = Terminal::spawn(command, initial).expect("failed to start the shell");

        // New output arrives in bursts; handle everything queued, then redraw once.
        cx.spawn(async move |this, cx| {
            while let Some(first) = events.next().await {
                let mut batch = vec![first];
                while let Ok(event) = events.try_recv() {
                    batch.push(event);
                }
                let alive = this.update(cx, |view, cx| {
                    for event in batch {
                        match event {
                            Event::PtyWrite(text) => view.terminal.write(text.into_bytes()),
                            Event::Exit | Event::ChildExit(_) => cx.quit(),
                            _ => {}
                        }
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
        Self { terminal, focus, font: gpui::font("Menlo"), font_size: px(13.5), palette: &LIGHT }
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
        let app_cursor = self.terminal.term.lock().mode().contains(TermMode::APP_CURSOR);
        let mods = Mods { ctrl: m.control, alt: m.alt, shift: m.shift };
        if let Some(bytes) = keys::to_bytes(&keystroke.key, keystroke.key_char.as_deref(), mods, app_cursor) {
            self.terminal.write(bytes);
            cx.stop_propagation();
        }
    }

    fn paste(&self, text: &str) {
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        let bracketed = self.terminal.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        let data = if bracketed { format!("\x1b[200~{text}\x1b[201~") } else { text };
        self.terminal.write(data.into_bytes());
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

/// Draws the terminal grid: one shaped line per row, plus cell backgrounds and the cursor.
struct TerminalElement {
    view: Entity<TerminalView>,
}

struct Frame {
    lines: Vec<ShapedLine>,
    backgrounds: Vec<(Bounds<Pixels>, Hsla)>,
    cursor: Option<Bounds<Pixels>>,
    line_height: Pixels,
}

impl IntoElement for TerminalElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = Frame;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Frame {
        let (font, font_size, palette) = {
            let view = self.view.read(cx);
            (view.font.clone(), view.font_size, view.palette)
        };
        let text_system = window.text_system().clone();
        let font_id = text_system.resolve_font(&font);
        let cell_width = text_system.advance(font_id, font_size, 'm').map(|s| s.width).unwrap_or(px(8.));
        let line_height = (font_size * 1.3).round();

        let cols = (bounds.size.width / cell_width).floor().max(2.0) as u16;
        let rows = (bounds.size.height / line_height).floor().max(1.0) as u16;
        let grid = GridSize {
            cols,
            rows,
            cell_width: f32::from(cell_width).round() as u16,
            cell_height: f32::from(line_height) as u16,
        };
        self.view.update(cx, |view, _| view.terminal.resize(grid));

        let bold = Font { weight: FontWeight::BOLD, ..font.clone() };
        let default_bg = Color::Named(NamedColor::Background);
        let term = self.view.read(cx).terminal.term.clone();
        let term = term.lock();
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let cursor_point = content.cursor.point;
        let cursor_shape = content.cursor.shape;

        let mut rows_text: Vec<(String, Vec<TextRun>)> = vec![(String::new(), Vec::new()); rows as usize];
        let mut backgrounds = Vec::new();
        for indexed in content.display_iter {
            let row = indexed.point.line.0 + offset;
            if row < 0 || row >= i32::from(rows) {
                continue;
            }
            let cell = indexed.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let (fg, bg) = if cell.flags.contains(Flags::INVERSE) { (cell.bg, cell.fg) } else { (cell.fg, cell.bg) };
            if bg != default_bg {
                let x = bounds.origin.x + cell_width * indexed.point.column.0 as f32;
                let y = bounds.origin.y + line_height * row as f32;
                let width = if cell.flags.contains(Flags::WIDE_CHAR) { cell_width * 2.0 } else { cell_width };
                backgrounds.push((Bounds::new(point(x, y), size(width, line_height)), palette.resolve(bg)));
            }
            // alacritty keeps tabs in the grid (for copying); they occupy a normal cell.
            let ch = if cell.flags.contains(Flags::HIDDEN) || cell.c == '\t' { ' ' } else { cell.c };
            let color = palette.resolve(fg);
            let is_bold = cell.flags.contains(Flags::BOLD);
            let (text, runs) = &mut rows_text[row as usize];
            text.push(ch);
            match runs.last_mut() {
                Some(run) if run.color == color && (run.font.weight == FontWeight::BOLD) == is_bold => {
                    run.len += ch.len_utf8();
                }
                _ => runs.push(TextRun {
                    len: ch.len_utf8(),
                    font: if is_bold { bold.clone() } else { font.clone() },
                    color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }),
            }
        }
        drop(term);

        let lines = rows_text
            .into_iter()
            .map(|(text, runs)| text_system.shape_line(text.into(), font_size, &runs, None))
            .collect();
        let cursor_row = cursor_point.line.0 + offset;
        let cursor = (cursor_shape != CursorShape::Hidden && (0..i32::from(rows)).contains(&cursor_row)).then(|| {
            Bounds::new(
                point(
                    bounds.origin.x + cell_width * cursor_point.column.0 as f32,
                    bounds.origin.y + line_height * cursor_row as f32,
                ),
                size(px(2.), line_height),
            )
        });
        Frame { lines, backgrounds, cursor, line_height }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        frame: &mut Frame,
        window: &mut Window,
        cx: &mut App,
    ) {
        for (cell, color) in frame.backgrounds.drain(..) {
            window.paint_quad(fill(cell, color));
        }
        for (row, line) in frame.lines.iter().enumerate() {
            let origin = point(bounds.origin.x, bounds.origin.y + frame.line_height * row as f32);
            let _ = line.paint(origin, frame.line_height, window, cx);
        }
        if let Some(cursor) = frame.cursor {
            let palette = self.view.read(cx).palette;
            window.paint_quad(fill(cursor, palette.cursor()));
        }
    }
}

use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
use gpui::{
    fill, point, px, relative, size, App, Bounds, Element, ElementId, Entity, Font, FontWeight,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, Pixels, ShapedLine, Style,
    TextRun, Window,
};

use crate::terminal::GridSize;
use crate::view::TerminalView;

/// Where the grid is drawn and how big its cells are.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub origin: gpui::Point<Pixels>,
    pub cell_width: Pixels,
    pub line_height: Pixels,
    pub grid: GridSize,
}

/// Draws the terminal grid: one shaped line per row, plus cell backgrounds and the cursor.
pub struct TerminalElement {
    pub view: Entity<TerminalView>,
}

pub struct Frame {
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
        let (font, font_size, palette, hovered) = {
            let view = self.view.read(cx);
            (
                view.font.clone(),
                view.font_size,
                view.palette,
                view.hovered,
            )
        };
        let text_system = window.text_system().clone();
        let font_id = text_system.resolve_font(&font);
        let cell_width = text_system
            .advance(font_id, font_size, 'm')
            .map(|s| s.width)
            .unwrap_or(px(8.));
        let line_height = (font_size * 1.3).round();

        let cols = (bounds.size.width / cell_width).floor().max(2.0) as u16;
        let rows = (bounds.size.height / line_height).floor().max(1.0) as u16;
        let grid = GridSize {
            cols,
            rows,
            cell_width: f32::from(cell_width).round() as u16,
            cell_height: f32::from(line_height) as u16,
        };
        let geometry = Geometry {
            origin: bounds.origin,
            cell_width,
            line_height,
            grid,
        };
        self.view
            .update(cx, |view, cx| view.set_geometry(geometry, cx));

        let bold = Font {
            weight: FontWeight::BOLD,
            ..font.clone()
        };
        let default_bg = Color::Named(NamedColor::Background);
        let term = self.view.read(cx).terminal.term.clone();
        let term = term.lock();
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let cursor_point = content.cursor.point;
        let cursor_shape = content.cursor.shape;
        let selection = content.selection;
        let selection_color = palette.selection();

        let mut rows_text: Vec<(String, Vec<TextRun>)> =
            vec![(String::new(), Vec::new()); rows as usize];
        let mut backgrounds = Vec::new();
        if let Some(block) = hovered {
            let first = (block.start.0 + offset).max(0);
            let last = (block.end.0 + offset).min(i32::from(rows) - 1);
            if first <= last {
                backgrounds.push((
                    Bounds::new(
                        point(
                            bounds.origin.x,
                            bounds.origin.y + line_height * first as f32,
                        ),
                        size(bounds.size.width, line_height * (last - first + 1) as f32),
                    ),
                    gpui::rgb(palette.hover).into(),
                ));
            }
        }
        let mut selections = Vec::new();
        for indexed in content.display_iter {
            let row = indexed.point.line.0 + offset;
            if row < 0 || row >= i32::from(rows) {
                continue;
            }
            let cell = indexed.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let (fg, bg) = if cell.flags.contains(Flags::INVERSE) {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };
            let x = bounds.origin.x + cell_width * indexed.point.column.0 as f32;
            let y = bounds.origin.y + line_height * row as f32;
            let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                cell_width * 2.0
            } else {
                cell_width
            };
            let cell_bounds = Bounds::new(point(x, y), size(width, line_height));
            if selection.is_some_and(|s| s.contains(indexed.point)) {
                selections.push((cell_bounds, selection_color));
            }
            if bg != default_bg {
                backgrounds.push((cell_bounds, palette.resolve(bg)));
            }
            // alacritty keeps tabs in the grid (for copying); they occupy a normal cell.
            let ch = if cell.flags.contains(Flags::HIDDEN) || cell.c == '\t' {
                ' '
            } else {
                cell.c
            };
            let color = palette.resolve(fg);
            let is_bold = cell.flags.contains(Flags::BOLD);
            let (text, runs) = &mut rows_text[row as usize];
            text.push(ch);
            match runs.last_mut() {
                Some(run)
                    if run.color == color && (run.font.weight == FontWeight::BOLD) == is_bold =>
                {
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
        backgrounds.extend(selections);

        let lines = rows_text
            .into_iter()
            .map(|(text, runs)| text_system.shape_line(text.into(), font_size, &runs, None))
            .collect();
        let cursor_row = cursor_point.line.0 + offset;
        let cursor = (cursor_shape != CursorShape::Hidden
            && (0..i32::from(rows)).contains(&cursor_row))
        .then(|| {
            Bounds::new(
                point(
                    bounds.origin.x + cell_width * cursor_point.column.0 as f32,
                    bounds.origin.y + line_height * cursor_row as f32,
                ),
                size(px(2.), line_height),
            )
        });
        Frame {
            lines,
            backgrounds,
            cursor,
            line_height,
        }
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
            let origin = point(
                bounds.origin.x,
                bounds.origin.y + frame.line_height * row as f32,
            );
            let _ = line.paint(origin, frame.line_height, window, cx);
        }
        if let Some(cursor) = frame.cursor {
            let palette = self.view.read(cx).palette;
            window.paint_quad(fill(cursor, palette.cursor()));
        }
    }
}

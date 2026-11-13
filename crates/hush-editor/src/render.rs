use crate::highlight::Span;
use crate::layout::{layout_text, Cell, Layout};

const ESC: &str = "\x1b[";
const GHOST_COLOR: u8 = 8;

fn sgr(color: u8) -> u8 {
    if color < 8 {
        30 + color
    } else {
        90 + color - 8
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderInput<'a> {
    pub text: &'a str,
    /// Byte offset of the cursor in `text`.
    pub cursor: usize,
    pub spans: &'a [Span],
    /// Suggested completion drawn dimmed after the text.
    pub ghost: &'a str,
    /// Column the command starts on, right after the prompt.
    pub start_col: usize,
    pub cols: usize,
    /// Where the terminal cursor is now, relative to the command start.
    pub from: Cell,
}

#[derive(Debug, Clone)]
pub struct RenderOutput {
    pub sequence: String,
    /// Where the terminal cursor ends up, relative to the command start.
    pub cursor: Cell,
    pub layout: Layout,
}

/// Escape sequence that redraws the command from its start and places the cursor.
pub fn render_command_line(input: &RenderInput) -> RenderOutput {
    let RenderInput {
        text,
        spans,
        ghost,
        start_col,
        cols,
        from,
        ..
    } = *input;
    let layout = layout_text(text, start_col, cols);
    let cursor = layout.cell_at(input.cursor);

    let mut out = erase_command_line(from, start_col);
    out += &styled(text, spans);
    if !ghost.is_empty() {
        out += &format!("{ESC}{}m{}{ESC}0m", sgr(GHOST_COLOR), sanitize(ghost));
    }

    // Text that exactly fills the last column leaves the cursor in pending-wrap state.
    let drawn = format!("{text}{ghost}");
    let end = layout_text(&drawn, start_col, cols).end;
    let mut at_row = end.row;
    if !drawn.is_empty() && end.col == 0 && !drawn.ends_with('\n') {
        if cursor.row == at_row {
            // The cursor belongs on a row that doesn't exist yet: create it.
            out += "\r\n";
        } else {
            at_row -= 1;
        }
    }

    if at_row > cursor.row {
        out += &format!("{ESC}{}A", at_row - cursor.row);
    }
    out += &format!("{ESC}{}G", cursor.col + 1);
    RenderOutput {
        sequence: out,
        cursor,
        layout,
    }
}

pub fn erase_command_line(from: Cell, start_col: usize) -> String {
    move_to_command_start(from, start_col) + ESC + "J"
}

pub fn move_to_command_start(from: Cell, start_col: usize) -> String {
    let up = if from.row > 0 {
        format!("{ESC}{}A", from.row)
    } else {
        String::new()
    };
    format!("{up}{ESC}{}G", start_col + 1)
}

fn styled(text: &str, spans: &[Span]) -> String {
    let mut out = String::new();
    let mut pos = 0;
    for span in spans {
        let start = span.start.max(pos);
        let end = span.end.max(start);
        out += &sanitize(&text[pos..start]);
        let chunk = sanitize(&text[start..end]);
        match span.style.color() {
            Some(color) => out += &format!("{ESC}{}m{chunk}{ESC}0m", sgr(color)),
            None => out += &chunk,
        }
        pos = end;
    }
    out + &sanitize(&text[pos..])
}

fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\t' => out.push(' '),
            '\n' => out.push_str("\r\n"),
            '\0'..='\x1f' | '\x7f' => out.push('␣'),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::Style;

    fn input(text: &str, cursor: usize, start_col: usize, cols: usize) -> RenderInput<'_> {
        RenderInput {
            text,
            cursor,
            spans: &[],
            ghost: "",
            start_col,
            cols,
            from: Cell {
                row: 0,
                col: start_col,
            },
        }
    }

    #[test]
    fn redraws_from_the_start_and_places_the_cursor() {
        let spans = [
            Span {
                start: 0,
                end: 2,
                style: Style::Command,
            },
            Span {
                start: 3,
                end: 6,
                style: Style::Flag,
            },
        ];
        let out = render_command_line(&RenderInput {
            spans: &spans,
            ..input("ls -la", 2, 10, 80)
        });
        assert_eq!(
            out.sequence,
            format!("{ESC}11G{ESC}J{ESC}34mls{ESC}0m {ESC}35m-la{ESC}0m{ESC}13G")
        );
        assert_eq!(out.cursor, Cell { row: 0, col: 12 });
    }

    #[test]
    fn moves_up_from_a_lower_row_before_redrawing() {
        let out = render_command_line(&RenderInput {
            from: Cell { row: 1, col: 1 },
            ..input("a\nb", 0, 2, 80)
        });
        assert!(out.sequence.starts_with(&format!("{ESC}1A{ESC}3G{ESC}J")));
        assert!(out.sequence.contains("a\r\nb"));
        assert!(out.sequence.ends_with(&format!("{ESC}1A{ESC}3G")));
    }

    #[test]
    fn draws_the_suggestion_dimmed_without_moving_the_cursor_past_the_text() {
        let out = render_command_line(&RenderInput {
            ghost: "eckout",
            ..input("git ch", 6, 0, 80)
        });
        assert!(out.sequence.contains(&format!("{ESC}90meckout{ESC}0m")));
        assert_eq!(out.cursor, Cell { row: 0, col: 6 });
        assert!(out.sequence.ends_with(&format!("{ESC}7G")));
    }

    #[test]
    fn creates_the_next_row_when_the_text_exactly_fills_the_last_column() {
        let out = render_command_line(&input("abc", 3, 2, 5));
        assert!(out.sequence.contains("abc\r\n"));
        assert_eq!(out.cursor, Cell { row: 1, col: 0 });
    }

    #[test]
    fn shows_control_characters_as_a_visible_placeholder() {
        let out = render_command_line(&input("a\x07b", 3, 0, 80));
        assert!(!out.sequence.contains('\x07'));
        assert!(out.sequence.contains("a␣b"));
    }

    #[test]
    fn places_the_cursor_by_display_columns_in_multibyte_text() {
        let text = "ä😀x";
        let spans = [Span {
            start: 0,
            end: 6,
            style: Style::String,
        }];
        let out = render_command_line(&RenderInput {
            spans: &spans,
            ..input(text, 6, 2, 80)
        });
        assert!(out.sequence.contains(&format!("{ESC}32mä😀{ESC}0mx")));
        assert_eq!(out.cursor, Cell { row: 0, col: 5 });
        assert!(out.sequence.ends_with(&format!("{ESC}6G")));
    }
}

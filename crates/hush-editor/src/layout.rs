/// A terminal cell relative to where the command starts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cell {
    /// Rows below the row the command starts on.
    pub row: usize,
    pub col: usize,
}

/// Where each char of the text lands in the terminal grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The byte offset and cell of each char, plus one entry for the end of the text.
    positions: Vec<(usize, Cell)>,
    /// Where the cursor goes after the last char. Never past the last column.
    pub end: Cell,
}

impl Layout {
    /// The cell of the char at `offset` (or containing it); the end for offsets past the text.
    pub fn cell_at(&self, offset: usize) -> Cell {
        let i = self.positions.partition_point(|&(o, _)| o <= offset);
        i.checked_sub(1).map_or(self.end, |i| self.positions[i].1)
    }

    /// The offset of the char under `cell`, or of the end of its row.
    pub fn offset_at(&self, cell: Cell) -> usize {
        self.positions
            .iter()
            .take_while(|(_, p)| *p <= cell)
            .last()
            .map_or(0, |&(offset, _)| offset)
    }
}

pub fn layout_text(text: &str, start_col: usize, cols: usize) -> Layout {
    let cols = cols.max(1);
    let mut positions = Vec::with_capacity(text.len() + 1);
    let mut row = 0;
    let mut col = start_col.min(cols - 1);

    for (i, c) in text.char_indices() {
        if c == '\n' {
            positions.push((i, Cell { row, col }));
            row += 1;
            col = 0;
            continue;
        }
        let width = display_width(c);
        if col + width > cols {
            row += 1;
            col = 0;
        }
        positions.push((i, Cell { row, col }));
        col += width;
    }
    if col >= cols {
        row += 1;
        col = 0;
    }
    let end = Cell { row, col };
    positions.push((text.len(), end));
    Layout { positions, end }
}

/// Terminal columns taken by `c`: 0 for combining marks, 2 for wide (CJK, emoji) chars.
pub fn display_width(c: char) -> usize {
    let cp = c as u32;
    if cp == 0 {
        0
    } else if cp < 0x300 {
        1
    } else if is_zero_width(cp) {
        0
    } else if is_wide(cp) {
        2
    } else {
        1
    }
}

fn is_zero_width(cp: u32) -> bool {
    matches!(
        cp,
        0x0300..=0x036f
            | 0x0483..=0x0489
            | 0x0591..=0x05bd
            | 0x0610..=0x061a
            | 0x064b..=0x065f
            | 0x200b..=0x200f
            | 0x20d0..=0x20ff
            | 0xfe00..=0xfe0f
            | 0xfe20..=0xfe2f
            | 0xe0100..=0xe01ef
    )
}

fn is_wide(cp: u32) -> bool {
    matches!(
        cp,
        0x1100..=0x115f
            | 0x231a..=0x231b
            | 0x23e9..=0x23ec
            | 0x25fd..=0x25fe
            | 0x2614..=0x2615
            | 0x2648..=0x2653
            | 0x26aa..=0x26ab
            | 0x26bd..=0x26be
            | 0x26c4..=0x26c5
            | 0x2705
            | 0x270a..=0x270b
            | 0x2728
            | 0x274c
            | 0x2753..=0x2755
            | 0x2795..=0x2797
            | 0x2e80..=0x303e
            | 0x3041..=0x33ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xa000..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe30..=0xfe4f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
            | 0x1f300..=0x1f64f
            | 0x1f680..=0x1f6ff
            | 0x1f900..=0x1f9ff
            | 0x1fa70..=0x1faff
            | 0x20000..=0x3fffd
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(row: usize, col: usize) -> Cell {
        Cell { row, col }
    }

    #[test]
    fn places_text_after_the_prompt_and_wraps_at_the_edge() {
        let layout = layout_text("abcdef", 2, 5);
        let cells: Vec<Cell> = (0..7).map(|i| layout.cell_at(i)).collect();
        assert_eq!(
            cells,
            [
                cell(0, 2),
                cell(0, 3),
                cell(0, 4),
                cell(1, 0),
                cell(1, 1),
                cell(1, 2),
                cell(1, 3),
            ]
        );
    }

    #[test]
    fn moves_the_end_to_the_next_row_when_the_text_fills_the_last_column() {
        assert_eq!(layout_text("abc", 2, 5).end, cell(1, 0));
    }

    #[test]
    fn starts_new_lines_at_column_zero() {
        let layout = layout_text("ab\ncd", 4, 10);
        assert_eq!(layout.cell_at(2), cell(0, 6));
        assert_eq!(layout.cell_at(3), cell(1, 0));
        assert_eq!(layout.end, cell(1, 2));
    }

    #[test]
    fn wraps_a_wide_character_that_does_not_fit() {
        let layout = layout_text("a漢", 0, 2);
        assert_eq!(layout.cell_at(1), cell(1, 0));
        assert_eq!(layout.end, cell(2, 0));
    }

    #[test]
    fn gives_every_byte_of_a_char_the_same_cell() {
        let layout = layout_text("😀x", 0, 10);
        assert_eq!(layout.cell_at(0), layout.cell_at(2));
        assert_eq!(layout.cell_at(4), cell(0, 2));
    }

    #[test]
    fn finds_the_character_under_a_cell_or_the_end_of_the_row() {
        let layout = layout_text("ab\ncdef", 2, 20);
        assert_eq!(layout.offset_at(cell(0, 3)), 1);
        assert_eq!(layout.offset_at(cell(0, 15)), 2);
        assert_eq!(layout.offset_at(cell(1, 2)), 5);
        assert_eq!(layout.offset_at(cell(5, 0)), 7);
    }

    #[test]
    fn finds_offsets_on_char_boundaries_in_multibyte_text() {
        let layout = layout_text("ä😀x", 0, 10);
        assert_eq!(layout.offset_at(cell(0, 1)), 2);
        assert_eq!(layout.offset_at(cell(0, 2)), 2);
        assert_eq!(layout.offset_at(cell(0, 3)), 6);
        assert_eq!(layout.end, cell(0, 4));
    }
}

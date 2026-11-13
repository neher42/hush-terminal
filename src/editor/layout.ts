export interface Cell {
  /** Rows below the row the command starts on. */
  row: number;
  col: number;
}

export interface Layout {
  /** The cell for each UTF-16 offset, plus one for the end of the text. */
  positions: Cell[];
  /** Where the cursor goes after the last character. Never past the last column. */
  end: Cell;
}

export function layoutText(text: string, startCol: number, cols: number): Layout {
  const positions: Cell[] = new Array<Cell>(text.length + 1);
  let row = 0;
  let col = Math.min(startCol, cols - 1);
  let i = 0;

  while (i < text.length) {
    const cp = text.codePointAt(i)!;
    const length = cp > 0xffff ? 2 : 1;
    if (cp === 0x0a) {
      positions[i] = { row, col };
      row++;
      col = 0;
    } else {
      const width = displayWidth(cp);
      if (col + width > cols) {
        row++;
        col = 0;
      }
      positions[i] = { row, col };
      col += width;
    }
    if (length === 2) positions[i + 1] = positions[i]!;
    i += length;
  }
  if (col >= cols) {
    row++;
    col = 0;
  }
  const end = { row, col };
  positions[text.length] = end;
  return { positions, end };
}

export function offsetAt(layout: Layout, cell: Cell): number {
  const { positions } = layout;
  let best = 0;
  for (let i = 0; i < positions.length; i++) {
    const p = positions[i]!;
    if (p.row < cell.row || (p.row === cell.row && p.col <= cell.col)) best = i;
    else break;
  }
  return best;
}

export function displayWidth(cp: number): 0 | 1 | 2 {
  if (cp === 0) return 0;
  if (cp < 0x300) return 1;
  if (isZeroWidth(cp)) return 0;
  return isWide(cp) ? 2 : 1;
}

function isZeroWidth(cp: number): boolean {
  return (
    (cp >= 0x0300 && cp <= 0x036f) ||
    (cp >= 0x0483 && cp <= 0x0489) ||
    (cp >= 0x0591 && cp <= 0x05bd) ||
    (cp >= 0x0610 && cp <= 0x061a) ||
    (cp >= 0x064b && cp <= 0x065f) ||
    (cp >= 0x200b && cp <= 0x200f) ||
    (cp >= 0x20d0 && cp <= 0x20ff) ||
    (cp >= 0xfe00 && cp <= 0xfe0f) ||
    (cp >= 0xfe20 && cp <= 0xfe2f) ||
    (cp >= 0xe0100 && cp <= 0xe01ef)
  );
}

function isWide(cp: number): boolean {
  return (
    (cp >= 0x1100 && cp <= 0x115f) ||
    (cp >= 0x231a && cp <= 0x231b) ||
    (cp >= 0x23e9 && cp <= 0x23ec) ||
    (cp >= 0x25fd && cp <= 0x25fe) ||
    (cp >= 0x2614 && cp <= 0x2615) ||
    (cp >= 0x2648 && cp <= 0x2653) ||
    (cp >= 0x26aa && cp <= 0x26ab) ||
    (cp >= 0x26bd && cp <= 0x26be) ||
    (cp >= 0x26c4 && cp <= 0x26c5) ||
    (cp >= 0x2705 && cp <= 0x2705) ||
    (cp >= 0x270a && cp <= 0x270b) ||
    cp === 0x2728 ||
    cp === 0x274c ||
    (cp >= 0x2753 && cp <= 0x2755) ||
    (cp >= 0x2795 && cp <= 0x2797) ||
    (cp >= 0x2e80 && cp <= 0x303e) ||
    (cp >= 0x3041 && cp <= 0x33ff) ||
    (cp >= 0x3400 && cp <= 0x4dbf) ||
    (cp >= 0x4e00 && cp <= 0x9fff) ||
    (cp >= 0xa000 && cp <= 0xa4cf) ||
    (cp >= 0xac00 && cp <= 0xd7a3) ||
    (cp >= 0xf900 && cp <= 0xfaff) ||
    (cp >= 0xfe30 && cp <= 0xfe4f) ||
    (cp >= 0xff00 && cp <= 0xff60) ||
    (cp >= 0xffe0 && cp <= 0xffe6) ||
    (cp >= 0x1f300 && cp <= 0x1f64f) ||
    (cp >= 0x1f680 && cp <= 0x1f6ff) ||
    (cp >= 0x1f900 && cp <= 0x1f9ff) ||
    (cp >= 0x1fa70 && cp <= 0x1faff) ||
    (cp >= 0x20000 && cp <= 0x3fffd)
  );
}

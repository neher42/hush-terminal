import type { Span, Style } from './highlight';
import { type Cell, type Layout, layoutText } from './layout';

const ESC = '\x1b[';

// ANSI palette indexes (8 = bright black), so colors follow the theme and zsh can match them.
const COLOR: Record<Style, number | null> = {
  plain: null,
  command: 4,
  unknownCommand: 1,
  flag: 5,
  string: 2,
  variable: 3,
  operator: 8,
  comment: 8,
};
const sgrFor = (color: number): string => String(color < 8 ? 30 + color : 90 + color - 8);
const GHOST_SGR = sgrFor(8);

// zsh region_highlight offsets count code points, not UTF-16 units.
export function zshHighlights(text: string, spans: Span[]): string {
  const toChars = (offset: number): number => [...text.slice(0, offset)].length;
  return spans
    .filter((span) => COLOR[span.style] !== null)
    .map((span) => `${toChars(span.start)} ${toChars(span.end)} fg=${COLOR[span.style]}`)
    .join('\n');
}

export interface RenderInput {
  text: string;
  cursor: number;
  spans: Span[];
  ghost: string;
  startCol: number;
  cols: number;
  from: Cell;
}

export interface RenderOutput {
  sequence: string;
  cursor: Cell;
  layout: Layout;
}

export function renderCommandLine(input: RenderInput): RenderOutput {
  const { text, spans, ghost, startCol, cols, from } = input;
  const layout = layoutText(text, startCol, cols);
  const cursor = layout.positions[input.cursor] ?? layout.end;

  let out = moveToStart(from, startCol) + `${ESC}J`;
  out += styled(text, spans);
  if (ghost) out += `${ESC}${GHOST_SGR}m${sanitize(ghost)}${ESC}0m`;

  // Text that exactly fills the last column leaves the cursor in pending-wrap state.
  const drawn = layoutText(text + ghost, startCol, cols);
  let at = drawn.end;
  const fillsRow = (text + ghost).length > 0 && at.col === 0 && !(text + ghost).endsWith('\n');
  if (fillsRow) {
    if (cursor.row === at.row) {
      // The cursor belongs on a row that doesn't exist yet: create it.
      out += '\r\n';
    } else {
      at = { row: at.row - 1, col: cols };
    }
  }

  if (at.row > cursor.row) out += `${ESC}${at.row - cursor.row}A`;
  out += `${ESC}${cursor.col + 1}G`;
  return { sequence: out, cursor, layout };
}

export function eraseCommandLine(from: Cell, startCol: number): string {
  return moveToStart(from, startCol) + `${ESC}J`;
}

export function moveToCommandStart(from: Cell, startCol: number): string {
  return moveToStart(from, startCol);
}

function moveToStart(from: Cell, startCol: number): string {
  return (from.row > 0 ? `${ESC}${from.row}A` : '') + `${ESC}${startCol + 1}G`;
}

function styled(text: string, spans: Span[]): string {
  let out = '';
  let pos = 0;
  for (const span of spans) {
    if (span.start > pos) out += sanitize(text.slice(pos, span.start));
    const color = COLOR[span.style];
    const chunk = sanitize(text.slice(span.start, span.end));
    out += color === null ? chunk : `${ESC}${sgrFor(color)}m${chunk}${ESC}0m`;
    pos = span.end;
  }
  return out + sanitize(text.slice(pos));
}

function sanitize(text: string): string {
  return text.replace(/\t/g, ' ').replace(/[\x00-\x09\x0b-\x1f\x7f]/g, '␣').replace(/\n/g, '\r\n');
}

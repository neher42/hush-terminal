export type Unit = 'char' | 'word' | 'line' | 'all';
export type Direction = -1 | 1;
type EditKind = 'type' | 'space' | 'other';

interface Snapshot {
  text: string;
  cursor: number;
}

export interface Range {
  start: number;
  end: number;
}

const MAX_UNDO = 500;
const isWordChar = (ch: string | undefined): boolean => ch !== undefined && /[\p{L}\p{N}_]/u.test(ch);
const isSpace = (ch: string | undefined): boolean => ch !== undefined && /\s/.test(ch);

export class LineEditor {
  private _text = '';
  private _cursor = 0;
  private _anchor: number | null = null;
  private undoStack: Snapshot[] = [];
  private redoStack: Snapshot[] = [];
  private lastEdit: EditKind | null = null;

  get text(): string {
    return this._text;
  }

  get cursor(): number {
    return this._cursor;
  }

  get selection(): Range | null {
    if (this._anchor === null || this._anchor === this._cursor) return null;
    return { start: Math.min(this._anchor, this._cursor), end: Math.max(this._anchor, this._cursor) };
  }

  get isEmpty(): boolean {
    return this._text.length === 0;
  }

  get isMultiline(): boolean {
    return this._text.includes('\n');
  }

  get cursorAtEnd(): boolean {
    return this._cursor === this._text.length;
  }

  insert(input: string): void {
    if (!input) return;
    const kind: EditKind = input.length > 1 ? 'other' : isSpace(input) ? 'space' : 'type';
    const { start, end } = this.selection ?? { start: this._cursor, end: this._cursor };
    this.edit(this._text.slice(0, start) + input + this._text.slice(end), start + input.length, kind);
  }

  delete(unit: Unit, direction: Direction): boolean {
    const sel = this.selection;
    if (sel) return this.deleteRange(sel.start, sel.end);
    const target = this.boundary(unit, direction);
    return direction < 0 ? this.deleteRange(target, this._cursor) : this.deleteRange(this._cursor, target);
  }

  deleteShellWord(): boolean {
    const sel = this.selection;
    if (sel) return this.deleteRange(sel.start, sel.end);
    let i = this._cursor;
    while (i > 0 && isSpace(this._text[i - 1])) i--;
    while (i > 0 && !isSpace(this._text[i - 1])) i--;
    return this.deleteRange(i, this._cursor);
  }

  deleteRange(start: number, end: number): boolean {
    const [from, to] = [this.clamp(Math.min(start, end)), this.clamp(Math.max(start, end))];
    if (from === to) return false;
    this.edit(this._text.slice(0, from) + this._text.slice(to), from, 'other');
    return true;
  }

  // Without `extend`, an existing selection collapses to its edge (macOS behavior).
  move(unit: Unit, direction: Direction, extend = false): void {
    const sel = this.selection;
    if (!extend && sel && unit === 'char') {
      this.moveTo(direction < 0 ? sel.start : sel.end);
      return;
    }
    this.moveTo(this.boundary(unit, direction), extend);
  }

  moveTo(offset: number, extend = false): void {
    if (extend) {
      this._anchor ??= this._cursor;
    } else {
      this._anchor = null;
    }
    this._cursor = this.clamp(offset);
    this.lastEdit = null;
  }

  // Returns false on the first/last line so the caller can browse history instead.
  moveVertical(direction: Direction, extend = false): boolean {
    const start = this.lineStart(this._cursor);
    const end = this.lineEnd(this._cursor);
    if ((direction < 0 && start === 0) || (direction > 0 && end === this._text.length)) return false;
    const column = this._cursor - start;
    const targetStart = direction < 0 ? this.lineStart(start - 1) : end + 1;
    this.moveTo(Math.min(targetStart + column, this.lineEnd(targetStart)), extend);
    return true;
  }

  select(anchor: number, cursor: number): void {
    this._anchor = this.clamp(anchor);
    this._cursor = this.clamp(cursor);
    this.lastEdit = null;
  }

  selectAll(): void {
    this.select(0, this._text.length);
  }

  clearSelection(): void {
    this._anchor = null;
  }

  setText(text: string, cursor = text.length): void {
    if (text === this._text) {
      this.moveTo(cursor);
      return;
    }
    this.edit(text, cursor, 'other');
  }

  reset(text = ''): void {
    this._text = text;
    this._cursor = text.length;
    this._anchor = null;
    this.undoStack = [];
    this.redoStack = [];
    this.lastEdit = null;
  }

  undo(): boolean {
    const snapshot = this.undoStack.pop();
    if (!snapshot) return false;
    this.redoStack.push({ text: this._text, cursor: this._cursor });
    this.restore(snapshot);
    return true;
  }

  redo(): boolean {
    const snapshot = this.redoStack.pop();
    if (!snapshot) return false;
    this.undoStack.push({ text: this._text, cursor: this._cursor });
    this.restore(snapshot);
    return true;
  }

  boundary(unit: Unit, direction: Direction): number {
    const i = this._cursor;
    switch (unit) {
      case 'char':
        return this.clamp(i + direction * this.charLength(i, direction));
      case 'word':
        return direction < 0 ? this.wordStart(i) : this.wordEnd(i);
      case 'line':
        return direction < 0 ? this.lineStart(i) : this.lineEnd(i);
      case 'all':
        return direction < 0 ? 0 : this._text.length;
    }
  }

  lineStart(offset: number): number {
    return this._text.lastIndexOf('\n', offset - 1) + 1;
  }

  lineEnd(offset: number): number {
    const i = this._text.indexOf('\n', offset);
    return i < 0 ? this._text.length : i;
  }

  private wordStart(i: number): number {
    while (i > 0 && !isWordChar(this._text[i - 1])) i--;
    while (i > 0 && isWordChar(this._text[i - 1])) i--;
    return i;
  }

  private wordEnd(i: number): number {
    const n = this._text.length;
    while (i < n && !isWordChar(this._text[i])) i++;
    while (i < n && isWordChar(this._text[i])) i++;
    return i;
  }

  /** Steps over surrogate pairs so the cursor never splits an emoji. */
  private charLength(i: number, direction: Direction): number {
    const code = direction < 0 ? this._text.charCodeAt(i - 1) : this._text.charCodeAt(i);
    const isLow = code >= 0xdc00 && code <= 0xdfff;
    const isHigh = code >= 0xd800 && code <= 0xdbff;
    return (direction < 0 && isLow) || (direction > 0 && isHigh) ? 2 : 1;
  }

  private edit(text: string, cursor: number, kind: EditKind): void {
    // Consecutive typed characters undo together, like in a text editor.
    const coalesce = kind === 'type' && this.lastEdit === 'type';
    if (!coalesce) {
      this.undoStack.push({ text: this._text, cursor: this._cursor });
      if (this.undoStack.length > MAX_UNDO) this.undoStack.shift();
    }
    this.redoStack = [];
    this._text = text;
    this._cursor = this.clamp(cursor);
    this._anchor = null;
    this.lastEdit = kind;
  }

  private restore(snapshot: Snapshot): void {
    this._text = snapshot.text;
    this._cursor = this.clamp(snapshot.cursor);
    this._anchor = null;
    this.lastEdit = null;
  }

  private clamp(offset: number): number {
    return Math.max(0, Math.min(this._text.length, offset));
  }
}

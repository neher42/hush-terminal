import type { IMarker, Terminal } from '@xterm/xterm';
import { type CompletionItem, type CompletionKind, type CompletionSources, commonPrefix, complete } from '../completion/complete';
import type { CompletionMenu, MenuItem } from '../completion/CompletionMenu';
import { completionContext } from '../completion/context';
import { HistoryBrowser, type HistoryStore } from '../history/HistoryStore';
import { fitsHere, type Place } from '../history/suggestions';
import { Emitter } from '../lib/emitter';
import type { CommandCatalog } from '../shell/CommandCatalog';
import type { ShellEvent } from '../shell/events';
import type { TerminalView } from '../terminal/TerminalView';
import type { IconName } from '../ui/icons';
import { highlight } from './highlight';
import { isIncomplete } from './incomplete';
import { type EditorCommand, resolveKey } from './keymap';
import { type Cell, type Layout, offsetAt } from './layout';
import { LineEditor } from './LineEditor';
import { eraseCommandLine, moveToCommandStart, renderCommandLine, zshHighlights } from './render';

export interface PromptEditorDeps {
  view: TerminalView;
  history: HistoryStore;
  catalog: CommandCatalog;
  menu: CompletionMenu;
  completionSources(): CompletionSources;
  place(): Place;
  submit(command: string, highlights: string, action: 'run' | 'cancel'): void;
  writeToShell(data: string): void;
  copyText(text: string): void;
}

type MenuState =
  | { kind: 'complete'; wordStart: number; items: CompletionItem[] }
  | { kind: 'history'; items: string[] };

const ICON_FOR: Record<CompletionKind, IconName> = {
  command: 'prompt',
  subcommand: 'prompt',
  folder: 'folder',
  file: 'file',
  branch: 'branch',
  flag: 'flag',
};

const REDRAW_FALLBACK_MS = 250;
const NOTICE_MS = 1600;

export class PromptEditor {
  readonly onRun = new Emitter<string>();

  private readonly term: Terminal;
  private readonly editor = new LineEditor();
  private active = false;
  private start: { marker: IMarker; col: number } | null = null;
  // Terminal cursor position relative to the start of the command.
  private at: Cell = { row: 0, col: 0 };
  private drawn = false;
  private layout: Layout | null = null;
  private ghost = '';
  private browser: HistoryBrowser | null = null;
  private menuState: MenuState | null = null;
  private completionToken = 0;
  private syncingSelection = false;
  private ownsSelection = false;
  private swallowKeypress = false;
  // Restored at the next prompt after runCommand() replaced what was typed.
  private stash: string | null = null;
  private redrawTimer = 0;
  private noticeTimer = 0;

  constructor(private readonly deps: PromptEditorDeps) {
    this.term = deps.view.term;
    this.term.attachCustomKeyEventHandler((e) => this.onKey(e));
    this.term.onSelectionChange(() => this.onTerminalSelection());
    deps.view.container.addEventListener('mouseup', (e) => this.onMouseUp(e));
    deps.menu.onPick = (index) => this.acceptMenuItem(index);
  }

  get isActive(): boolean {
    return this.active;
  }

  handleShellEvent(event: ShellEvent): void {
    if (event.type === 'commandInputStart') this.beginInput();
    else if (event.type === 'commandStart') this.deactivate();
  }

  handleData(data: string): boolean {
    if (!this.active) return false;
    this.hideNotice();
    const isPaste = data.startsWith('\x1b[200~');
    let text = data.replace(/\x1b\[20[01]~/g, '');
    // Unhandled special keys (F1 etc.) arrive as escape sequences.
    if (!isPaste && text.startsWith('\x1b')) return true;
    text = text.replace(/\r\n?/g, '\n');
    text = text.replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '');
    if (isPaste) text = text.replace(/\n+$/, '');
    if (!text) return true;
    this.clearForeignSelection();
    this.editor.insert(text);
    this.afterEdit();
    return true;
  }

  runCommand(command: string): boolean {
    if (!this.active) return false;
    if (this.editor.text.trim()) this.stash = this.editor.text;
    this.finish(command, 'run');
    return true;
  }

  insertText(text: string): boolean {
    if (!this.active) return false;
    const before = this.editor.text[this.editor.cursor - 1];
    const lead = before !== undefined && !/\s/.test(before) ? ' ' : '';
    this.editor.insert(lead + text);
    this.afterEdit();
    return true;
  }

  // First ⌘A selects the command, the second one the whole buffer (returns false).
  selectAll(): boolean {
    const e = this.editor;
    if (!this.active || e.isEmpty || (e.selection?.start === 0 && e.selection.end === e.text.length)) {
      if (e.selection) {
        e.clearSelection();
        this.render();
      }
      this.ownsSelection = false;
      return false;
    }
    e.selectAll();
    this.render();
    return true;
  }

  cut(): void {
    const sel = this.editor.selection;
    if (!this.active || !sel) return;
    this.deps.copyText(this.editor.text.slice(sel.start, sel.end));
    this.editor.delete('char', -1);
    this.afterEdit();
  }

  clearScreen(): void {
    if (this.active) {
      void this.erase();
      this.term.clear();
      // ^L makes zsh redraw the prompt, which re-renders the command.
      this.deps.writeToShell('\x0c');
    } else {
      this.term.clear();
    }
  }

  // Put the cursor back where zsh expects it so its SIGWINCH redraw lines up.
  afterResize(): void {
    if (!this.active || !this.start) return;
    window.clearTimeout(this.redrawTimer);
    // Wait for queued writes so the cursor position is current.
    this.term.write('', () => {
      if (!this.active || !this.start || this.start.marker.isDisposed) return;
      const buffer = this.term.buffer.active;
      const row = buffer.baseY + buffer.cursorY - this.start.marker.line;
      if (row < 0) return;
      this.term.write(moveToCommandStart({ row, col: buffer.cursorX }, this.start.col));
      this.at = { row: 0, col: this.start.col };
      // Fallback in case zsh doesn't redraw.
      this.redrawTimer = window.setTimeout(() => this.render(), REDRAW_FALLBACK_MS);
    });
  }

  deactivate(): void {
    this.active = false;
    this.drawn = false;
    this.closeMenu();
    window.clearTimeout(this.redrawTimer);
  }

  private beginInput(): void {
    if (this.term.buffer.active.type === 'alternate') return;
    const marker = this.term.registerMarker(0);
    if (!marker) return;
    this.start?.marker.dispose();
    this.start = { marker, col: this.term.buffer.active.cursorX };
    this.at = { row: 0, col: this.start.col };
    this.drawn = false;
    window.clearTimeout(this.redrawTimer);

    // A redrawn prompt (resize, ^L) keeps the current command.
    if (!this.active) {
      this.editor.reset(this.stash ?? '');
      this.stash = null;
      this.browser = null;
    }
    this.active = true;
    this.render();
  }

  // Leaves the command on screen; zsh redraws it with the same colors (no flicker).
  private finish(command: string, action: 'run' | 'cancel'): void {
    this.closeMenu();
    this.releaseSelection();
    const spans = highlight(command, (name) => this.deps.catalog.isKnown(name));
    if (this.start && (this.drawn || command)) {
      const out = renderCommandLine({ text: command, cursor: 0, spans, ghost: '', startCol: this.start.col, cols: this.term.cols, from: this.at });
      this.term.write(out.sequence);
      this.at = out.cursor;
    }
    this.drawn = false;
    this.ghost = '';
    this.active = false;
    if (action === 'run') {
      this.deps.history.add(command);
      this.onRun.emit(command);
    }
    this.deps.submit(command, zshHighlights(command, spans), action);
    this.editor.reset();
  }

  private onKey(e: KeyboardEvent): boolean {
    if (e.type === 'keypress') return !this.swallowKeypress;
    if (e.type !== 'keydown') return true;
    this.swallowKeypress = false;
    const editing = this.active && !e.isComposing && e.keyCode !== 229;
    if (editing) {
      this.hideNotice();
      const command = resolveKey(e, this.menuState !== null);
      if (command) {
        e.preventDefault();
        this.swallowKeypress = true;
        this.execute(command);
        this.term.scrollToBottom();
        return false;
      }
    }
    // Let ⌘ shortcuts reach the app menu. xterm would otherwise eat some (⌥⌘→).
    return !e.metaKey;
  }

  private execute(command: EditorCommand): void {
    const e = this.editor;
    if (command.type !== 'vertical') this.browser = null;

    switch (command.type) {
      case 'submit':
        if (isIncomplete(e.text)) {
          e.insert('\n');
          return this.afterEdit();
        }
        return this.finish(e.text, 'run');
      case 'newline':
        e.insert('\n');
        return this.afterEdit();
      case 'cancel':
        return this.finish(e.text, 'cancel');
      case 'complete':
        void this.startCompletion();
        return;
      case 'historySearch':
        this.menuState = { kind: 'history', items: [] };
        this.refreshMenu();
        return this.render();
      case 'clearScreen':
        return this.clearScreen();
      case 'escape':
        if (this.term.hasSelection()) {
          this.term.clearSelection();
          e.clearSelection();
        }
        return this.render();
      case 'undo':
        e.undo();
        return this.afterEdit();
      case 'redo':
        e.redo();
        return this.afterEdit();
      case 'move':
        if (command.direction > 0 && !command.extend && e.cursorAtEnd && this.ghost) {
          return this.acceptGhost(command.unit === 'word' ? 'word' : 'all');
        }
        this.clearForeignSelection();
        e.move(command.unit, command.direction, command.extend);
        return this.render();
      case 'vertical':
        return this.moveVertical(command.direction, command.extend);
      case 'delete':
        this.clearForeignSelection();
        e.delete(command.unit, command.direction);
        return this.afterEdit();
      case 'deleteShellWord':
        e.deleteShellWord();
        return this.afterEdit();
      case 'menu':
        return this.menuAction(command.action);
    }
  }

  private moveVertical(direction: 1 | -1, extend: boolean): void {
    const e = this.editor;
    if (e.moveVertical(direction, extend)) return this.render();
    if (extend) {
      e.move('all', direction, true);
      return this.render();
    }
    this.browser ??= new HistoryBrowser(this.deps.history, e.text);
    const entry = direction < 0 ? this.browser.older() : this.browser.newer();
    if (entry === undefined) return;
    e.setText(entry);
    this.render();
  }

  private acceptGhost(unit: 'word' | 'all'): void {
    const piece = unit === 'word' ? (/^\s*[^\s/]*\/?/.exec(this.ghost)?.[0] || this.ghost) : this.ghost;
    this.editor.insert(piece);
    this.afterEdit();
  }

  private afterEdit(): void {
    this.browser = null;
    if (this.menuState) this.refreshMenu();
    this.render();
    this.term.scrollToBottom();
  }

  private onMouseUp(e: MouseEvent): void {
    if (!this.active || !this.start || !this.layout || e.button !== 0 || e.metaKey || e.altKey) return;
    // Run after xterm's own mouseup handling.
    window.setTimeout(() => {
      if (!this.start || !this.layout || this.term.hasSelection()) return;
      const cell = this.deps.view.cellAt(e.clientX, e.clientY);
      const startLine = this.start.marker.line;
      if (!cell || cell.line < startLine) return;
      this.editor.moveTo(offsetAt(this.layout, { row: cell.line - startLine, col: cell.col }));
      this.browser = null;
      this.render();
    });
  }

  private render(): void {
    if (!this.active || !this.start) return;
    const e = this.editor;
    const ghost = this.computeGhost();
    // Don't touch the line (e.g. RPROMPT) until there's something to draw.
    if (!this.drawn && e.isEmpty && !ghost) {
      this.layout = renderCommandLine({ text: '', cursor: 0, spans: [], ghost: '', startCol: this.start.col, cols: this.term.cols, from: this.at }).layout;
      this.positionMenu();
      return;
    }
    const out = renderCommandLine({
      text: e.text,
      cursor: e.cursor,
      spans: highlight(e.text, (name) => this.deps.catalog.isKnown(name), e.cursor),
      ghost,
      startCol: this.start.col,
      cols: this.term.cols,
      from: this.at,
    });
    this.at = out.cursor;
    this.layout = out.layout;
    this.ghost = ghost;
    this.drawn = true;
    this.term.write(out.sequence, () => {
      this.syncSelectionToTerminal();
      this.positionMenu();
    });
  }

  private erase(): Promise<void> {
    if (!this.start || !this.drawn) return Promise.resolve();
    const sequence = eraseCommandLine(this.at, this.start.col);
    this.at = { row: 0, col: this.start.col };
    this.drawn = false;
    this.ghost = '';
    return new Promise((resolve) => this.term.write(sequence, resolve));
  }

  private computeGhost(): string {
    const e = this.editor;
    if (e.selection || this.menuState || !e.cursorAtEnd || this.term.hasSelection()) return '';
    const place = this.deps.place();
    const suggestion = this.deps.history.suggest(e.text, (entry) => fitsHere(entry, place));
    return suggestion ? suggestion.slice(e.text.length) : '';
  }

  refreshSuggestion(): void {
    if (this.active && this.computeGhost() !== this.ghost) this.render();
  }

  // Mirror the editor selection as the terminal selection so ⌘C works on it.
  private syncSelectionToTerminal(): void {
    const sel = this.editor.selection;
    if (sel && this.layout && this.start) {
      const a = this.layout.positions[sel.start]!;
      const b = this.layout.positions[sel.end]!;
      const line = this.start.marker.line + a.row;
      const current = this.term.getSelectionPosition();
      const endLine = this.start.marker.line + b.row;
      if (current && current.start.x === a.col && current.start.y === line && current.end.x === b.col && current.end.y === endLine) return;
      this.syncingSelection = true;
      this.term.select(a.col, line, (b.row - a.row) * this.term.cols + (b.col - a.col));
      this.syncingSelection = false;
      this.ownsSelection = true;
    } else {
      this.releaseSelection();
    }
  }

  private releaseSelection(): void {
    if (!this.ownsSelection) return;
    this.syncingSelection = true;
    this.term.clearSelection();
    this.syncingSelection = false;
    this.ownsSelection = false;
  }

  private clearForeignSelection(): void {
    if (this.term.hasSelection() && !this.ownsSelection) this.term.clearSelection();
  }

  private onTerminalSelection(): void {
    if (this.syncingSelection || !this.active || !this.start || !this.layout) return;
    this.ownsSelection = false;
    const pos = this.term.getSelectionPosition();
    const startLine = this.start.marker.line;
    const layout = this.layout;
    const toOffset = (x: number, y: number): number | null => (y < startLine ? null : offsetAt(layout, { row: y - startLine, col: x }));
    const a = pos ? toOffset(pos.start.x, pos.start.y) : null;
    const b = pos ? toOffset(pos.end.x, pos.end.y) : null;
    if (a !== null && b !== null && a !== b) {
      this.editor.select(a, b);
      this.ownsSelection = true;
    } else {
      this.editor.clearSelection();
    }
    // Hide the suggestion while something is selected.
    if (this.ghost || (!pos && this.drawn)) this.render();
  }

  private async startCompletion(): Promise<void> {
    const e = this.editor;
    const ctx = completionContext(e.text, e.cursor);
    const token = ++this.completionToken;
    const items = await complete(ctx, this.deps.completionSources());
    if (token !== this.completionToken || !this.active) return;
    if (items.length === 0) {
      this.showNotice(ctx.word ? `No matches for '${ctx.word}'` : 'Nothing to complete here', ctx.wordStart);
      return;
    }
    if (items.length === 1) {
      this.applyCompletion(items[0]!, ctx.wordStart);
      return;
    }
    const prefix = commonPrefix(items);
    if (prefix.length > ctx.rawWord.length) {
      e.select(ctx.wordStart, e.cursor);
      e.insert(prefix);
    }
    this.menuState = { kind: 'complete', wordStart: ctx.wordStart, items };
    this.render();
    this.showMenu();
  }

  private refreshMenu(): void {
    const state = this.menuState;
    if (!state) return;
    const e = this.editor;
    if (state.kind === 'history') {
      state.items = this.deps.history.search(e.text);
      this.showMenu();
      return;
    }
    const typed = e.text.slice(state.wordStart, e.cursor);
    if (e.cursor < state.wordStart || /\s/.test(typed.replace(/\\\s/g, ''))) {
      this.closeMenu();
      return;
    }
    const ctx = completionContext(e.text, e.cursor);
    const token = ++this.completionToken;
    void complete(ctx, this.deps.completionSources()).then((items) => {
      if (token !== this.completionToken || this.menuState !== state) return;
      if (items.length === 0) {
        this.closeMenu();
        this.render();
        return;
      }
      state.items = items;
      state.wordStart = ctx.wordStart;
      this.showMenu();
    });
  }

  private menuAction(action: 'next' | 'previous' | 'accept' | 'close'): void {
    if (action === 'next') this.deps.menu.move(1);
    else if (action === 'previous') this.deps.menu.move(-1);
    else if (action === 'accept') this.acceptMenuItem(this.deps.menu.selectedIndex);
    else {
      this.closeMenu();
      this.render();
    }
  }

  private acceptMenuItem(index: number): void {
    const state = this.menuState;
    if (!state) return;
    if (state.kind === 'history') {
      const entry = state.items[index];
      this.closeMenu();
      if (entry !== undefined) this.editor.setText(entry);
      this.render();
      return;
    }
    const item = state.items[index];
    this.closeMenu();
    if (item) this.applyCompletion(item, state.wordStart);
    else this.render();
  }

  private applyCompletion(item: CompletionItem, wordStart: number): void {
    this.editor.select(wordStart, this.editor.cursor);
    this.editor.insert(item.insert + (item.appendSpace ? ' ' : ''));
    this.afterEdit();
  }

  private closeMenu(): void {
    this.menuState = null;
    this.completionToken++;
    window.clearTimeout(this.noticeTimer);
    this.deps.menu.hide();
  }

  private showNotice(text: string, offset: number): void {
    this.deps.menu.show({ items: [], emptyText: text, hints: [] }, this.anchorFor(offset));
    window.clearTimeout(this.noticeTimer);
    this.noticeTimer = window.setTimeout(() => this.hideNotice(), NOTICE_MS);
  }

  private hideNotice(): void {
    if (this.menuState === null && this.deps.menu.isOpen) this.deps.menu.hide();
    window.clearTimeout(this.noticeTimer);
  }

  private showMenu(): void {
    const state = this.menuState;
    if (!state) return;
    if (state.kind === 'history') {
      const query = this.editor.text.trim();
      this.deps.menu.show(
        {
          title: query ? `History matching '${query}'` : 'History',
          items: state.items.map((label): MenuItem => ({ label, matched: 0, icon: 'history' })),
          emptyText: 'No matching commands in your history',
          hints: ['type to filter', '↑↓ choose', '↩ use', 'esc close'],
        },
        this.anchorFor(0),
      );
      return;
    }
    this.deps.menu.show(
      {
        items: state.items.map((item): MenuItem => ({ label: item.label, matched: item.matched, ...(item.detail ? { detail: item.detail } : {}), icon: ICON_FOR[item.kind] })),
        emptyText: 'No completions',
        hints: ['↑↓ choose', '⇥ or ↩ accept', 'esc close'],
      },
      this.anchorFor(state.wordStart),
    );
  }

  private positionMenu(): void {
    if (this.menuState && this.deps.menu.isOpen) this.showMenu();
  }

  private anchorFor(offset: number) {
    const fallback = { left: 0, top: 0, width: 0, height: 0 };
    if (!this.start || !this.layout) return fallback;
    const cell = this.layout.positions[Math.min(offset, this.layout.positions.length - 1)] ?? this.layout.end;
    return this.deps.view.cellBox(this.start.marker.line + cell.row, cell.col) ?? fallback;
  }
}

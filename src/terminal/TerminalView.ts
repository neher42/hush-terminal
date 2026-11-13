import { FitAddon } from '@xterm/addon-fit';
import { Unicode11Addon } from '@xterm/addon-unicode11';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { type ITheme, Terminal } from '@xterm/xterm';

export interface CellPosition {
  /** Absolute line in the buffer, including scrollback. */
  line: number;
  col: number;
}

export interface CellBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface TerminalViewOptions {
  theme: ITheme;
  fontSize: number;
  openLink(url: string): void;
  onResize(cols: number, rows: number): void;
}

export const FONT_FAMILY = '"JetBrains Mono", "SF Mono", Menlo, monospace';

export class TerminalView {
  readonly term: Terminal;
  private readonly fitAddon = new FitAddon();
  private readonly resizeObserver: ResizeObserver;
  private resizeFrame = 0;

  constructor(
    readonly container: HTMLElement,
    options: TerminalViewOptions,
  ) {
    this.term = new Terminal({
      allowProposedApi: true,
      fontFamily: FONT_FAMILY,
      fontSize: options.fontSize,
      lineHeight: 1.25,
      cursorBlink: true,
      cursorStyle: 'bar',
      cursorWidth: 2,
      cursorInactiveStyle: 'outline',
      scrollback: 10_000,
      macOptionIsMeta: false,
      macOptionClickForcesSelection: true,
      drawBoldTextInBrightColors: false,
      minimumContrastRatio: 1,
      rightClickSelectsWord: false,
      theme: options.theme,
    });
    this.term.loadAddon(this.fitAddon);
    this.term.loadAddon(new Unicode11Addon());
    this.term.unicode.activeVersion = '11';
    this.term.loadAddon(
      new WebLinksAddon((event, uri) => {
        // Like most terminals, links open on ⌘-click so plain clicks still select text.
        if (event.metaKey) options.openLink(uri);
      }),
    );
    this.term.onResize(({ cols, rows }) => options.onResize(cols, rows));
    this.resizeObserver = new ResizeObserver(() => this.scheduleFit());
  }

  open(): void {
    // No WebGL addon: it clears the canvas on every resize and redraws a frame later,
    // which flickers while resizing the window.
    this.term.open(this.container);
    this.fit();
    this.resizeObserver.observe(this.container);
  }

  setTheme(theme: ITheme): void {
    this.term.options.theme = theme;
  }

  setFontSize(size: number): void {
    this.term.options.fontSize = size;
    this.fit();
  }

  focus(): void {
    this.term.focus();
  }

  fit(): void {
    const next = this.fitAddon.proposeDimensions();
    if (!next || Number.isNaN(next.cols) || Number.isNaN(next.rows)) return;
    if (next.cols === this.term.cols && next.rows === this.term.rows) return;
    this.fitAddon.fit();
  }

  cellAt(clientX: number, clientY: number): CellPosition | null {
    const screen = this.screenRect();
    if (!screen) return null;
    const { width, height } = this.cellSize(screen);
    const col = Math.floor((clientX - screen.left) / width);
    const row = Math.floor((clientY - screen.top) / height);
    if (row < 0 || row >= this.term.rows) return null;
    return { line: this.term.buffer.active.viewportY + row, col: Math.max(0, Math.min(this.term.cols - 1, col)) };
  }

  cellBox(line: number, col: number): CellBox | null {
    const screen = this.screenRect();
    if (!screen) return null;
    const { width, height } = this.cellSize(screen);
    const outer = this.container.getBoundingClientRect();
    const row = line - this.term.buffer.active.viewportY;
    return { left: screen.left - outer.left + col * width, top: screen.top - outer.top + row * height, width, height };
  }

  dispose(): void {
    this.resizeObserver.disconnect();
    cancelAnimationFrame(this.resizeFrame);
    this.term.dispose();
  }

  private scheduleFit(): void {
    cancelAnimationFrame(this.resizeFrame);
    this.resizeFrame = requestAnimationFrame(() => this.fit());
  }

  private screenRect(): DOMRect | null {
    return this.container.querySelector('.xterm-screen')?.getBoundingClientRect() ?? null;
  }

  private cellSize(screen: DOMRect): { width: number; height: number } {
    return { width: screen.width / this.term.cols, height: screen.height / this.term.rows };
  }
}

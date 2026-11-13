import { backend, PtySession } from '../backend';
import { BlockActions } from '../blocks/BlockActions';
import { BlockTracker } from '../blocks/BlockTracker';
import type { CompletionSources } from '../completion/complete';
import { CompletionMenu } from '../completion/CompletionMenu';
import { PromptEditor } from '../editor/PromptEditor';
import type { HistoryStore } from '../history/HistoryStore';
import { Emitter } from '../lib/emitter';
import { baseName, shellPath } from '../lib/paths';
import { CommandCatalog } from '../shell/CommandCatalog';
import type { FolderCheck } from '../shell/FolderCheck';
import type { ShellEvent } from '../shell/events';
import { ShellIntegration } from '../shell/ShellIntegration';
import type { Location } from '../sidebar/DirectoryTree';
import { TerminalView } from '../terminal/TerminalView';
import { type Palette, terminalTheme } from '../theme';

export interface SessionContext {
  home: string;
  history: HistoryStore;
  folders: FolderCheck;
  palette(): Palette;
  fontSize(): number;
  fontFamily(): string;
  copy(text: string, confirmation?: string): void;
  sendToClaude(text: string): void;
  openLink(url: string): void;
  notify(message: string): void;
}

const PTY_RESIZE_DELAY_MS = 80;
const BUSY_MESSAGE = 'Wait for the current command to finish first.';

let nextId = 1;

export class TerminalSession {
  readonly id = nextId++;
  readonly element: HTMLElement;
  readonly changed = new Emitter<void>();
  readonly commandFinished = new Emitter<void>();
  readonly exited = new Emitter<void>();

  private readonly view: TerminalView;
  private readonly editor: PromptEditor;
  private readonly blocks: BlockTracker;
  private readonly catalog = new CommandCatalog();
  private pty: PtySession | null = null;
  private cwd: string;
  private remoteHost: string | null = null;
  private localHost: string | null = null;
  private running: string | null = null;
  private programTitle = '';
  private ptyResizeTimer = 0;
  private unsubscribe: Array<() => void> = [];

  constructor(
    parent: HTMLElement,
    private readonly ctx: SessionContext,
    cwd: string,
  ) {
    this.cwd = cwd;
    this.element = document.createElement('div');
    this.element.className = 'terminal';
    parent.appendChild(this.element);

    this.view = new TerminalView(this.element, {
      theme: terminalTheme(ctx.palette()),
      fontSize: ctx.fontSize(),
      fontFamily: ctx.fontFamily(),
      openLink: (url) => ctx.openLink(url),
      onResize: (cols, rows) => this.onResize(cols, rows),
    });
    this.editor = new PromptEditor({
      view: this.view,
      history: ctx.history,
      catalog: this.catalog,
      menu: new CompletionMenu(this.element),
      completionSources: () => this.completionSources(),
      place: () => ({ cwd: this.cwd, home: ctx.home, isDir: (path) => ctx.folders.isDir(path) }),
      submit: (command, highlights, action) => this.pty?.submit(command, highlights, action),
      writeToShell: (data) => this.pty?.write(data),
      copyText: (text) => ctx.copy(text),
    });
    this.blocks = new BlockTracker(this.view.term);
    this.editor.onRun.on((command) => {
      this.blocks.setCommand(command);
      this.running = command;
      this.changed.emit();
    });
    new BlockActions({
      view: this.view,
      tracker: this.blocks,
      hoverColor: () => ctx.palette().hover,
      copy: (text, confirmation) => ctx.copy(text, confirmation),
      runAgain: (command) => this.run(command),
      sendToClaude: (text) => ctx.sendToClaude(text),
    });

    new ShellIntegration(this.view.term).events.on((event) => this.onShellEvent(event));
    this.view.term.onData((data) => {
      if (!this.editor.handleData(data)) this.pty?.write(data);
    });
    this.view.term.onBinary((data) => this.pty?.write(data));
    this.view.term.onTitleChange((title) => {
      this.programTitle = title;
      this.changed.emit();
    });
    this.unsubscribe.push(ctx.folders.changed.on(() => this.editor.refreshSuggestion()));
    // Clicking the padding around the grid still focuses the terminal.
    this.element.addEventListener('mousedown', () => this.view.focus());
  }

  async start(): Promise<void> {
    this.view.open();
    try {
      this.pty = await PtySession.spawn(this.view.term.cols, this.view.term.rows, this.cwd, {
        onData: (data) => this.view.term.write(data),
        onExit: () => {
          this.pty = null;
          this.editor.deactivate();
          this.exited.emit();
        },
      });
    } catch (err) {
      this.view.term.write(`\r\n\x1b[31mCouldn't start the shell: ${String(err)}\x1b[0m\r\n`);
    }
  }

  get location(): Location {
    return { cwd: this.cwd, home: this.ctx.home, remoteHost: this.remoteHost };
  }

  get title(): string {
    if (this.running !== null && !this.editor.isActive) return this.programTitle || this.running.split(/\s+/).slice(0, 3).join(' ');
    return this.cwd === this.ctx.home ? '~' : baseName(this.cwd);
  }

  show(): void {
    this.element.hidden = false;
    this.view.fit();
    this.view.focus();
  }

  hide(): void {
    this.element.hidden = true;
  }

  focus(): void {
    this.view.focus();
  }

  run(command: string): boolean {
    const ok = this.editor.runCommand(command);
    if (!ok) this.ctx.notify(BUSY_MESSAGE);
    this.view.focus();
    return ok;
  }

  insertPath(path: string, isDir: boolean): void {
    const word = shellPath(path, this.cwd, this.ctx.home) + (isDir && path !== this.cwd ? '/' : '');
    if (!this.editor.insertText(word)) this.ctx.notify(BUSY_MESSAGE);
    this.view.focus();
  }

  openFolder(path: string): void {
    this.run(`cd ${shellPath(path, this.cwd, this.ctx.home)}`);
  }

  clearScreen(): void {
    this.editor.clearScreen();
  }

  selectAll(): void {
    if (!this.editor.selectAll()) this.view.term.selectAll();
  }

  cut(): void {
    this.editor.cut();
  }

  setFontSize(size: number): void {
    this.view.setFontSize(size);
  }

  setFontFamily(family: string): void {
    this.view.setFontFamily(family);
  }

  setPalette(palette: Palette): void {
    this.view.setTheme(terminalTheme(palette));
  }

  dispose(): void {
    window.clearTimeout(this.ptyResizeTimer);
    for (const off of this.unsubscribe) off();
    this.pty?.kill();
    this.view.dispose();
    this.element.remove();
  }

  private onResize(cols: number, rows: number): void {
    this.editor.afterResize();
    window.clearTimeout(this.ptyResizeTimer);
    this.ptyResizeTimer = window.setTimeout(() => this.pty?.resize(cols, rows), PTY_RESIZE_DELAY_MS);
  }

  private onShellEvent(event: ShellEvent): void {
    this.blocks.handleShellEvent(event);
    this.editor.handleShellEvent(event);
    switch (event.type) {
      case 'cwd':
        this.localHost ??= event.host;
        this.remoteHost = event.host && event.host !== this.localHost ? event.host : null;
        this.cwd = event.path;
        this.changed.emit();
        return;
      case 'commandEnd':
        this.running = null;
        this.programTitle = '';
        this.changed.emit();
        this.commandFinished.emit();
        return;
      case 'meta':
        this.onShellMeta(event.key, event.value);
        return;
      default:
        return;
    }
  }

  private onShellMeta(key: string, value: string): void {
    if (key === 'histfile') {
      backend.readHistory(value).then(
        (entries) => this.ctx.history.load(entries),
        (err: unknown) => console.error('Could not read history:', err),
      );
    } else if (key === 'path') {
      this.catalog.setPath(value);
    } else if (key === 'names') {
      this.catalog.setShellNames(value.split(' ').filter(Boolean));
    }
  }

  private completionSources(): CompletionSources {
    return {
      cwd: this.cwd,
      home: this.ctx.home,
      listDir: backend.listDir,
      commands: () => this.catalog.names(),
      gitBranches: backend.gitBranches,
    };
  }
}

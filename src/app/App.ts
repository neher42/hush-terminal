import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { backend } from '../backend';
import { HistoryStore } from '../history/HistoryStore';
import { displayPath } from '../lib/paths';
import { FolderCheck } from '../shell/FolderCheck';
import { DirectoryTree } from '../sidebar/DirectoryTree';
import { PinnedFolders } from '../sidebar/PinnedFolders';
import { togglePin } from '../sidebar/pins';
import { ThemeManager } from '../theme';
import { ContextMenu } from '../ui/ContextMenu';
import { icons } from '../ui/icons';
import { Toast } from '../ui/toast';
import { type Appearance, clampFontSize, DEFAULT_FONT_SIZE, saveSettings, type Settings } from './settings';
import { TabBar } from './TabBar';
import { type SessionContext, TerminalSession } from './TerminalSession';

type MenuId =
  | 'new_tab'
  | 'close_tab'
  | 'clear'
  | 'cut'
  | 'select_all'
  | 'copy_path'
  | 'toggle_sidebar'
  | 'font_bigger'
  | 'font_smaller'
  | 'font_reset'
  | 'previous_tab'
  | 'next_tab'
  | `appearance_${Appearance}`;

interface Elements {
  app: HTMLElement;
  tabs: HTMLElement;
  sidebar: HTMLElement;
  terminalPane: HTMLElement;
  toggleSidebar: HTMLButtonElement;
  newTab: HTMLButtonElement;
  copyCwd: HTMLButtonElement;
}

export class App {
  private readonly theme = new ThemeManager();
  private readonly history = new HistoryStore();
  private readonly folders = new FolderCheck(backend.isDir);
  private readonly toast: Toast;
  private readonly contextMenu: ContextMenu;
  private readonly tree: DirectoryTree;
  private readonly pinned: PinnedFolders;
  private readonly tabBar: TabBar;
  private readonly sessionContext: SessionContext;
  private sessions: TerminalSession[] = [];
  private active: TerminalSession | null = null;

  constructor(
    private readonly el: Elements,
    private readonly home: string,
    private readonly settings: Settings,
  ) {
    this.toast = new Toast(el.app);
    this.contextMenu = new ContextMenu(el.app);
    this.sessionContext = {
      home,
      history: this.history,
      folders: this.folders,
      palette: () => this.theme.palette,
      fontSize: () => this.settings.fontSize,
      copy: (text, confirmation) => void this.copy(text, confirmation),
      sendToClaude: (text) => void this.sendToClaude(text),
      openLink: (url) => void openUrl(url).catch((err: unknown) => this.showError(err)),
      notify: (message) => this.toast.show(message),
    };

    this.tree = new DirectoryTree(el.sidebar, {
      listDir: backend.listDir,
      contextMenu: this.contextMenu,
      insertPath: (path, isDir) => this.active?.insertPath(path, isDir),
      openFolder: (path) => this.active?.openFolder(path),
      copyPath: (path) => void this.copy(path, `Copied ${path}`),
      reveal: (path) => this.reveal(path),
      togglePin: (path) => this.togglePin(path),
    });
    this.pinned = new PinnedFolders({
      contextMenu: this.contextMenu,
      isDir: backend.isDir,
      open: (path) => this.active?.openFolder(path),
      openInNewTab: (path) => void this.openTab(path),
      copyPath: (path) => void this.copy(path, `Copied ${path}`),
      reveal: (path) => this.reveal(path),
      unpin: (path) => this.togglePin(path),
      notify: (message) => this.toast.show(message),
    });
    this.tree.header.after(this.pinned.element);
    this.tabBar = new TabBar(el.tabs, {
      select: (id) => this.activate(this.sessions.find((s) => s.id === id)),
      close: (id) => this.closeTab(this.sessions.find((s) => s.id === id)),
    });

    this.theme.changed.on((palette) => {
      for (const session of this.sessions) session.setPalette(palette);
      this.matchWindowBackground(palette.background);
    });
    this.applyAppearance(this.settings.appearance);
    this.matchWindowBackground(this.theme.palette.background);
    this.setupChrome();
  }

  async start(): Promise<void> {
    await listen<string>('menu', (event) => this.onMenu(event.payload as MenuId));
    await this.openTab(this.home);
  }

  private async openTab(cwd = this.defaultCwd()): Promise<void> {
    const session = new TerminalSession(this.el.terminalPane, this.sessionContext, cwd);
    this.sessions.push(session);
    session.changed.on(() => {
      this.renderTabs();
      if (session === this.active) this.updateLocation();
    });
    session.commandFinished.on(() => {
      // The command may have created, moved or removed files.
      this.folders.clear();
      void this.pinned.checkMissing();
      if (session === this.active) void this.tree.refresh();
    });
    session.exited.on(() => this.closeTab(session));
    this.activate(session);
    await session.start();
    if (session === this.active) session.focus();
  }

  private closeTab(session: TerminalSession | undefined): void {
    if (!session) return;
    const index = this.sessions.indexOf(session);
    if (index < 0) return;
    this.sessions.splice(index, 1);
    session.dispose();
    if (this.sessions.length === 0) {
      // Closing the last tab closes the window, as in a browser.
      void getCurrentWindow().close();
      return;
    }
    // Like browsers: the tab to the right takes its place, or the new last tab.
    if (session === this.active) this.activate(this.sessions[Math.min(index, this.sessions.length - 1)]);
    else this.renderTabs();
  }

  private activate(session: TerminalSession | undefined): void {
    if (!session) return;
    if (this.active && this.active !== session) this.active.hide();
    this.active = session;
    session.show();
    this.updateLocation();
    this.renderTabs();
  }

  private cycle(delta: 1 | -1): void {
    if (!this.active || this.sessions.length < 2) return;
    const index = this.sessions.indexOf(this.active);
    this.activate(this.sessions[(index + delta + this.sessions.length) % this.sessions.length]);
  }

  private selectByNumber(n: number): void {
    const session = n === 9 ? this.sessions[this.sessions.length - 1] : this.sessions[n - 1];
    this.activate(session);
  }

  private defaultCwd(): string {
    const location = this.active?.location;
    return location && !location.remoteHost ? location.cwd : this.home;
  }

  private renderTabs(): void {
    const tabs = this.sessions.map((s) => ({ id: s.id, title: s.title }));
    this.tabBar.render(tabs, this.active?.id ?? -1, this.windowTitle());
  }

  private onMenu(id: MenuId): void {
    const session = this.active;
    switch (id) {
      case 'new_tab':
        void this.openTab();
        return;
      case 'close_tab':
        this.closeTab(session ?? undefined);
        return;
      case 'previous_tab':
        this.cycle(-1);
        return;
      case 'next_tab':
        this.cycle(1);
        return;
      case 'clear':
        session?.clearScreen();
        return;
      case 'cut':
        session?.cut();
        return;
      case 'select_all':
        session?.selectAll();
        return;
      case 'copy_path':
        this.copyCwd();
        return;
      case 'toggle_sidebar':
        this.toggleSidebar();
        return;
      case 'font_bigger':
        this.setFontSize(this.settings.fontSize + 1);
        return;
      case 'font_smaller':
        this.setFontSize(this.settings.fontSize - 1);
        return;
      case 'font_reset':
        this.setFontSize(DEFAULT_FONT_SIZE);
        return;
      case 'appearance_system':
      case 'appearance_light':
      case 'appearance_dark':
        this.settings.appearance = id.slice('appearance_'.length) as Appearance;
        saveSettings(this.settings);
        this.applyAppearance(this.settings.appearance);
        return;
    }
  }

  private setupChrome(): void {
    const { toggleSidebar, newTab, copyCwd, app } = this.el;
    toggleSidebar.innerHTML = icons.sidebar;
    newTab.innerHTML = icons.plus;
    copyCwd.innerHTML = icons.copy;
    for (const button of [toggleSidebar, newTab, copyCwd]) button.addEventListener('mousedown', (e) => e.preventDefault());
    toggleSidebar.addEventListener('click', () => this.toggleSidebar());
    newTab.addEventListener('click', () => void this.openTab());
    copyCwd.addEventListener('click', () => this.copyCwd());
    app.classList.toggle('sidebar-hidden', !this.settings.sidebarVisible);

    // ⌘1–⌘9 aren't menu items (nine entries would crowd the Window menu).
    window.addEventListener(
      'keydown',
      (e) => {
        if (!e.metaKey || e.ctrlKey || e.altKey || e.shiftKey || !/^[1-9]$/.test(e.key)) return;
        e.preventDefault();
        e.stopPropagation();
        this.selectByNumber(Number(e.key));
      },
      { capture: true },
    );
  }

  private updateLocation(): void {
    if (!this.active) return;
    this.tree.setLocation(this.active.location);
    this.renderPins();
    document.title = `${this.active.title} — Hush`;
  }

  private togglePin(path: string): void {
    this.settings.pinnedFolders = togglePin(this.settings.pinnedFolders, path);
    saveSettings(this.settings);
    this.renderPins();
  }

  private renderPins(): void {
    const location = this.active?.location ?? { cwd: this.home, home: this.home, remoteHost: null };
    this.tree.setPinned(this.settings.pinnedFolders);
    this.pinned.render(this.settings.pinnedFolders, location);
  }

  private reveal(path: string): void {
    void revealItemInDir(path).catch((err: unknown) => this.showError(err));
  }

  private windowTitle(): Array<Node | string> {
    if (!this.active) return [];
    const { cwd, remoteHost } = this.active.location;
    const where = (remoteHost ? `${remoteHost}:` : '') + displayPath(cwd, this.home);
    const name = this.active.title;
    const strong = document.createElement('b');
    strong.textContent = name;
    return where === name ? [strong] : [strong, ` — ${where}`];
  }

  private copyCwd(): void {
    const cwd = this.active?.location.cwd;
    if (cwd) void this.copy(cwd, `Copied ${cwd}`);
  }

  private toggleSidebar(): void {
    this.settings.sidebarVisible = !this.settings.sidebarVisible;
    this.el.app.classList.toggle('sidebar-hidden', !this.settings.sidebarVisible);
    saveSettings(this.settings);
    this.active?.focus();
  }

  private setFontSize(size: number): void {
    this.settings.fontSize = clampFontSize(size);
    saveSettings(this.settings);
    for (const session of this.sessions) session.setFontSize(this.settings.fontSize);
  }

  // Setting the window theme also switches prefers-color-scheme in the webview, so
  // ThemeManager picks up the change like a system appearance change.
  private applyAppearance(mode: Appearance): void {
    void getCurrentWindow()
      .setTheme(mode === 'system' ? null : mode)
      .catch((err: unknown) => console.error('Could not set the appearance:', err));
    void backend.setAppearanceMenu(mode).catch((err: unknown) => console.error('Could not update the menu:', err));
  }

  // Avoids white flashes in newly exposed areas while resizing in dark mode.
  private matchWindowBackground(color: string): void {
    void getCurrentWindow()
      .setBackgroundColor(color)
      .catch((err: unknown) => console.error('Could not set the window background:', err));
  }

  private async copy(text: string, confirmation?: string): Promise<void> {
    try {
      await writeText(text);
      if (confirmation) this.toast.show(confirmation);
    } catch (err) {
      this.showError(err);
    }
  }

  private async sendToClaude(text: string): Promise<void> {
    try {
      const delivery = await backend.sendToClaude(text);
      if (delivery.result === 'copied') this.toast.show(delivery.message);
    } catch (err) {
      this.showError(err);
    }
  }

  private showError(err: unknown): void {
    this.toast.show(err instanceof Error ? err.message : String(err), 'error');
  }
}

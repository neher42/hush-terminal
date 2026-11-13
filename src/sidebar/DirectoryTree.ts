import type { DirEntry } from '../backend';
import { displayPath, isInside, joinPath } from '../lib/paths';
import type { ContextMenu } from '../ui/ContextMenu';
import { icons } from '../ui/icons';

export interface Location {
  cwd: string;
  home: string;
  remoteHost: string | null;
}

export interface DirectoryTreeDeps {
  listDir(path: string): Promise<DirEntry[]>;
  contextMenu: ContextMenu;
  insertPath(path: string, isDir: boolean): void;
  openFolder(path: string): void;
  copyPath(path: string): void;
  reveal(path: string): void;
  togglePin(path: string): void;
}

const DIMMED = new Set(['node_modules', '.git', 'target', 'dist', 'build', '.venv', '__pycache__']);
/** Delay before a single click acts, so a double-click can cancel it. */
const CLICK_DELAY_MS = 220;

interface Row {
  path: string;
  name: string;
  depth: number;
  isDir: boolean;
  isOpen: boolean;
}

export class DirectoryTree {
  readonly header: HTMLElement;
  private readonly pathText: HTMLElement;
  private readonly pinButton: HTMLButtonElement;
  private pinned = new Set<string>();
  private readonly tree: HTMLElement;
  private location: Location = { cwd: '/', home: '/', remoteHost: null };
  private readonly listings = new Map<string, DirEntry[]>();
  private readonly expanded = new Set<string>();
  private readonly collapsed = new Set<string>();
  private clickTimer = 0;
  /** Let a newer render or refresh supersede an older one still waiting for listings. */
  private renderToken = 0;
  private refreshToken = 0;
  private centerPending = false;
  private signature = '';

  constructor(
    root: HTMLElement,
    private readonly deps: DirectoryTreeDeps,
  ) {
    const header = document.createElement('div');
    this.header = header;
    header.className = 'sidebar-header';
    const label = document.createElement('div');
    label.className = 'sidebar-label';
    label.textContent = 'Current directory';
    const box = document.createElement('div');
    box.className = 'cwd-box';
    const folder = document.createElement('span');
    folder.className = 'cwd-icon';
    folder.innerHTML = icons.folder;
    this.pathText = document.createElement('span');
    this.pathText.className = 'cwd-path';
    const copy = document.createElement('button');
    copy.type = 'button';
    copy.className = 'icon-button';
    copy.innerHTML = icons.copy;
    copy.setAttribute('aria-label', 'Copy current path');
    copy.dataset.tip = 'Copy path  ⌘⇧C';
    copy.addEventListener('mousedown', (e) => e.preventDefault());
    copy.addEventListener('click', () => deps.copyPath(this.location.cwd));
    this.pinButton = document.createElement('button');
    this.pinButton.type = 'button';
    this.pinButton.className = 'icon-button';
    this.pinButton.innerHTML = icons.pin;
    this.pinButton.addEventListener('mousedown', (e) => e.preventDefault());
    this.pinButton.addEventListener('click', () => deps.togglePin(this.location.cwd));
    box.append(folder, this.pathText, this.pinButton, copy);
    header.append(label, box);

    this.tree = document.createElement('div');
    this.tree.className = 'tree';
    this.tree.setAttribute('role', 'tree');

    const footer = document.createElement('div');
    footer.className = 'sidebar-footer';
    footer.textContent = 'Click inserts a path · double-click opens · right-click for more';

    root.replaceChildren(header, this.tree, footer);
    this.bindEvents();
  }

  setLocation(location: Location): void {
    const moved = location.cwd !== this.location.cwd || location.remoteHost !== this.location.remoteHost;
    this.location = location;
    this.pathText.textContent = (location.remoteHost ? `${location.remoteHost}:` : '') + displayPath(location.cwd, location.home);
    this.pathText.title = location.cwd;
    this.updatePinButton();
    if (moved) {
      this.centerPending = true;
      void this.render();
    }
  }

  setPinned(paths: readonly string[]): void {
    this.pinned = new Set(paths);
    this.updatePinButton();
  }

  private updatePinButton(): void {
    const isPinned = this.pinned.has(this.location.cwd);
    const label = isPinned ? 'Unpin this folder' : 'Pin this folder';
    this.pinButton.classList.toggle('is-on', isPinned);
    this.pinButton.setAttribute('aria-pressed', String(isPinned));
    this.pinButton.setAttribute('aria-label', label);
    this.pinButton.dataset.tip = label;
    this.pinButton.hidden = this.location.remoteHost !== null;
  }

  // Keeps showing the old listings until the new ones are loaded.
  async refresh(): Promise<void> {
    const token = ++this.refreshToken;
    const visible = [...this.listings.keys()].filter((path) => this.isOpen(path));
    const fresh = await Promise.all(visible.map((path) => this.load(path)));
    if (token !== this.refreshToken) return;
    visible.forEach((path, i) => this.listings.set(path, fresh[i]!));
    await this.render();
  }

  // Loads all needed listings first so the tree never renders half-built.
  private async render(): Promise<void> {
    const token = ++this.renderToken;
    const { remoteHost } = this.location;
    if (remoteHost) {
      const note = document.createElement('p');
      note.className = 'tree-note';
      note.textContent = `Browsing folders on ${remoteHost} isn't available. The tree shows your Mac's folders when you're back.`;
      this.tree.replaceChildren(note);
      this.signature = '';
      return;
    }

    let { rows, missing } = this.rows();
    while (missing.length > 0) {
      const loaded = await Promise.all(missing.map((path) => this.load(path)));
      if (token !== this.renderToken) return;
      missing.forEach((path, i) => this.listings.set(path, loaded[i]!));
      ({ rows, missing } = this.rows());
    }

    const signature = `${this.location.cwd}\n${rows.map((r) => `${r.path}:${r.isOpen ? 1 : 0}`).join('\n')}`;
    if (signature !== this.signature) {
      this.signature = signature;
      this.tree.replaceChildren(...rows.map((row) => this.rowElement(row)));
    }
    if (this.centerPending) {
      this.centerPending = false;
      this.centerCurrent();
    }
  }

  private centerCurrent(): void {
    const current = this.tree.querySelector<HTMLElement>('.is-current');
    if (!current) return;
    this.tree.scrollTop = current.offsetTop - (this.tree.clientHeight - current.offsetHeight) / 2;
  }

  private rows(): { rows: Row[]; missing: string[] } {
    const { cwd, home } = this.location;
    const rootPath = home && isInside(cwd, home) ? home : '/';
    const rows: Row[] = [];
    const missing: string[] = [];
    const walk = (path: string, name: string, depth: number): void => {
      const open = this.isOpen(path);
      rows.push({ path, name, depth, isDir: true, isOpen: open });
      if (!open) return;
      const entries = this.listings.get(path);
      if (!entries) {
        missing.push(path);
        return;
      }
      const showFiles = path === cwd || this.expanded.has(path);
      for (const entry of entries) {
        if (entry.name.startsWith('.')) continue;
        const child = joinPath(path, entry.name);
        if (entry.isDir) walk(child, entry.name, depth + 1);
        else if (showFiles) rows.push({ path: child, name: entry.name, depth: depth + 1, isDir: false, isOpen: false });
      }
    };
    walk(rootPath, rootPath === home ? '~' : '/', 0);
    return { rows, missing };
  }

  private isOpen(path: string): boolean {
    if (this.collapsed.has(path)) return false;
    if (this.expanded.has(path)) return true;
    return isInside(this.location.cwd, path);
  }

  private load(path: string): Promise<DirEntry[]> {
    return this.deps.listDir(path).catch(() => []);
  }

  private rowElement(row: Row): HTMLElement {
    const el = document.createElement('div');
    const isCurrent = row.path === this.location.cwd;
    el.className = [
      'tree-row',
      isCurrent ? 'is-current' : '',
      row.isOpen ? 'is-open' : '',
      DIMMED.has(row.name) ? 'is-dimmed' : '',
      !isCurrent && row.isDir && isInside(this.location.cwd, row.path) ? 'is-ancestor' : '',
    ]
      .filter(Boolean)
      .join(' ');
    el.setAttribute('role', 'treeitem');
    if (row.isDir) el.setAttribute('aria-expanded', String(row.isOpen));
    el.style.paddingLeft = `${4 + row.depth * 14}px`;
    el.dataset.path = row.path;
    el.dataset.dir = String(row.isDir);
    el.title = row.path;

    const chevron = document.createElement('span');
    chevron.className = 'tree-chevron';
    if (row.isDir) chevron.innerHTML = icons.chevron;
    const icon = document.createElement('span');
    icon.className = 'tree-icon';
    icon.innerHTML = row.isDir ? icons.folder : icons.file;
    const name = document.createElement('span');
    name.className = 'tree-name';
    name.textContent = row.name;
    el.append(chevron, icon, name);
    return el;
  }

  private bindEvents(): void {
    const rowFrom = (e: Event): { path: string; isDir: boolean; el: HTMLElement } | null => {
      const el = (e.target as HTMLElement).closest<HTMLElement>('.tree-row');
      if (!el?.dataset.path) return null;
      return { path: el.dataset.path, isDir: el.dataset.dir === 'true', el };
    };

    // Keep keyboard focus in the terminal.
    this.tree.addEventListener('mousedown', (e) => {
      if ((e.target as HTMLElement).closest('.tree-row')) e.preventDefault();
    });

    this.tree.addEventListener('click', (e) => {
      const row = rowFrom(e);
      if (!row) return;
      if (row.isDir && (e.target as HTMLElement).closest('.tree-chevron')) {
        this.toggle(row.path, row.el.classList.contains('is-open'));
        return;
      }
      window.clearTimeout(this.clickTimer);
      this.clickTimer = window.setTimeout(() => this.deps.insertPath(row.path, row.isDir), row.isDir ? CLICK_DELAY_MS : 0);
    });

    this.tree.addEventListener('dblclick', (e) => {
      const row = rowFrom(e);
      if (!row || (e.target as HTMLElement).closest('.tree-chevron')) return;
      window.clearTimeout(this.clickTimer);
      if (row.isDir) this.deps.openFolder(row.path);
    });

    this.tree.addEventListener('contextmenu', (e) => {
      const row = rowFrom(e);
      if (!row) return;
      e.preventDefault();
      this.deps.contextMenu.show(
        [
          { label: 'Copy path', run: () => this.deps.copyPath(row.path) },
          { label: 'Insert path', run: () => this.deps.insertPath(row.path, row.isDir) },
          ...(row.isDir
            ? [
                { label: 'Open here', shortcut: 'cd', run: () => this.deps.openFolder(row.path) },
                { label: this.pinned.has(row.path) ? 'Unpin' : 'Pin', run: () => this.deps.togglePin(row.path) },
              ]
            : []),
          null,
          { label: 'Reveal in Finder', run: () => this.deps.reveal(row.path) },
        ],
        e.clientX,
        e.clientY,
      );
    });
  }

  private toggle(path: string, isOpen: boolean): void {
    if (isOpen) {
      this.expanded.delete(path);
      if (isInside(this.location.cwd, path)) this.collapsed.add(path);
    } else {
      this.collapsed.delete(path);
      this.expanded.add(path);
    }
    void this.render();
  }
}

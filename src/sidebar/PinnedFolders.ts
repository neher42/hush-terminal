import { baseName, displayPath, parentPath } from '../lib/paths';
import type { ContextMenu } from '../ui/ContextMenu';
import { icons } from '../ui/icons';
import type { Location } from './DirectoryTree';

export interface PinnedFoldersDeps {
  contextMenu: ContextMenu;
  isDir(path: string): Promise<boolean>;
  open(path: string): void;
  openInNewTab(path: string): void;
  copyPath(path: string): void;
  reveal(path: string): void;
  unpin(path: string): void;
  notify(message: string): void;
}

export class PinnedFolders {
  readonly element: HTMLElement;
  private readonly list: HTMLElement;
  private paths: string[] = [];
  private location: Location = { cwd: '/', home: '/', remoteHost: null };
  private missing = new Set<string>();
  private checkToken = 0;

  constructor(private readonly deps: PinnedFoldersDeps) {
    this.element = document.createElement('section');
    this.element.className = 'pinned';
    this.element.setAttribute('aria-label', 'Pinned folders');
    const label = document.createElement('div');
    label.className = 'sidebar-label';
    label.textContent = 'Pinned';
    this.list = document.createElement('div');
    this.list.className = 'pinned-list';
    this.element.append(label, this.list);
    this.bindEvents();
  }

  render(paths: string[], location: Location): void {
    const pathsChanged = paths.join('\n') !== this.paths.join('\n');
    this.paths = paths;
    this.location = location;
    this.element.hidden = paths.length === 0;
    this.draw();
    if (pathsChanged) void this.checkMissing();
  }

  async checkMissing(): Promise<void> {
    const token = ++this.checkToken;
    const exists = await Promise.all(this.paths.map((path) => this.deps.isDir(path).catch(() => false)));
    if (token !== this.checkToken) return;
    const missing = new Set(this.paths.filter((_, i) => !exists[i]));
    if ([...missing].join('\n') === [...this.missing].join('\n')) return;
    this.missing = missing;
    this.draw();
  }

  private draw(): void {
    const { cwd, home } = this.location;
    this.list.replaceChildren(
      ...this.paths.map((path) => {
        const row = document.createElement('div');
        const isCurrent = path === cwd && !this.location.remoteHost;
        const isMissing = this.missing.has(path);
        row.className = `pinned-row${isCurrent ? ' is-current' : ''}${isMissing ? ' is-missing' : ''}`;
        row.dataset.path = path;
        row.title = isMissing ? `${path} no longer exists` : path;
        row.setAttribute('role', 'button');

        const icon = document.createElement('span');
        icon.className = 'tree-icon';
        icon.innerHTML = icons.folder;
        const name = document.createElement('span');
        name.className = 'pinned-name';
        name.textContent = path === home ? '~' : baseName(path);
        const where = document.createElement('span');
        where.className = 'pinned-where';
        where.textContent = path === home || path === '/' ? '' : displayPath(parentPath(path), home);
        row.append(icon, name, where);
        return row;
      }),
    );
  }

  private bindEvents(): void {
    const pathFrom = (e: Event): string | null => (e.target as HTMLElement).closest<HTMLElement>('.pinned-row')?.dataset.path ?? null;

    // Keep keyboard focus in the terminal.
    this.list.addEventListener('mousedown', (e) => {
      if (pathFrom(e)) e.preventDefault();
    });
    this.list.addEventListener('click', (e) => {
      const path = pathFrom(e);
      if (!path) return;
      if (this.missing.has(path)) {
        this.deps.notify(`${displayPath(path, this.location.home)} no longer exists. Right-click to unpin it.`);
        return;
      }
      this.deps.open(path);
    });
    this.list.addEventListener('contextmenu', (e) => {
      const path = pathFrom(e);
      if (!path) return;
      e.preventDefault();
      this.deps.contextMenu.show(
        [
          { label: 'Open here', shortcut: 'cd', run: () => this.deps.open(path) },
          { label: 'Open in new tab', run: () => this.deps.openInNewTab(path) },
          null,
          { label: 'Copy path', run: () => this.deps.copyPath(path) },
          { label: 'Reveal in Finder', run: () => this.deps.reveal(path) },
          null,
          { label: 'Unpin', run: () => this.deps.unpin(path) },
        ],
        e.clientX,
        e.clientY,
      );
    });
  }
}

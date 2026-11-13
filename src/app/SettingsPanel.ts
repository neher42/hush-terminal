import { baseName, displayPath, parentPath } from '../lib/paths';
import { movePin } from '../sidebar/pins';
import { type Appearance, clampFontSize, FONTS, type FontChoice, type Settings } from './settings';

export interface SettingsPanelDeps {
  home: string;
  settings(): Settings;
  update(patch: Partial<Settings>): void;
  claudePermission(): Promise<boolean>;
  openPermissionSettings(): void;
  onClose(): void;
}

const APPEARANCE_LABELS: Record<Appearance, string> = { system: 'Match System', light: 'Light', dark: 'Dark' };

export class SettingsPanel {
  private readonly backdrop: HTMLElement;
  private readonly body: HTMLElement;
  private permission: boolean | null = null;

  constructor(
    parent: HTMLElement,
    private readonly deps: SettingsPanelDeps,
  ) {
    this.backdrop = document.createElement('div');
    this.backdrop.className = 'settings-backdrop';
    this.backdrop.hidden = true;

    const panel = document.createElement('div');
    panel.className = 'settings-panel';
    panel.setAttribute('role', 'dialog');
    panel.setAttribute('aria-modal', 'true');
    panel.setAttribute('aria-labelledby', 'settings-title');

    const header = document.createElement('header');
    header.className = 'settings-header';
    const title = document.createElement('h2');
    title.id = 'settings-title';
    title.textContent = 'Settings';
    const close = button('×', 'settings-close', () => this.close());
    close.setAttribute('aria-label', 'Close settings');
    close.dataset.key = 'close';
    header.append(title, close);

    this.body = document.createElement('div');
    this.body.className = 'settings-body';
    panel.append(header, this.body);
    this.backdrop.appendChild(panel);
    parent.appendChild(this.backdrop);

    this.backdrop.addEventListener('mousedown', (e) => {
      if (e.target === this.backdrop) this.close();
    });
    document.addEventListener(
      'keydown',
      (e) => {
        if (this.isOpen && e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          this.close();
        }
      },
      { capture: true },
    );
    // Coming back from System Settings: show whether the permission was granted.
    window.addEventListener('focus', () => {
      if (this.isOpen) void this.checkPermission();
    });
  }

  get isOpen(): boolean {
    return !this.backdrop.hidden;
  }

  open(): void {
    this.backdrop.hidden = false;
    this.render();
    this.body.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
    void this.checkPermission();
  }

  close(): void {
    if (!this.isOpen) return;
    this.backdrop.hidden = true;
    this.deps.onClose();
  }

  /** Redraws after a setting changed elsewhere (menu, sidebar). */
  refresh(): void {
    if (this.isOpen) this.render();
  }

  private async checkPermission(): Promise<void> {
    const granted = await this.deps.claudePermission().catch(() => false);
    if (granted === this.permission) return;
    this.permission = granted;
    this.refresh();
  }

  private render(): void {
    // Rebuilding replaces the focused button, so remember which one it was.
    const focusKey = (document.activeElement as HTMLElement | null)?.dataset.key;
    const s = this.deps.settings();
    this.body.replaceChildren(
      section('Appearance', [
        choices<Appearance>('Appearance', ['system', 'light', 'dark'], s.appearance, (v) => APPEARANCE_LABELS[v], (appearance) =>
          this.deps.update({ appearance }),
        ),
      ]),
      section('Text', [this.fontSizeRow(s), this.fontRow(s)]),
      section('Sidebar', [this.sidebarRow(s), this.gitRow(s), this.pinnedList(s)]),
      section('Send to Claude', [this.permissionRow()]),
    );
    if (focusKey) this.backdrop.querySelector<HTMLElement>(`[data-key="${focusKey}"]`)?.focus();
  }

  private fontSizeRow(s: Settings): HTMLElement {
    const smaller = button('−', 'stepper', () => this.deps.update({ fontSize: clampFontSize(s.fontSize - 1) }));
    smaller.setAttribute('aria-label', 'Smaller text');
    smaller.dataset.key = 'font-smaller';
    const bigger = button('+', 'stepper', () => this.deps.update({ fontSize: clampFontSize(s.fontSize + 1) }));
    bigger.setAttribute('aria-label', 'Bigger text');
    bigger.dataset.key = 'font-bigger';
    const value = document.createElement('span');
    value.className = 'stepper-value';
    value.textContent = `${s.fontSize} pt`;
    const controls = document.createElement('div');
    controls.className = 'stepper-group';
    controls.append(smaller, value, bigger);
    return row('Size', controls);
  }

  private fontRow(s: Settings): HTMLElement {
    const ids = Object.keys(FONTS) as FontChoice[];
    const group = choices<FontChoice>('Font', ids, s.font, (id) => FONTS[id].label, (font) => this.deps.update({ font }));
    group.querySelectorAll<HTMLElement>('[role="radio"]').forEach((el, i) => {
      el.style.fontFamily = FONTS[ids[i]!].css;
    });
    return row('Font', group);
  }

  private sidebarRow(s: Settings): HTMLElement {
    return row('Show folder tree', toggle('sidebar', s.sidebarVisible, (sidebarVisible) => this.deps.update({ sidebarVisible })), '⌘B');
  }

  private gitRow(s: Settings): HTMLElement {
    return row('Show git status', toggle('git', s.showGitStatus, (showGitStatus) => this.deps.update({ showGitStatus })));
  }

  private pinnedList(s: Settings): HTMLElement {
    const wrap = document.createElement('div');
    wrap.className = 'settings-pins';
    const heading = document.createElement('div');
    heading.className = 'settings-row-label';
    heading.textContent = 'Pinned folders';
    wrap.appendChild(heading);
    if (s.pinnedFolders.length === 0) {
      wrap.appendChild(note('Nothing pinned yet. Use the pin button next to the current folder.'));
      return wrap;
    }
    const list = document.createElement('ul');
    list.className = 'pin-list';
    s.pinnedFolders.forEach((path, i) => {
      const item = document.createElement('li');
      const name = document.createElement('span');
      name.className = 'pin-name';
      name.textContent = path === this.deps.home ? '~' : baseName(path);
      name.title = path;
      const where = document.createElement('span');
      where.className = 'pin-where';
      where.textContent = displayPath(parentPath(path), this.deps.home);
      const up = button('↑', 'pin-action', () => this.deps.update({ pinnedFolders: movePin(s.pinnedFolders, i, -1) }));
      up.setAttribute('aria-label', `Move ${name.textContent} up`);
      up.disabled = i === 0;
      up.dataset.key = `up-${path}`;
      const down = button('↓', 'pin-action', () => this.deps.update({ pinnedFolders: movePin(s.pinnedFolders, i, 1) }));
      down.setAttribute('aria-label', `Move ${name.textContent} down`);
      down.disabled = i === s.pinnedFolders.length - 1;
      down.dataset.key = `down-${path}`;
      const remove = button('×', 'pin-action', () => this.deps.update({ pinnedFolders: s.pinnedFolders.filter((p) => p !== path) }));
      remove.setAttribute('aria-label', `Unpin ${name.textContent}`);
      item.append(name, where, up, down, remove);
      list.appendChild(item);
    });
    wrap.appendChild(list);
    return wrap;
  }

  private permissionRow(): HTMLElement {
    const status = document.createElement('span');
    status.className = `permission ${this.permission ? 'is-granted' : 'is-missing'}`;
    status.textContent = this.permission === null ? 'Checking…' : this.permission ? 'Allowed' : 'Not allowed';
    const open = button('Open Accessibility Settings', 'settings-button', () => this.deps.openPermissionSettings());
    open.dataset.key = 'permission';
    const controls = document.createElement('div');
    controls.className = 'permission-controls';
    controls.append(status, open);
    const wrap = document.createElement('div');
    wrap.append(
      row('Accessibility permission', controls),
      note('Still not allowed after enabling it? Remove Hush from the list and add it again.'),
    );
    return wrap;
  }
}

function section(title: string, children: HTMLElement[]): HTMLElement {
  const el = document.createElement('section');
  el.className = 'settings-section';
  const h = document.createElement('h3');
  h.textContent = title;
  el.append(h, ...children);
  return el;
}

function row(label: string, control: HTMLElement, hint?: string): HTMLElement {
  const el = document.createElement('div');
  el.className = 'settings-row';
  const text = document.createElement('span');
  text.className = 'settings-row-label';
  text.textContent = label;
  if (hint) {
    const kbd = document.createElement('kbd');
    kbd.textContent = hint;
    text.append(' ', kbd);
  }
  el.append(text, control);
  return el;
}

function toggle(key: string, checked: boolean, change: (checked: boolean) => void): HTMLElement {
  const label = document.createElement('label');
  label.className = 'switch';
  const input = document.createElement('input');
  input.type = 'checkbox';
  input.checked = checked;
  input.dataset.key = key;
  input.addEventListener('change', () => change(input.checked));
  const track = document.createElement('span');
  track.className = 'switch-track';
  label.append(input, track);
  return label;
}

function choices<T extends string>(label: string, values: T[], current: T, text: (v: T) => string, pick: (v: T) => void): HTMLElement {
  const group = document.createElement('div');
  group.className = 'segmented';
  group.setAttribute('role', 'radiogroup');
  group.setAttribute('aria-label', label);
  for (const value of values) {
    const el = button(text(value), 'segment', () => pick(value));
    el.setAttribute('role', 'radio');
    el.setAttribute('aria-checked', String(value === current));
    el.dataset.key = `${label}-${value}`;
    group.appendChild(el);
  }
  return group;
}

function button(text: string, className: string, onClick: () => void): HTMLButtonElement {
  const el = document.createElement('button');
  el.type = 'button';
  el.className = className;
  el.textContent = text;
  el.addEventListener('click', onClick);
  return el;
}

function note(text: string): HTMLElement {
  const el = document.createElement('p');
  el.className = 'settings-note';
  el.textContent = text;
  return el;
}

export interface MenuEntry {
  label: string;
  shortcut?: string;
  run(): void;
}

export class ContextMenu {
  private readonly el: HTMLElement;

  constructor(private readonly parent: HTMLElement) {
    this.el = document.createElement('div');
    this.el.className = 'context-menu';
    this.el.setAttribute('role', 'menu');
    this.el.hidden = true;
    parent.appendChild(this.el);
    document.addEventListener('mousedown', (e) => {
      if (!this.el.contains(e.target as Node)) this.hide();
    });
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') this.hide();
    });
    window.addEventListener('blur', () => this.hide());
  }

  /** `null` entries draw a separator. */
  show(entries: Array<MenuEntry | null>, clientX: number, clientY: number): void {
    this.el.replaceChildren(
      ...entries.map((entry) => {
        if (!entry) {
          const hr = document.createElement('hr');
          return hr;
        }
        const item = document.createElement('button');
        item.type = 'button';
        item.className = 'context-menu-item';
        item.setAttribute('role', 'menuitem');
        item.textContent = entry.label;
        if (entry.shortcut) {
          const kbd = document.createElement('kbd');
          kbd.textContent = entry.shortcut;
          item.appendChild(kbd);
        }
        item.addEventListener('mousedown', (e) => e.preventDefault());
        item.addEventListener('click', () => {
          this.hide();
          entry.run();
        });
        return item;
      }),
    );
    this.el.hidden = false;
    const bounds = this.parent.getBoundingClientRect();
    const x = Math.min(clientX - bounds.left, bounds.width - this.el.offsetWidth - 8);
    const y = Math.min(clientY - bounds.top, bounds.height - this.el.offsetHeight - 8);
    this.el.style.left = `${Math.max(8, x)}px`;
    this.el.style.top = `${Math.max(8, y)}px`;
  }

  hide(): void {
    this.el.hidden = true;
  }
}

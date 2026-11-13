import type { CellBox } from '../terminal/TerminalView';
import { type IconName, icons } from '../ui/icons';

export interface MenuItem {
  label: string;
  matched: number;
  detail?: string;
  icon: IconName;
}

export interface MenuContent {
  title?: string;
  items: MenuItem[];
  emptyText: string;
  hints: string[];
}

export class CompletionMenu {
  private readonly el: HTMLElement;
  private readonly list: HTMLElement;
  private items: MenuItem[] = [];
  private selected = 0;
  onPick: (index: number) => void = () => {};

  constructor(private readonly container: HTMLElement) {
    this.el = document.createElement('div');
    this.el.className = 'completion-menu';
    this.el.hidden = true;
    this.el.setAttribute('role', 'listbox');
    this.list = document.createElement('div');
    this.list.className = 'completion-list';
    container.appendChild(this.el);
  }

  get isOpen(): boolean {
    return !this.el.hidden;
  }

  get selectedIndex(): number {
    return this.selected;
  }

  show(content: MenuContent, anchor: CellBox): void {
    this.items = content.items;
    this.selected = Math.min(this.selected, Math.max(0, this.items.length - 1));
    const children: HTMLElement[] = [];
    if (content.title) {
      const title = document.createElement('div');
      title.className = 'completion-title';
      title.textContent = content.title;
      children.push(title);
    }
    this.renderItems(content.emptyText);
    children.push(this.list);
    if (content.hints.length > 0) {
      const hints = document.createElement('div');
      hints.className = 'completion-hints';
      for (const hint of content.hints) {
        const span = document.createElement('span');
        span.textContent = hint;
        hints.appendChild(span);
      }
      children.push(hints);
    }
    this.el.replaceChildren(...children);
    this.el.hidden = false;
    this.position(anchor);
  }

  hide(): void {
    this.el.hidden = true;
    this.selected = 0;
  }

  move(delta: 1 | -1): void {
    if (this.items.length === 0) return;
    this.selected = (this.selected + delta + this.items.length) % this.items.length;
    this.list.querySelectorAll('.completion-item').forEach((row, i) => row.classList.toggle('is-selected', i === this.selected));
    this.list.children[this.selected]?.scrollIntoView({ block: 'nearest' });
  }

  private renderItems(emptyText: string): void {
    if (this.items.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'completion-empty';
      empty.textContent = emptyText;
      this.list.replaceChildren(empty);
      return;
    }
    this.list.replaceChildren(
      ...this.items.map((item, i) => {
        const row = document.createElement('div');
        row.className = `completion-item${i === this.selected ? ' is-selected' : ''}`;
        row.setAttribute('role', 'option');
        const icon = document.createElement('span');
        icon.className = 'completion-icon';
        icon.innerHTML = icons[item.icon];
        const label = document.createElement('span');
        label.className = 'completion-label';
        const matched = Math.min(item.matched, item.label.length);
        if (matched > 0) {
          const strong = document.createElement('b');
          strong.textContent = item.label.slice(0, matched);
          label.append(strong, item.label.slice(matched));
        } else {
          label.textContent = item.label;
        }
        row.append(icon, label);
        if (item.detail) {
          const detail = document.createElement('span');
          detail.className = 'completion-detail';
          detail.textContent = item.detail;
          row.appendChild(detail);
        }
        // Keep focus in the terminal while clicking.
        row.addEventListener('mousedown', (e) => {
          e.preventDefault();
          this.onPick(i);
        });
        return row;
      }),
    );
    this.list.children[this.selected]?.scrollIntoView({ block: 'nearest' });
  }

  private position(anchor: CellBox): void {
    const bounds = this.container.getBoundingClientRect();
    const { offsetWidth: width, offsetHeight: height } = this.el;
    const left = Math.max(8, Math.min(anchor.left - 8, bounds.width - width - 8));
    const below = anchor.top + anchor.height + 4;
    const top = below + height <= bounds.height - 8 ? below : Math.max(8, anchor.top - height - 4);
    this.el.style.left = `${left}px`;
    this.el.style.top = `${top}px`;
  }
}

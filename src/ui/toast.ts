const VISIBLE_MS = 1800;
const ERROR_VISIBLE_MS = 5000;

export class Toast {
  private readonly el: HTMLElement;
  private timer = 0;

  constructor(parent: HTMLElement) {
    this.el = document.createElement('div');
    this.el.className = 'toast';
    this.el.setAttribute('role', 'status');
    this.el.setAttribute('aria-live', 'polite');
    parent.appendChild(this.el);
  }

  show(message: string, kind: 'info' | 'error' = 'info'): void {
    this.el.textContent = message;
    this.el.classList.toggle('toast-error', kind === 'error');
    this.el.classList.add('toast-visible');
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.el.classList.remove('toast-visible'), kind === 'error' ? ERROR_VISIBLE_MS : VISIBLE_MS);
  }
}

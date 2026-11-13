export interface TabInfo {
  id: number;
  title: string;
}

export interface TabBarHandlers {
  select(id: number): void;
  close(id: number): void;
}

export class TabBar {
  constructor(
    private readonly el: HTMLElement,
    handlers: TabBarHandlers,
  ) {
    el.setAttribute('role', 'tablist');
    // Keep keyboard focus in the terminal when clicking tabs.
    el.addEventListener('mousedown', (e) => {
      if ((e.target as HTMLElement).closest('.tab')) e.preventDefault();
    });
    el.addEventListener('click', (e) => {
      const target = e.target as HTMLElement;
      const tab = target.closest<HTMLElement>('.tab');
      if (!tab) return;
      const id = Number(tab.dataset.id);
      if (target.closest('.tab-close')) handlers.close(id);
      else handlers.select(id);
    });
    // Middle-click closes a tab, as in browsers.
    el.addEventListener('auxclick', (e) => {
      const tab = (e.target as HTMLElement).closest<HTMLElement>('.tab');
      if (tab && e.button === 1) handlers.close(Number(tab.dataset.id));
    });
  }

  render(tabs: TabInfo[], activeId: number, windowTitle: Array<Node | string>): void {
    this.el.classList.toggle('has-tabs', tabs.length > 1);
    if (tabs.length <= 1) {
      const title = document.createElement('div');
      title.className = 'titlebar-title';
      title.dataset.tauriDragRegion = '';
      title.append(...windowTitle);
      this.el.replaceChildren(title);
      return;
    }
    this.el.replaceChildren(
      ...tabs.map((tab) => {
        const el = document.createElement('div');
        el.className = `tab${tab.id === activeId ? ' is-active' : ''}`;
        el.setAttribute('role', 'tab');
        el.setAttribute('aria-selected', String(tab.id === activeId));
        el.dataset.id = String(tab.id);
        el.title = tab.title;
        const label = document.createElement('span');
        label.className = 'tab-title';
        label.textContent = tab.title;
        const close = document.createElement('button');
        close.type = 'button';
        close.className = 'tab-close';
        close.setAttribute('aria-label', `Close ${tab.title}`);
        close.textContent = '×';
        el.append(close, label);
        return el;
      }),
    );
  }
}

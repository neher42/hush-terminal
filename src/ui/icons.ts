const svg = (paths: string): string =>
  `<svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths}</svg>`;

export const icons = {
  copy: svg('<rect x="5.5" y="5.5" width="8" height="8" rx="1.6"/><path d="M10.5 3.5v-.3a.9.9 0 0 0-.9-.9H3.2a.9.9 0 0 0-.9.9v6.4c0 .5.4.9.9.9h.3"/>'),
  command: svg('<path d="M3 4.5 6.5 8 3 11.5M8.5 12H13"/>'),
  rerun: svg('<path d="M13 8a5 5 0 1 1-1.5-3.6"/><path d="M13 2.5v3h-3"/>'),
  send: svg('<path d="M8 13V3.5M4 7.5 8 3.5l4 4"/>'),
  folder: svg('<path d="M2 4.6c0-.6.5-1.1 1.1-1.1h2.8l1.5 1.6h5.5c.6 0 1.1.5 1.1 1.1v5.7c0 .6-.5 1.1-1.1 1.1H3.1c-.6 0-1.1-.5-1.1-1.1z"/>'),
  file: svg('<path d="M4 2h5l3 3v9H4z"/><path d="M9 2v3h3"/>'),
  branch: svg('<circle cx="5" cy="3.8" r="1.5"/><circle cx="5" cy="12.2" r="1.5"/><circle cx="11" cy="5.5" r="1.5"/><path d="M5 5.3v5.4M11 7c0 2.6-6 1.4-6 3.6"/>'),
  prompt: svg('<path d="M4.5 4.5 8 8l-3.5 3.5"/>'),
  history: svg('<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3.2l2 1.3"/>'),
  flag: svg('<path d="M4.5 8h7"/>'),
  chevron: svg('<path d="M6 3.5 10.5 8 6 12.5"/>'),
  pin: svg('<path d="M5.5 2.5h5M6.5 2.5v4L4.5 9h7l-2-2.5v-4M8 9v4.5"/>'),
  plus: svg('<path d="M8 3.5v9M3.5 8h9"/>'),
  sidebar: svg('<rect x="2" y="3" width="12" height="10" rx="1.6"/><path d="M6.2 3v10"/>'),
  remote: svg('<rect x="2.5" y="3" width="11" height="4" rx="1"/><rect x="2.5" y="9" width="11" height="4" rx="1"/>'),
} as const;

export type IconName = keyof typeof icons;

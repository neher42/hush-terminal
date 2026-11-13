import type { ITheme } from '@xterm/xterm';
import { Emitter } from './lib/emitter';

export interface Palette {
  background: string;
  chrome: string;
  line: string;
  foreground: string;
  secondary: string;
  muted: string;
  faint: string;
  accent: string;
  selection: string;
  /** Faint wash behind a hovered command. Must be a #rrggbb color. */
  hover: string;
  currentRow: string;
  currentRowText: string;
  menu: string;
  menuSelected: string;
  tooltip: string;
  tooltipText: string;
  shadow: string;
  ansi: [black: string, red: string, green: string, yellow: string, blue: string, magenta: string, cyan: string, white: string];
}

const light: Palette = {
  background: '#ffffff',
  chrome: '#f5f6f8',
  line: '#e2e5ea',
  foreground: '#1b1e23',
  secondary: '#3d434c',
  muted: '#878e99',
  faint: '#a9afb8',
  accent: '#2a63c7',
  selection: 'rgba(42, 99, 199, 0.22)',
  hover: '#f6f7f9',
  currentRow: '#e2eafa',
  currentRowText: '#1d4ea6',
  menu: '#ffffff',
  menuSelected: '#e7eefa',
  tooltip: '#1b1e23',
  tooltipText: '#ffffff',
  shadow: '0 1px 2px rgba(15, 20, 30, 0.08), 0 10px 28px rgba(15, 25, 45, 0.16)',
  ansi: ['#1b1e23', '#cc3a2f', '#25804b', '#a2670a', '#2a63c7', '#8a4ec2', '#1b7f8c', '#c9ced6'],
};

const dark: Palette = {
  background: '#16181c',
  chrome: '#1b1e23',
  line: '#2a2e35',
  foreground: '#dde1e7',
  secondary: '#b9bfc8',
  muted: '#737a85',
  faint: '#5a616c',
  accent: '#7aa8ff',
  selection: 'rgba(122, 168, 255, 0.30)',
  hover: '#1c1f24',
  currentRow: '#223150',
  currentRowText: '#adc8ff',
  menu: '#202329',
  menuSelected: '#2a3447',
  tooltip: '#e6e9ee',
  tooltipText: '#16181c',
  shadow: '0 1px 2px rgba(0, 0, 0, 0.4), 0 12px 32px rgba(0, 0, 0, 0.5)',
  ansi: ['#3a3f47', '#ff6e61', '#7fcf98', '#e2b25d', '#7aa8ff', '#c4a1ff', '#5fc7d3', '#dde1e7'],
};

export function terminalTheme(p: Palette): ITheme {
  const [black, red, green, yellow, blue, magenta, cyan, white] = p.ansi;
  return {
    background: p.background,
    foreground: p.foreground,
    cursor: p.foreground,
    cursorAccent: p.background,
    selectionBackground: p.selection,
    selectionInactiveBackground: p.selection,
    scrollbarSliderBackground: 'rgba(128, 128, 128, 0.25)',
    scrollbarSliderHoverBackground: 'rgba(128, 128, 128, 0.4)',
    scrollbarSliderActiveBackground: 'rgba(128, 128, 128, 0.5)',
    black,
    red,
    green,
    yellow,
    blue,
    magenta,
    cyan,
    white,
    brightBlack: p.faint,
    brightRed: red,
    brightGreen: green,
    brightYellow: yellow,
    brightBlue: blue,
    brightMagenta: magenta,
    brightCyan: cyan,
    brightWhite: p.foreground,
  };
}

export class ThemeManager {
  readonly changed = new Emitter<Palette>();
  private readonly query = window.matchMedia('(prefers-color-scheme: dark)');

  constructor() {
    this.query.addEventListener('change', () => {
      this.applyCss();
      this.changed.emit(this.palette);
    });
    this.applyCss();
  }

  get palette(): Palette {
    return this.query.matches ? dark : light;
  }

  private applyCss(): void {
    const p = this.palette;
    const root = document.documentElement.style;
    const vars: Record<string, string> = {
      '--bg': p.background,
      '--chrome': p.chrome,
      '--line': p.line,
      '--fg': p.foreground,
      '--fg2': p.secondary,
      '--muted': p.muted,
      '--faint': p.faint,
      '--accent': p.accent,
      '--hover': p.hover,
      '--current-row': p.currentRow,
      '--current-row-text': p.currentRowText,
      '--menu': p.menu,
      '--menu-selected': p.menuSelected,
      '--tooltip': p.tooltip,
      '--tooltip-text': p.tooltipText,
      '--shadow': p.shadow,
      '--danger': p.ansi[1],
    };
    for (const [name, value] of Object.entries(vars)) root.setProperty(name, value);
    root.setProperty('color-scheme', this.query.matches ? 'dark' : 'light');
  }
}

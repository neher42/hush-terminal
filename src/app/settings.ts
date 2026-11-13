import { backend } from '../backend';

export type Appearance = 'system' | 'light' | 'dark';

export const FONTS = {
  'jetbrains-mono': { label: 'JetBrains Mono', css: '"JetBrains Mono", ui-monospace, Menlo, monospace' },
  'sf-mono': { label: 'SF Mono', css: 'ui-monospace, Menlo, monospace' },
  menlo: { label: 'Menlo', css: 'Menlo, monospace' },
} as const;
export type FontChoice = keyof typeof FONTS;

export interface Settings {
  fontSize: number;
  font: FontChoice;
  appearance: Appearance;
  sidebarVisible: boolean;
  /** Absolute paths, in the order they show in the sidebar. */
  pinnedFolders: string[];
}

export const DEFAULT_FONT_SIZE = 13.5;
export const FONT_SIZE_RANGE = { min: 9, max: 28 } as const;
const DEFAULTS: Settings = { fontSize: DEFAULT_FONT_SIZE, font: 'jetbrains-mono', appearance: 'system', sidebarVisible: true, pinnedFolders: [] };
const APPEARANCES: readonly Appearance[] = ['system', 'light', 'dark'];
const LEGACY_STORAGE_KEY = 'hush.settings';

export async function loadSettings(): Promise<Settings> {
  try {
    const stored = await backend.readSettings();
    if (stored !== null) return normalizeSettings(JSON.parse(stored));
  } catch (err) {
    console.error('Could not read settings, using defaults:', err);
    return { ...DEFAULTS, pinnedFolders: [] };
  }
  const migrated = normalizeSettings(readLegacySettings());
  saveSettings(migrated);
  return migrated;
}

export function saveSettings(settings: Settings): void {
  backend.writeSettings(JSON.stringify(settings, null, 2)).catch((err: unknown) => console.error('Could not save settings:', err));
}

export function normalizeSettings(raw: unknown): Settings {
  const value = typeof raw === 'object' && raw !== null ? (raw as Record<string, unknown>) : {};
  const pinned = Array.isArray(value.pinnedFolders) ? value.pinnedFolders : [];
  return {
    fontSize: typeof value.fontSize === 'number' && Number.isFinite(value.fontSize) ? clampFontSize(value.fontSize) : DEFAULTS.fontSize,
    font: typeof value.font === 'string' && value.font in FONTS ? (value.font as FontChoice) : DEFAULTS.font,
    appearance: APPEARANCES.includes(value.appearance as Appearance) ? (value.appearance as Appearance) : DEFAULTS.appearance,
    sidebarVisible: typeof value.sidebarVisible === 'boolean' ? value.sidebarVisible : DEFAULTS.sidebarVisible,
    pinnedFolders: [...new Set(pinned.filter((p): p is string => typeof p === 'string' && p.startsWith('/')))],
  };
}

export function clampFontSize(size: number): number {
  return Math.min(FONT_SIZE_RANGE.max, Math.max(FONT_SIZE_RANGE.min, Math.round(size * 2) / 2));
}

function readLegacySettings(): unknown {
  try {
    const raw = localStorage.getItem(LEGACY_STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

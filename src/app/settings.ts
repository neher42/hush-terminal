import { backend } from '../backend';

export interface Settings {
  fontSize: number;
  sidebarVisible: boolean;
  /** Absolute paths, in the order they show in the sidebar. */
  pinnedFolders: string[];
}

export const DEFAULT_FONT_SIZE = 13.5;
export const FONT_SIZE_RANGE = { min: 9, max: 28 } as const;
const DEFAULTS: Settings = { fontSize: DEFAULT_FONT_SIZE, sidebarVisible: true, pinnedFolders: [] };
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

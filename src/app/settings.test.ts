import { describe, expect, it } from 'vitest';
import { normalizeSettings } from './settings';

describe('normalizeSettings', () => {
  it('fills in defaults for anything missing or malformed', () => {
    expect(normalizeSettings(null)).toEqual({
      fontSize: 13.5,
      font: 'jetbrains-mono',
      appearance: 'system',
      sidebarVisible: true,
      pinnedFolders: [],
    });
    expect(normalizeSettings({ fontSize: 'big', appearance: 'blue', sidebarVisible: 1 })).toEqual({
      fontSize: 13.5,
      font: 'jetbrains-mono',
      appearance: 'system',
      sidebarVisible: true,
      pinnedFolders: [],
    });
    expect(normalizeSettings({ font: 'comic-sans' }).font).toBe('jetbrains-mono');
  });

  it('keeps valid values and clamps the font size', () => {
    expect(normalizeSettings({ fontSize: 40, font: 'menlo', appearance: 'dark', sidebarVisible: false })).toMatchObject({
      fontSize: 28,
      font: 'menlo',
      appearance: 'dark',
      sidebarVisible: false,
    });
  });

  it('keeps only absolute, unique pinned paths in order', () => {
    expect(normalizeSettings({ pinnedFolders: ['/a', 'rel', 3, '/b', '/a'] }).pinnedFolders).toEqual(['/a', '/b']);
  });
});

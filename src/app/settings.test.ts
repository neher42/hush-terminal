import { describe, expect, it } from 'vitest';
import { normalizeSettings } from './settings';

describe('normalizeSettings', () => {
  it('fills in defaults for anything missing or malformed', () => {
    expect(normalizeSettings(null)).toEqual({ fontSize: 13.5, sidebarVisible: true, pinnedFolders: [] });
    expect(normalizeSettings({ fontSize: 'big', sidebarVisible: 1 })).toEqual({ fontSize: 13.5, sidebarVisible: true, pinnedFolders: [] });
  });

  it('keeps valid values and clamps the font size', () => {
    expect(normalizeSettings({ fontSize: 40, sidebarVisible: false })).toMatchObject({ fontSize: 28, sidebarVisible: false });
  });

  it('keeps only absolute, unique pinned paths in order', () => {
    expect(normalizeSettings({ pinnedFolders: ['/a', 'rel', 3, '/b', '/a'] }).pinnedFolders).toEqual(['/a', '/b']);
  });
});

import { describe, expect, it } from 'vitest';
import { HistoryBrowser, HistoryStore } from './HistoryStore';

function store(...entries: string[]): HistoryStore {
  const s = new HistoryStore();
  s.load(entries);
  return s;
}

describe('HistoryStore', () => {
  it('suggests the newest continuation', () => {
    const s = store('git checkout main', 'git status', 'git checkout -b fix');
    expect(s.suggest('git ch')).toBe('git checkout -b fix');
    expect(s.suggest('git status')).toBeUndefined();
    expect(s.suggest('')).toBeUndefined();
  });

  it('skips entries the filter rejects, falling back to older ones', () => {
    const s = store('cd workspace', 'cd starliner');
    expect(s.suggest('cd ', (entry) => entry !== 'cd starliner')).toBe('cd workspace');
  });

  it('moves re-run commands to the newest position', () => {
    const s = store('ls', 'pwd');
    s.add('ls');
    expect(s.search('')).toEqual(['ls', 'pwd']);
  });

  it('keeps session commands newer than ones loaded later', () => {
    const s = new HistoryStore();
    s.add('npm test');
    s.load(['ls', 'npm test']);
    expect(s.search('')).toEqual(['npm test', 'ls']);
  });

  it('searches by substring first, then by subsequence', () => {
    const s = store('git checkout main', 'echo gco', 'grep -c o');
    expect(s.search('gco')).toEqual(['echo gco', 'grep -c o', 'git checkout main']);
  });
});

describe('HistoryBrowser', () => {
  it('walks matching entries and returns to what was typed', () => {
    const b = new HistoryBrowser(store('git pull', 'ls', 'git push'), 'git');
    expect(b.older()).toBe('git push');
    expect(b.older()).toBe('git pull');
    expect(b.older()).toBeUndefined();
    expect(b.newer()).toBe('git push');
    expect(b.newer()).toBe('git');
    expect(b.newer()).toBeUndefined();
  });
});

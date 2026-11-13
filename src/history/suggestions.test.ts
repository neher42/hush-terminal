import { describe, expect, it } from 'vitest';
import { cdTarget, fitsHere } from './suggestions';

describe('cdTarget', () => {
  it('finds the folder of a plain cd', () => {
    expect(cdTarget('cd starliner')).toBe('starliner');
    expect(cdTarget('cd ~/My\\ Docs/')).toBe('~/My Docs/');
    expect(cdTarget("cd 'two words'")).toBe('two words');
  });

  it('ignores everything else', () => {
    expect(cdTarget('cd')).toBeNull();
    expect(cdTarget('cd -')).toBeNull();
    expect(cdTarget('cd app && npm test')).toBeNull();
    expect(cdTarget('cd $(git rev-parse --show-toplevel)')).toBeNull();
    expect(cdTarget('ls starliner')).toBeNull();
  });
});

describe('fitsHere', () => {
  const folders = new Set(['/Users/me/starliner']);
  const place = (cwd: string) => ({ cwd, home: '/Users/me', isDir: (path: string) => folders.has(path) });

  it('only suggests cd into folders that exist from here', () => {
    expect(fitsHere('cd starliner', place('/Users/me'))).toBe(true);
    expect(fitsHere('cd starliner', place('/Users/me/workspace'))).toBe(false);
    expect(fitsHere('cd ~/starliner', place('/Users/me/workspace'))).toBe(true);
  });

  it('suggests other commands anywhere', () => {
    expect(fitsHere('git status', place('/tmp'))).toBe(true);
  });

  it('holds back a cd while its folder is still being checked', () => {
    expect(fitsHere('cd x', { cwd: '/', home: '/', isDir: () => undefined })).toBe(false);
  });
});

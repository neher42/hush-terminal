import { describe, expect, it } from 'vitest';
import { baseName, displayPath, escapeShellWord, parentPath, resolvePath, shellPath } from './paths';

const home = '/Users/me';

describe('paths', () => {
  it('shows home as a tilde', () => {
    expect(displayPath('/Users/me/src', home)).toBe('~/src');
    expect(displayPath('/Users/me', home)).toBe('~');
    expect(displayPath('/Users/meg', home)).toBe('/Users/meg');
  });

  it('resolves relative, home and parent paths', () => {
    expect(resolvePath('../b', '/Users/me/a', home)).toBe('/Users/me/b');
    expect(resolvePath('~/x/./y', '/tmp', home)).toBe('/Users/me/x/y');
    expect(resolvePath('/', '/tmp', home)).toBe('/');
  });

  it('escapes special characters for the shell', () => {
    expect(escapeShellWord('My Docs (old)')).toBe('My\\ Docs\\ \\(old\\)');
  });

  it('writes the shortest shell path', () => {
    expect(shellPath('/Users/me/src/app', '/Users/me/src', home)).toBe('app');
    expect(shellPath('/Users/me/Movies', '/Users/me/src', home)).toBe('~/Movies');
    expect(shellPath('/Users/me/src', '/Users/me/src', home)).toBe('.');
    expect(shellPath('/etc/hosts', '/Users/me', home)).toBe('/etc/hosts');
  });

  it('splits names and parents', () => {
    expect(baseName('/a/b/')).toBe('b');
    expect(parentPath('/a/b')).toBe('/a');
    expect(parentPath('/a')).toBe('/');
  });
});

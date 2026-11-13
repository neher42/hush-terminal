import { describe, expect, it } from 'vitest';
import { type KeyLike, resolveKey } from './keymap';

const key = (k: string, mods: Partial<Omit<KeyLike, 'key'>> = {}): KeyLike => ({
  key: k,
  metaKey: false,
  altKey: false,
  ctrlKey: false,
  shiftKey: false,
  ...mods,
});

describe('resolveKey', () => {
  it('leaves typing, copy and paste to the terminal', () => {
    expect(resolveKey(key('a'), false)).toBeNull();
    expect(resolveKey(key('c', { metaKey: true }), false)).toBeNull();
    expect(resolveKey(key('v', { metaKey: true }), false)).toBeNull();
  });

  it('uses macOS text shortcuts', () => {
    expect(resolveKey(key('Backspace', { metaKey: true }), false)).toEqual({ type: 'delete', unit: 'line', direction: -1 });
    expect(resolveKey(key('Backspace', { altKey: true }), false)).toEqual({ type: 'delete', unit: 'word', direction: -1 });
    expect(resolveKey(key('ArrowLeft', { altKey: true, shiftKey: true }), false)).toEqual({
      type: 'move',
      unit: 'word',
      direction: -1,
      extend: true,
    });
    expect(resolveKey(key('z', { metaKey: true, shiftKey: true }), false)).toEqual({ type: 'redo' });
  });

  it('leaves app menu shortcuts to the menu', () => {
    expect(resolveKey(key('a', { metaKey: true }), false)).toBeNull();
    expect(resolveKey(key('k', { metaKey: true }), false)).toBeNull();
    expect(resolveKey(key('ArrowLeft', { metaKey: true, altKey: true }), false)).toBeNull();
    expect(resolveKey(key('ArrowRight', { metaKey: true, altKey: true }), false)).toBeNull();
  });

  it('keeps shell control keys', () => {
    expect(resolveKey(key('a', { ctrlKey: true }), false)).toEqual({ type: 'move', unit: 'line', direction: -1, extend: false });
    expect(resolveKey(key('c', { ctrlKey: true }), false)).toEqual({ type: 'cancel' });
    expect(resolveKey(key('r', { ctrlKey: true }), false)).toEqual({ type: 'historySearch' });
    expect(resolveKey(key('w', { ctrlKey: true }), false)).toEqual({ type: 'deleteShellWord' });
  });

  it('submits on Enter and adds a line on Shift+Enter', () => {
    expect(resolveKey(key('Enter'), false)).toEqual({ type: 'submit' });
    expect(resolveKey(key('Enter', { shiftKey: true }), false)).toEqual({ type: 'newline' });
  });

  it('routes navigation keys to an open menu', () => {
    expect(resolveKey(key('ArrowDown'), true)).toEqual({ type: 'menu', action: 'next' });
    expect(resolveKey(key('Enter'), true)).toEqual({ type: 'menu', action: 'accept' });
    expect(resolveKey(key('Escape'), true)).toEqual({ type: 'menu', action: 'close' });
    expect(resolveKey(key('ArrowLeft'), true)).toEqual({ type: 'move', unit: 'char', direction: -1, extend: false });
  });
});

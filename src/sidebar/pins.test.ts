import { describe, expect, it } from 'vitest';
import { movePin, togglePin } from './pins';

describe('togglePin', () => {
  it('adds new pins at the end', () => {
    expect(togglePin(['/a'], '/b')).toEqual(['/a', '/b']);
  });

  it('removes an existing pin and keeps the order of the rest', () => {
    expect(togglePin(['/a', '/b', '/c'], '/b')).toEqual(['/a', '/c']);
  });

  it('does not change the list it was given', () => {
    const pinned = ['/a'];
    togglePin(pinned, '/b');
    expect(pinned).toEqual(['/a']);
  });
});

describe('movePin', () => {
  it('swaps with the neighbour', () => {
    expect(movePin(['/a', '/b', '/c'], 2, -1)).toEqual(['/a', '/c', '/b']);
    expect(movePin(['/a', '/b', '/c'], 0, 1)).toEqual(['/b', '/a', '/c']);
  });

  it('ignores moves past either end', () => {
    expect(movePin(['/a', '/b'], 0, -1)).toEqual(['/a', '/b']);
    expect(movePin(['/a', '/b'], 1, 1)).toEqual(['/a', '/b']);
  });
});

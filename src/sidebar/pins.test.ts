import { describe, expect, it } from 'vitest';
import { togglePin } from './pins';

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

import { describe, expect, it } from 'vitest';
import { layoutText, offsetAt } from './layout';

describe('layoutText', () => {
  it('places text after the prompt and wraps at the edge', () => {
    const layout = layoutText('abcdef', 2, 5);
    expect(layout.positions.slice(0, 7)).toEqual([
      { row: 0, col: 2 },
      { row: 0, col: 3 },
      { row: 0, col: 4 },
      { row: 1, col: 0 },
      { row: 1, col: 1 },
      { row: 1, col: 2 },
      { row: 1, col: 3 },
    ]);
  });

  it('moves the end to the next row when the text fills the last column', () => {
    expect(layoutText('abc', 2, 5).end).toEqual({ row: 1, col: 0 });
  });

  it('starts new lines at column zero', () => {
    const layout = layoutText('ab\ncd', 4, 10);
    expect(layout.positions[2]).toEqual({ row: 0, col: 6 });
    expect(layout.positions[3]).toEqual({ row: 1, col: 0 });
    expect(layout.end).toEqual({ row: 1, col: 2 });
  });

  it('wraps a wide character that does not fit', () => {
    const layout = layoutText('a漢', 0, 2);
    expect(layout.positions[1]).toEqual({ row: 1, col: 0 });
    expect(layout.end).toEqual({ row: 2, col: 0 });
  });

  it('gives both halves of a surrogate pair the same cell', () => {
    const layout = layoutText('😀x', 0, 10);
    expect(layout.positions[0]).toEqual(layout.positions[1]);
    expect(layout.positions[2]).toEqual({ row: 0, col: 2 });
  });
});

describe('offsetAt', () => {
  it('finds the character under a cell, or the end of the row', () => {
    const layout = layoutText('ab\ncdef', 2, 20);
    expect(offsetAt(layout, { row: 0, col: 3 })).toBe(1);
    expect(offsetAt(layout, { row: 0, col: 15 })).toBe(2);
    expect(offsetAt(layout, { row: 1, col: 2 })).toBe(5);
    expect(offsetAt(layout, { row: 5, col: 0 })).toBe(7);
  });
});

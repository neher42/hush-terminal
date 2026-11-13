import { describe, expect, it } from 'vitest';
import { isIncomplete } from './incomplete';

describe('isIncomplete', () => {
  it('detects unclosed quotes', () => {
    expect(isIncomplete('echo "hi')).toBe(true);
    expect(isIncomplete("echo 'it\"s")).toBe(true);
    expect(isIncomplete('echo "a \\" b"')).toBe(false);
  });

  it('detects continuation at the end of the line', () => {
    expect(isIncomplete('ls |')).toBe(true);
    expect(isIncomplete('make &&  ')).toBe(true);
    expect(isIncomplete('echo a \\')).toBe(true);
    expect(isIncomplete('echo a \\\\')).toBe(false);
  });

  it('ignores quotes inside comments', () => {
    expect(isIncomplete("ls # don't")).toBe(false);
  });

  it('accepts complete commands', () => {
    expect(isIncomplete('git commit -m "done"')).toBe(false);
    expect(isIncomplete('')).toBe(false);
  });
});

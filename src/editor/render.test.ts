import { describe, expect, it } from 'vitest';
import { renderCommandLine, zshHighlights } from './render';

const ESC = '\x1b[';

describe('renderCommandLine', () => {
  it('redraws from the start and places the cursor', () => {
    const out = renderCommandLine({
      text: 'ls -la',
      cursor: 2,
      spans: [
        { start: 0, end: 2, style: 'command' },
        { start: 3, end: 6, style: 'flag' },
      ],
      ghost: '',
      startCol: 10,
      cols: 80,
      from: { row: 0, col: 10 },
    });
    expect(out.sequence).toBe(`${ESC}11G${ESC}J${ESC}34mls${ESC}0m ${ESC}35m-la${ESC}0m${ESC}13G`);
    expect(out.cursor).toEqual({ row: 0, col: 12 });
  });

  it('moves up from a lower row before redrawing', () => {
    const out = renderCommandLine({ text: 'a\nb', cursor: 0, spans: [], ghost: '', startCol: 2, cols: 80, from: { row: 1, col: 1 } });
    expect(out.sequence.startsWith(`${ESC}1A${ESC}3G${ESC}J`)).toBe(true);
    expect(out.sequence).toContain('a\r\nb');
    expect(out.sequence.endsWith(`${ESC}1A${ESC}3G`)).toBe(true);
  });

  it('draws the suggestion dimmed after the text without moving the cursor past the text', () => {
    const out = renderCommandLine({ text: 'git ch', cursor: 6, spans: [], ghost: 'eckout', startCol: 0, cols: 80, from: { row: 0, col: 0 } });
    expect(out.sequence).toContain(`${ESC}90meckout${ESC}0m`);
    expect(out.cursor).toEqual({ row: 0, col: 6 });
    expect(out.sequence.endsWith(`${ESC}7G`)).toBe(true);
  });

  it('creates the next row when the text exactly fills the last column', () => {
    const out = renderCommandLine({ text: 'abc', cursor: 3, spans: [], ghost: '', startCol: 2, cols: 5, from: { row: 0, col: 2 } });
    expect(out.sequence).toContain('abc\r\n');
    expect(out.cursor).toEqual({ row: 1, col: 0 });
  });

  it('converts spans to zsh highlights in characters, not UTF-16 units', () => {
    const text = 'echo 😀 | grep x';
    const spans = [
      { start: 0, end: 4, style: 'command' as const },
      { start: 8, end: 9, style: 'operator' as const },
      { start: 10, end: 14, style: 'command' as const },
    ];
    expect(zshHighlights(text, spans)).toBe('0 4 fg=4\n7 8 fg=8\n9 13 fg=4');
  });

  it('shows control characters as a visible placeholder', () => {
    const out = renderCommandLine({ text: 'a\x07b', cursor: 3, spans: [], ghost: '', startCol: 0, cols: 80, from: { row: 0, col: 0 } });
    expect(out.sequence).not.toContain('\x07');
    expect(out.sequence).toContain('a␣b');
  });
});

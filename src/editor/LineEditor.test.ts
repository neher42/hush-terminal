import { describe, expect, it } from 'vitest';
import { LineEditor } from './LineEditor';

function editor(text: string, cursor = text.length): LineEditor {
  const e = new LineEditor();
  e.reset(text);
  e.moveTo(cursor);
  return e;
}

describe('LineEditor', () => {
  it('inserts at the cursor and replaces a selection', () => {
    const e = editor('git status', 3);
    e.insert('!');
    expect(e.text).toBe('git! status');
    e.select(5, 11);
    e.insert('log');
    expect(e.text).toBe('git! log');
    expect(e.cursor).toBe(8);
  });

  it('deletes by character, word and line', () => {
    const e = editor('git checkout -b fix-tests');
    e.delete('word', -1);
    expect(e.text).toBe('git checkout -b fix-');
    e.delete('char', -1);
    expect(e.text).toBe('git checkout -b fix');
    e.delete('line', -1);
    expect(e.text).toBe('');
  });

  it('deletes a shell word back to whitespace', () => {
    const e = editor('cd ~/workspace/food-planner');
    e.deleteShellWord();
    expect(e.text).toBe('cd ');
  });

  it('jumps words over punctuation like macOS', () => {
    const e = editor('npm run build:prod', 18);
    e.move('word', -1);
    expect(e.cursor).toBe(14);
    e.move('word', -1);
    expect(e.cursor).toBe(8);
    e.move('word', 1);
    expect(e.cursor).toBe(13);
  });

  it('extends and collapses selections', () => {
    const e = editor('hello world', 5);
    e.move('word', 1, true);
    expect(e.selection).toEqual({ start: 5, end: 11 });
    e.move('char', -1);
    expect(e.cursor).toBe(5);
    expect(e.selection).toBeNull();
  });

  it('moves between lines and reports the edges', () => {
    const e = editor('one\ntwo three\nfour', 6);
    expect(e.moveVertical(-1)).toBe(true);
    expect(e.cursor).toBe(2);
    expect(e.moveVertical(-1)).toBe(false);
    e.moveTo(16);
    expect(e.moveVertical(1)).toBe(false);
    expect(e.moveVertical(-1)).toBe(true);
    expect(e.cursor).toBe(6);
  });

  it('works on the current line for line operations', () => {
    const e = editor('echo one\necho two', 13);
    e.delete('line', -1);
    expect(e.text).toBe('echo one\n two');
  });

  it('undoes typing as one step and redoes it', () => {
    const e = editor('');
    for (const ch of 'git') e.insert(ch);
    e.insert(' ');
    for (const ch of 'log') e.insert(ch);
    e.undo();
    expect(e.text).toBe('git ');
    e.undo();
    expect(e.text).toBe('git');
    e.undo();
    expect(e.text).toBe('');
    e.redo();
    expect(e.text).toBe('git');
  });

  it('never splits a surrogate pair', () => {
    const e = editor('ok 👍');
    e.delete('char', -1);
    expect(e.text).toBe('ok ');
    const f = editor('👍', 0);
    f.move('char', 1);
    expect(f.cursor).toBe(2);
  });

  it('selects all and resets', () => {
    const e = editor('ls -la');
    e.selectAll();
    expect(e.selection).toEqual({ start: 0, end: 6 });
    e.reset();
    expect(e.isEmpty).toBe(true);
    expect(e.undo()).toBe(false);
  });
});

import { describe, expect, it } from 'vitest';
import { highlight, type Style } from './highlight';

const known = new Set(['git', 'ls', 'grep', 'sudo', 'echo', 'npm']);
const lookup = (name: string): boolean => known.has(name);

function styles(text: string, cursor = -1): Array<[string, Style]> {
  return highlight(text, lookup, cursor).map((s) => [text.slice(s.start, s.end), s.style]);
}

describe('highlight', () => {
  it('marks the command, flags, strings and variables', () => {
    expect(styles('git commit -m "fix it" $HOME')).toEqual([
      ['git', 'command'],
      ['-m', 'flag'],
      ['"fix it"', 'string'],
      ['$HOME', 'variable'],
    ]);
  });

  it('finds commands after pipes and prefix commands', () => {
    expect(styles('sudo ls | grep x && nope')).toEqual([
      ['sudo', 'command'],
      ['ls', 'command'],
      ['|', 'operator'],
      ['grep', 'command'],
      ['&&', 'operator'],
      ['nope', 'unknownCommand'],
    ]);
  });

  it('does not flag a command while it is still being typed', () => {
    expect(styles('gi', 2)).toEqual([['gi', 'command']]);
    expect(styles('gi x', 2)).toEqual([['gi', 'command']]);
    expect(styles('gi x', 4)).toEqual([['gi', 'unknownCommand']]);
  });

  it('treats unknown availability as known', () => {
    expect(highlight('anything', () => undefined).map((s) => s.style)).toEqual(['command']);
  });

  it('handles assignments, paths, comments and new lines', () => {
    expect(styles('FOO=1 ./run.sh # go\necho hi')).toEqual([
      ['FOO=1', 'variable'],
      ['./run.sh', 'command'],
      ['# go', 'comment'],
      ['echo', 'command'],
    ]);
  });
});

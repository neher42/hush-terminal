import { describe, expect, it } from 'vitest';
import type { DirEntry } from '../backend';
import { complete, commonPrefix, type CompletionSources } from './complete';
import { completionContext } from './context';

const tree: Record<string, DirEntry[]> = {
  '/home/me': [
    { name: 'workspace', isDir: true, isSymlink: false },
    { name: 'Work Notes', isDir: true, isSymlink: false },
    { name: '.zshrc', isDir: false, isSymlink: false },
    { name: 'todo.txt', isDir: false, isSymlink: false },
  ],
  '/home/me/workspace': [
    { name: 'food-planner', isDir: true, isSymlink: false },
    { name: 'fonts.zip', isDir: false, isSymlink: false },
  ],
};

const sources: CompletionSources = {
  cwd: '/home/me',
  home: '/home/me',
  listDir: async (path) => {
    const entries = tree[path];
    if (!entries) throw new Error('missing');
    return entries;
  },
  commands: async () => ['git', 'grep', 'gzip', 'ls'],
  gitBranches: async () => ['main', 'fix-tests'],
};

async function run(text: string, cursor = text.length) {
  return complete(completionContext(text, cursor), sources);
}

describe('completionContext', () => {
  it('finds the word at the cursor and earlier arguments', () => {
    expect(completionContext('git checkout ma', 15)).toEqual({ rawWord: 'ma', word: 'ma', wordStart: 13, args: ['git', 'checkout'] });
    expect(completionContext('ls | gr', 7)).toEqual({ rawWord: 'gr', word: 'gr', wordStart: 5, args: [] });
    expect(completionContext('cd ', 3)).toEqual({ rawWord: '', word: '', wordStart: 3, args: ['cd'] });
    expect(completionContext('cd Work\\ N', 10).word).toBe('Work N');
  });
});

describe('complete', () => {
  it('completes commands in command position', async () => {
    expect((await run('g')).map((i) => i.label)).toEqual(['git', 'grep', 'gzip']);
  });

  it('completes subcommands, flags and branches', async () => {
    expect((await run('git ch')).map((i) => i.label)).toEqual(['checkout', 'cherry-pick']);
    expect((await run('git commit --am')).map((i) => i.label)).toEqual(['--amend']);
    expect((await run('git checkout f')).map((i) => i.insert)).toEqual(['fix-tests']);
  });

  it('completes paths with escapes and folder slashes', async () => {
    const items = await run('cat W');
    expect(items.map((i) => [i.insert, i.appendSpace])).toEqual([['Work\\ Notes/', false]]);
    expect((await run('ls ~/workspace/fo')).map((i) => i.insert)).toEqual(['~/workspace/food-planner/', '~/workspace/fonts.zip']);
  });

  it('only offers folders to cd and hides dotfiles unless asked', async () => {
    expect((await run('cd ')).map((i) => i.label)).toEqual(['workspace/', 'Work Notes/']);
    expect((await run('cat .z')).map((i) => i.label)).toEqual(['.zshrc']);
  });

  it('falls back to case-insensitive matches', async () => {
    expect((await run('cd work')).map((i) => i.label)).toEqual(['workspace/']);
    expect((await run('cd WORK')).map((i) => i.label)).toEqual(['workspace/', 'Work Notes/']);
  });

  it('finds the shared prefix', async () => {
    expect(commonPrefix(await run('ls workspace/fo'))).toBe('workspace/fo');
    expect(commonPrefix(await run('git ch'))).toBe('che');
  });
});

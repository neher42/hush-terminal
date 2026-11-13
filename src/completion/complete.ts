import type { DirEntry } from '../backend';
import { escapeShellWord, expandTilde, resolvePath } from '../lib/paths';
import type { CompletionContext } from './context';
import { BRANCH_SUBCOMMANDS, FLAGS, FOLDER_COMMANDS, SUBCOMMANDS, type Suggestion } from './specs';

export type CompletionKind = 'command' | 'subcommand' | 'folder' | 'file' | 'branch' | 'flag';

export interface CompletionItem {
  insert: string;
  label: string;
  kind: CompletionKind;
  detail?: string;
  appendSpace: boolean;
  matched: number;
}

export interface CompletionSources {
  cwd: string;
  home: string;
  listDir(path: string): Promise<DirEntry[]>;
  commands(): Promise<string[]>;
  gitBranches(cwd: string): Promise<string[]>;
}

const MAX_ITEMS = 200;

export async function complete(ctx: CompletionContext, sources: CompletionSources): Promise<CompletionItem[]> {
  const { word, args } = ctx;
  const [command, sub] = args;

  if (command === undefined) {
    if (word.includes('/')) return completePath(ctx, sources, false);
    return byName(await sources.commands(), word).map((name) => item(name, 'command', word.length));
  }
  if (word.startsWith('-')) {
    const key = sub !== undefined && SUBCOMMANDS[command] ? `${command} ${sub}` : command;
    return fromSuggestions(FLAGS[key] ?? [], word, 'flag');
  }
  if (args.length === 1 && SUBCOMMANDS[command]) {
    return fromSuggestions(SUBCOMMANDS[command], word, 'subcommand');
  }
  if (command === 'git' && sub !== undefined && BRANCH_SUBCOMMANDS.has(sub) && !['-b', '-c'].includes(args[args.length - 1]!)) {
    const branches = byName(await sources.gitBranches(sources.cwd), word).map((name) => item(name, 'branch', word.length));
    if (branches.length > 0 || sub !== 'checkout') return branches;
  }
  return completePath(ctx, sources, FOLDER_COMMANDS.has(command));
}

async function completePath(ctx: CompletionContext, sources: CompletionSources, foldersOnly: boolean): Promise<CompletionItem[]> {
  const { word, rawWord } = ctx;
  const slash = word.lastIndexOf('/');
  const typedDir = slash >= 0 ? word.slice(0, slash + 1) : '';
  const prefix = word.slice(slash + 1);
  // Keep the folder part exactly as typed (with its escapes); only the name is rewritten.
  const rawSlash = rawWord.lastIndexOf('/');
  const rawDir = rawSlash >= 0 ? rawWord.slice(0, rawSlash + 1) : '';

  const dir = typedDir ? resolvePath(expandTilde(typedDir, sources.home), sources.cwd, sources.home) : sources.cwd;
  let entries: DirEntry[];
  try {
    entries = await sources.listDir(dir);
  } catch {
    return [];
  }
  const showHidden = prefix.startsWith('.');
  const candidates = entries.filter((e) => (showHidden || !e.name.startsWith('.')) && (!foldersOnly || e.isDir));
  return byName(candidates.map((e) => e.name), prefix).map((name) => {
    const entry = candidates.find((e) => e.name === name)!;
    return {
      insert: rawDir + escapeShellWord(name) + (entry.isDir ? '/' : ''),
      label: entry.isDir ? `${name}/` : name,
      kind: entry.isDir ? 'folder' : 'file',
      appendSpace: !entry.isDir,
      matched: prefix.length,
    };
  });
}

/** Names starting with `prefix`; falls back to ignoring case, as macOS file names do. */
function byName(names: string[], prefix: string): string[] {
  const exact = names.filter((n) => n.startsWith(prefix));
  const matches = exact.length > 0 ? exact : names.filter((n) => n.toLowerCase().startsWith(prefix.toLowerCase()));
  return matches.slice(0, MAX_ITEMS);
}

function fromSuggestions(list: Suggestion[], word: string, kind: CompletionKind): CompletionItem[] {
  const names = byName(list.map((x) => x.name), word);
  return names.map((name) => ({ ...item(name, kind, word.length), detail: list.find((x) => x.name === name)!.description }));
}

function item(name: string, kind: CompletionKind, matched: number): CompletionItem {
  return { insert: escapeShellWord(name), label: name, kind, appendSpace: true, matched };
}

export function commonPrefix(items: CompletionItem[]): string {
  if (items.length === 0) return '';
  let prefix = items[0]!.insert;
  for (const { insert } of items) {
    let i = 0;
    while (i < prefix.length && prefix[i] === insert[i]) i++;
    prefix = prefix.slice(0, i);
  }
  return prefix;
}

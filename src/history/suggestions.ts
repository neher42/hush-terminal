import { resolvePath } from '../lib/paths';

export function cdTarget(command: string): string | null {
  const match = /^\s*cd\s+((?:\\.|'[^']*'|"[^"$`]*"|[^\s;&|<>()`$'"])+)\s*$/.exec(command);
  if (!match) return null;
  const raw = match[1]!;
  if (raw === '-') return null;
  // Remove the shell's quoting: backslash escapes and the quote marks themselves.
  return raw.replace(/\\(.)|'([^']*)'|"([^"]*)"/g, (_, escaped, single, double) => escaped ?? single ?? double ?? '');
}

export interface Place {
  cwd: string;
  home: string;
  /** True or false when known; undefined while still being checked. */
  isDir(path: string): boolean | undefined;
}

export function fitsHere(entry: string, place: Place): boolean {
  const target = cdTarget(entry);
  if (target === null) return true;
  return place.isDir(resolvePath(target, place.cwd, place.home)) === true;
}

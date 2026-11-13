export function joinPath(dir: string, name: string): string {
  return dir.endsWith('/') ? dir + name : `${dir}/${name}`;
}

export function parentPath(path: string): string {
  if (path === '/') return '/';
  const i = path.replace(/\/+$/, '').lastIndexOf('/');
  return i <= 0 ? '/' : path.slice(0, i);
}

export function baseName(path: string): string {
  if (path === '/') return '/';
  return path.replace(/\/+$/, '').split('/').pop() ?? path;
}

export function isInside(path: string, dir: string): boolean {
  if (dir === '/') return path.startsWith('/');
  return path === dir || path.startsWith(dir.endsWith('/') ? dir : `${dir}/`);
}

export function displayPath(path: string, home: string): string {
  if (!home || !isInside(path, home)) return path;
  return path === home ? '~' : `~${path.slice(home.length)}`;
}

export function expandTilde(path: string, home: string): string {
  if (path === '~') return home;
  if (path.startsWith('~/')) return joinPath(home, path.slice(2));
  return path;
}

export function resolvePath(path: string, cwd: string, home: string): string {
  const expanded = expandTilde(path, home);
  const absolute = expanded.startsWith('/') ? expanded : joinPath(cwd, expanded);
  const parts: string[] = [];
  for (const part of absolute.split('/')) {
    if (!part || part === '.') continue;
    if (part === '..') parts.pop();
    else parts.push(part);
  }
  return `/${parts.join('/')}`;
}

const SHELL_SPECIAL = /[\s'"\\$`!&*()|;<>?[\]{}#^,=]/g;

export function escapeShellWord(word: string): string {
  return word.replace(SHELL_SPECIAL, (ch) => `\\${ch}`);
}

// Relative inside cwd, ~/... inside home, absolute otherwise.
export function shellPath(path: string, cwd: string, home: string): string {
  if (path === cwd) return '.';
  if (isInside(path, cwd) && cwd !== '/') return escapeShellWord(path.slice(cwd.length + 1));
  if (home && isInside(path, home)) {
    return path === home ? '~' : `~/${escapeShellWord(path.slice(home.length + 1))}`;
  }
  return escapeShellWord(path);
}

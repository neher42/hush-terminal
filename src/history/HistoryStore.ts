const MAX_ENTRIES = 20_000;

export class HistoryStore {
  /** Oldest first, no duplicates. */
  private entries: string[] = [];

  get size(): number {
    return this.entries.length;
  }

  load(fromFile: string[]): void {
    const session = new Set(this.entries);
    const older = fromFile.filter((entry) => !session.has(entry));
    this.entries = [...older, ...this.entries].slice(-MAX_ENTRIES);
  }

  add(command: string): void {
    const entry = command.trimEnd();
    if (!entry.trim()) return;
    const existing = this.entries.lastIndexOf(entry);
    if (existing >= 0) this.entries.splice(existing, 1);
    this.entries.push(entry);
    if (this.entries.length > MAX_ENTRIES) this.entries.shift();
  }

  suggest(prefix: string, accept: (entry: string) => boolean = () => true): string | undefined {
    if (!prefix.trim() || prefix.includes('\n')) return undefined;
    for (let i = this.entries.length - 1; i >= 0; i--) {
      const entry = this.entries[i]!;
      if (entry.length > prefix.length && entry.startsWith(prefix) && accept(entry)) return entry;
    }
    return undefined;
  }

  // Substring matches first, then subsequence matches ("gco" -> "git checkout").
  search(query: string, limit = 50): string[] {
    const q = query.trim().toLowerCase();
    const contains: string[] = [];
    const fuzzy: string[] = [];
    for (let i = this.entries.length - 1; i >= 0 && contains.length < limit; i--) {
      const entry = this.entries[i]!;
      const lower = entry.toLowerCase();
      if (!q || lower.includes(q)) contains.push(entry);
      else if (isSubsequence(q, lower)) fuzzy.push(entry);
    }
    return [...contains, ...fuzzy].slice(0, limit);
  }

  *walkBack(prefix: string): Generator<string> {
    for (let i = this.entries.length - 1; i >= 0; i--) {
      const entry = this.entries[i]!;
      if (entry.startsWith(prefix)) yield entry;
    }
  }
}

// Like zsh's up-line-or-beginning-search.
export class HistoryBrowser {
  private readonly matches: string[];
  private index = -1;

  constructor(
    store: HistoryStore,
    private readonly typed: string,
  ) {
    this.matches = [...store.walkBack(typed)].filter((entry) => entry !== typed);
  }

  older(): string | undefined {
    if (this.index + 1 >= this.matches.length) return undefined;
    this.index++;
    return this.matches[this.index];
  }

  newer(): string | undefined {
    if (this.index < 0) return undefined;
    this.index--;
    return this.index < 0 ? this.typed : this.matches[this.index];
  }
}

function isSubsequence(needle: string, haystack: string): boolean {
  let i = 0;
  for (const ch of haystack) {
    if (ch === needle[i]) i++;
    if (i === needle.length) return true;
  }
  return needle.length === 0;
}

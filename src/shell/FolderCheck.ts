import { Emitter } from '../lib/emitter';

export class FolderCheck {
  readonly changed = new Emitter<void>();
  private readonly known = new Map<string, boolean>();
  private readonly pending = new Set<string>();

  constructor(private readonly check: (path: string) => Promise<boolean>) {}

  /** True or false when known; undefined while the check is still running. */
  isDir(path: string): boolean | undefined {
    const known = this.known.get(path);
    if (known !== undefined || this.pending.has(path)) return known;
    this.pending.add(path);
    this.check(path)
      .catch(() => false)
      .then((isDir) => {
        this.pending.delete(path);
        this.known.set(path, isDir);
        this.changed.emit();
      });
    return undefined;
  }

  clear(): void {
    this.known.clear();
  }
}

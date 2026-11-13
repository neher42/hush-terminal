import { backend } from '../backend';
import { SHELL_BUILTINS } from '../completion/specs';

export class CommandCatalog {
  private executables: string[] | null = null;
  private shellNames: string[] = [];
  private known: Set<string> | null = null;
  private loading: Promise<void> | null = null;
  private pathVar = '';

  setPath(pathVar: string): void {
    if (pathVar === this.pathVar) return;
    this.pathVar = pathVar;
    const load = backend.listExecutables(pathVar).then(
      (names) => {
        if (this.pathVar !== pathVar) return;
        this.executables = names;
        this.rebuild();
      },
      (err: unknown) => console.error('Could not list commands:', err),
    );
    this.loading = load;
  }

  setShellNames(names: string[]): void {
    this.shellNames = names;
    this.rebuild();
  }

  /** Whether `name` is a command; undefined until the PATH has been read. */
  isKnown(name: string): boolean | undefined {
    return this.known ? this.known.has(name) : undefined;
  }

  async names(): Promise<string[]> {
    if (!this.known && this.loading) await this.loading;
    return this.known ? [...this.known].sort() : [...SHELL_BUILTINS];
  }

  private rebuild(): void {
    if (!this.executables) return;
    this.known = new Set([...this.executables, ...SHELL_BUILTINS, ...this.shellNames]);
  }
}

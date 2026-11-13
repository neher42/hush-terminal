import { Channel, invoke } from '@tauri-apps/api/core';

export interface DirEntry {
  name: string;
  isDir: boolean;
  isSymlink: boolean;
}

export type ClaudeDelivery = { result: 'pasted' } | { result: 'copied'; message: string };

export interface GitStatus {
  branch: string | null;
  commit: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  files: number;
  added: number;
  removed: number;
}

export const backend = {
  homeDir: () => invoke<string>('home_dir'),
  listDir: (path: string) => invoke<DirEntry[]>('list_dir', { path }),
  isDir: (path: string) => invoke<boolean>('is_dir', { path }),
  readHistory: (path: string) => invoke<string[]>('read_history', { path }),
  listExecutables: (pathVar: string) => invoke<string[]>('list_executables', { pathVar }),
  gitBranches: (cwd: string) => invoke<string[]>('git_branches', { cwd }),
  gitStatus: (cwd: string) => invoke<GitStatus | null>('git_status', { cwd }),
  sendToClaude: (text: string) => invoke<ClaudeDelivery>('send_to_claude', { text }),
  readSettings: () => invoke<string | null>('read_settings'),
  writeSettings: (json: string) => invoke<void>('write_settings', { json }),
  setAppearanceMenu: (mode: string) => invoke<void>('set_appearance_menu', { mode }),
  claudePermission: () => invoke<boolean>('claude_permission'),
  openClaudePermissionSettings: () => invoke<void>('open_claude_permission_settings'),
};

export interface PtyHandlers {
  onData(data: Uint8Array): void;
  onExit(code: number): void;
}

export class PtySession {
  private queue: Promise<void> = Promise.resolve();
  private pending = '';
  private closed = false;

  private constructor(readonly id: number) {}

  static async spawn(cols: number, rows: number, cwd: string | null, handlers: PtyHandlers): Promise<PtySession> {
    const onData = new Channel<ArrayBuffer | number[]>();
    onData.onmessage = (chunk) => handlers.onData(chunk instanceof ArrayBuffer ? new Uint8Array(chunk) : Uint8Array.from(chunk));
    const onExit = new Channel<number>();
    let session: PtySession | undefined;
    onExit.onmessage = (code) => {
      if (session) session.closed = true;
      handlers.onExit(code);
    };
    const id = await invoke<number>('pty_spawn', { cols, rows, cwd, onData, onExit });
    session = new PtySession(id);
    return session;
  }

  get isClosed(): boolean {
    return this.closed;
  }

  write(data: string): void {
    if (this.closed || !data) return;
    const flushNow = this.pending === '';
    this.pending += data;
    if (!flushNow) return;
    this.enqueue(() => {
      const chunk = this.pending;
      this.pending = '';
      return invoke('pty_write', { id: this.id, data: chunk });
    });
  }

  // `highlights` are zsh region_highlight entries ("start end fg=N").
  submit(command: string, highlights: string, action: 'run' | 'cancel'): void {
    if (this.closed) return;
    this.enqueue(() => invoke('pty_submit_command', { id: this.id, command, highlights, action }));
  }

  resize(cols: number, rows: number): void {
    if (this.closed) return;
    this.enqueue(() => invoke('pty_resize', { id: this.id, cols, rows }));
  }

  kill(): void {
    if (this.closed) return;
    this.enqueue(() => invoke('pty_kill', { id: this.id }));
  }

  private enqueue(task: () => Promise<unknown>): void {
    this.queue = this.queue.then(task).then(
      () => undefined,
      (err: unknown) => console.error('Terminal backend error:', err),
    );
  }
}

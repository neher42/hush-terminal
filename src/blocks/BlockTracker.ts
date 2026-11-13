import type { IMarker, Terminal } from '@xterm/xterm';
import type { ShellEvent } from '../shell/events';

export interface Block {
  readonly prompt: IMarker;
  output: IMarker | null;
  end: IMarker | null;
  endCol: number;
  command: string | null;
  exitCode: number | null;
}

export class BlockTracker {
  private blocks: Block[] = [];

  constructor(private readonly term: Terminal) {}

  handleShellEvent(event: ShellEvent): void {
    const current = this.blocks[this.blocks.length - 1];
    switch (event.type) {
      case 'promptStart': {
        // A prompt redrawn before anything ran (resize, clear screen) is the same block.
        if (current && current.command === null && current.output === null) this.remove(current);
        const marker = this.term.registerMarker(0);
        if (!marker) return;
        const block: Block = { prompt: marker, output: null, end: null, endCol: 0, command: null, exitCode: null };
        marker.onDispose(() => this.remove(block));
        this.blocks.push(block);
        return;
      }
      case 'commandStart':
        if (current && !current.output) current.output = this.term.registerMarker(0) ?? null;
        return;
      case 'commandEnd':
        if (current?.output && !current.end) {
          current.end = this.term.registerMarker(0) ?? null;
          current.endCol = this.term.buffer.normal.cursorX;
          current.exitCode = event.exitCode;
        }
        return;
      default:
        return;
    }
  }

  setCommand(command: string): void {
    const current = this.blocks[this.blocks.length - 1];
    if (current && current.command === null) current.command = command;
  }

  blockAt(line: number): Block | null {
    for (let i = this.blocks.length - 1; i >= 0; i--) {
      const block = this.blocks[i]!;
      if (block.prompt.line <= line) return block.command !== null && line <= this.lastLine(block) ? block : null;
    }
    return null;
  }

  lastLine(block: Block): number {
    const index = this.blocks.indexOf(block);
    const next = this.blocks[index + 1];
    if (next) return Math.max(block.prompt.line, next.prompt.line - 1);
    const buffer = this.term.buffer.normal;
    return block.end ? block.end.line : buffer.baseY + buffer.cursorY;
  }

  outputText(block: Block): string {
    if (!block.output) return '';
    const buffer = this.term.buffer.normal;
    const first = block.output.line;
    const last = block.end ? block.end.line : buffer.baseY + buffer.cursorY;
    const lastCol = block.end ? block.endCol : buffer.cursorX;
    let text = '';
    for (let y = first; y <= last; y++) {
      const line = buffer.getLine(y);
      if (!line) continue;
      if (y > first && !line.isWrapped) text += '\n';
      text += y === last ? line.translateToString(true, 0, lastCol) : line.translateToString(true);
    }
    return text.replace(/\s+$/, '');
  }

  private remove(block: Block): void {
    const i = this.blocks.indexOf(block);
    if (i < 0) return;
    this.blocks.splice(i, 1);
    if (!block.prompt.isDisposed) block.prompt.dispose();
    block.output?.dispose();
    block.end?.dispose();
  }
}

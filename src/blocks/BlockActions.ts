import type { IDecoration } from '@xterm/xterm';
import type { TerminalView } from '../terminal/TerminalView';
import { icons } from '../ui/icons';
import type { Block, BlockTracker } from './BlockTracker';

export interface BlockActionsDeps {
  view: TerminalView;
  tracker: BlockTracker;
  hoverColor(): string;
  copy(text: string, confirmation: string): void;
  runAgain(command: string): void;
  sendToClaude(text: string): void;
}

export class BlockActions {
  private readonly toolbar: HTMLElement;
  private readonly copyOutputButton: HTMLButtonElement;
  private block: Block | null = null;
  private decoration: IDecoration | undefined;
  private pointer: { x: number; y: number } | null = null;

  constructor(private readonly deps: BlockActionsDeps) {
    const { view } = deps;
    this.toolbar = document.createElement('div');
    this.toolbar.className = 'block-actions';
    this.toolbar.hidden = true;
    this.copyOutputButton = this.button(icons.copy, 'Copy output', (b) => deps.copy(deps.tracker.outputText(b), 'Copied output'));
    this.toolbar.append(
      this.copyOutputButton,
      this.button(icons.command, 'Copy command', (b) => deps.copy(b.command ?? '', 'Copied command')),
      this.button(icons.rerun, 'Run again', (b) => b.command && deps.runAgain(b.command)),
      this.button(icons.send, 'Send to Claude', (b) => deps.sendToClaude(this.transcript(b))),
    );
    view.container.appendChild(this.toolbar);

    view.container.addEventListener('mousemove', (e) => {
      this.pointer = { x: e.clientX, y: e.clientY };
      this.update();
    });
    view.container.addEventListener('mouseleave', () => {
      this.pointer = null;
      this.hide();
    });
    view.term.onScroll(() => this.update());
    view.term.onResize(() => this.hide());
  }

  private update(): void {
    const { view, tracker } = this.deps;
    const buffer = view.term.buffer.active;
    if (!this.pointer || buffer.type === 'alternate') return this.hide();
    if (this.toolbar.contains(document.elementFromPoint(this.pointer.x, this.pointer.y))) return;
    const cell = view.cellAt(this.pointer.x, this.pointer.y);
    const block = cell ? tracker.blockAt(cell.line) : null;
    if (!block) return this.hide();
    this.show(block);
  }

  private show(block: Block): void {
    const { view, tracker } = this.deps;
    const term = view.term;
    if (block !== this.block) {
      this.decoration?.dispose();
      this.block = block;
      this.decoration = term.registerDecoration({
        marker: block.prompt,
        width: term.cols,
        height: tracker.lastLine(block) - block.prompt.line + 1,
        backgroundColor: this.deps.hoverColor(),
        layer: 'bottom',
      });
    }
    this.copyOutputButton.hidden = !block.output;
    // Pin the toolbar to the block's first visible line.
    const line = Math.max(block.prompt.line, term.buffer.active.viewportY);
    const box = view.cellBox(line, 0);
    if (!box) return this.hide();
    this.toolbar.style.top = `${box.top}px`;
    this.toolbar.hidden = false;
  }

  private hide(): void {
    this.toolbar.hidden = true;
    this.decoration?.dispose();
    this.decoration = undefined;
    this.block = null;
  }

  private button(icon: string, label: string, run: (block: Block) => void): HTMLButtonElement {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'block-action';
    button.innerHTML = icon;
    button.setAttribute('aria-label', label);
    button.dataset.tip = label;
    // Keep keyboard focus (and any selection) in the terminal.
    button.addEventListener('mousedown', (e) => e.preventDefault());
    button.addEventListener('click', () => {
      if (this.block) run(this.block);
    });
    return button;
  }

  private transcript(block: Block): string {
    const output = this.deps.tracker.outputText(block);
    const status = block.exitCode ? `\n(exit code ${block.exitCode})` : '';
    return `\`\`\`\n$ ${block.command ?? ''}${output ? `\n${output}` : ''}\n\`\`\`${status}\n`;
  }
}

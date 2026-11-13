import type { IDisposable, Terminal } from '@xterm/xterm';
import { Emitter } from '../lib/emitter';
import { parseMeta, parsePromptMarker, parseWorkingDirectory, type ShellEvent } from './events';

const OSC_PROMPT = 133;
const OSC_CWD = 7;
const OSC_HUSH = 7777;

export class ShellIntegration implements IDisposable {
  readonly events = new Emitter<ShellEvent>();
  private readonly handlers: IDisposable[];

  constructor(term: Terminal) {
    // Handlers run during parsing, so the cursor is where the marker was emitted.
    const handle = (parse: (data: string) => ShellEvent | null) => (data: string) => {
      const event = parse(data);
      if (event) this.events.emit(event);
      return true;
    };
    this.handlers = [
      term.parser.registerOscHandler(OSC_PROMPT, handle(parsePromptMarker)),
      term.parser.registerOscHandler(OSC_CWD, handle(parseWorkingDirectory)),
      term.parser.registerOscHandler(OSC_HUSH, handle(parseMeta)),
    ];
  }

  dispose(): void {
    for (const handler of this.handlers) handler.dispose();
  }
}

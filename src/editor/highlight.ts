export type Style = 'plain' | 'command' | 'unknownCommand' | 'flag' | 'string' | 'variable' | 'operator' | 'comment';

export interface Span {
  start: number;
  end: number;
  style: Style;
}

/** True, false, or undefined when Hush doesn't know the available commands yet. */
export type CommandLookup = (name: string) => boolean | undefined;

const OPERATOR_CHARS = '|&;<>()';
/** Words after which the next word is still a command, e.g. `sudo git push`. */
const PREFIX_COMMANDS = new Set(['sudo', 'time', 'nohup', 'exec', 'command', 'builtin', 'env', 'nice', 'xargs', 'watch']);

export function highlight(text: string, isKnown: CommandLookup, cursor = -1): Span[] {
  const spans: Span[] = [];
  let i = 0;
  let commandPosition = true;
  const push = (start: number, end: number, style: Style): void => {
    if (end > start) spans.push({ start, end, style });
  };

  while (i < text.length) {
    const ch = text[i]!;

    if (/\s/.test(ch)) {
      if (ch === '\n') commandPosition = true;
      i++;
      continue;
    }

    if (ch === '#' && (i === 0 || /\s/.test(text[i - 1]!))) {
      const end = lineEnd(text, i);
      push(i, end, 'comment');
      i = end;
      continue;
    }

    if (OPERATOR_CHARS.includes(ch)) {
      let end = i;
      while (end < text.length && OPERATOR_CHARS.includes(text[end]!)) end++;
      const op = text.slice(i, end);
      push(i, end, 'operator');
      if (/[|&;(]/.test(op)) commandPosition = true;
      i = end;
      continue;
    }

    const end = wordEnd(text, i);
    const word = text.slice(i, end);

    if (commandPosition) {
      if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(word)) {
        push(i, end, 'variable');
      } else {
        const name = unquote(word);
        const known = isKnown(name) ?? true;
        const stillTyping = cursor === end;
        const isPath = name.includes('/');
        push(i, end, known || stillTyping || isPath ? 'command' : 'unknownCommand');
        commandPosition = PREFIX_COMMANDS.has(name);
      }
    } else {
      highlightArgument(word, i, push);
    }
    i = end;
  }
  return spans;
}

function highlightArgument(word: string, offset: number, push: (s: number, e: number, style: Style) => void): void {
  if (word.startsWith('-')) {
    push(offset, offset + word.length, 'flag');
    return;
  }
  // Mark quoted sections and $variables inside the word; the rest stays plain.
  let i = 0;
  while (i < word.length) {
    const ch = word[i]!;
    if (ch === '"' || ch === "'") {
      const close = word.indexOf(ch, i + 1);
      const end = close < 0 ? word.length : close + 1;
      push(offset + i, offset + end, 'string');
      i = end;
    } else if (ch === '$') {
      const match = /^\$(\{[^}]*\}?|[A-Za-z_][A-Za-z0-9_]*|[0-9?!#@*$-])/.exec(word.slice(i));
      const end = i + (match ? match[0].length : 1);
      push(offset + i, offset + end, 'variable');
      i = end;
    } else {
      i++;
    }
  }
}

function wordEnd(text: string, start: number): number {
  let i = start;
  while (i < text.length) {
    const ch = text[i]!;
    if (/\s/.test(ch) || OPERATOR_CHARS.includes(ch)) break;
    if (ch === '\\') {
      i += 2;
      continue;
    }
    if (ch === '"' || ch === "'") {
      const close = text.indexOf(ch, i + 1);
      i = close < 0 ? text.length : close + 1;
      continue;
    }
    i++;
  }
  return Math.min(i, text.length);
}

function lineEnd(text: string, start: number): number {
  const i = text.indexOf('\n', start);
  return i < 0 ? text.length : i;
}

export function unquote(word: string): string {
  return word.replace(/\\(.)/g, '$1').replace(/^(["'])(.*)\1$/, '$2');
}

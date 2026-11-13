export interface CompletionContext {
  rawWord: string;
  word: string;
  wordStart: number;
  args: string[];
}

export function completionContext(text: string, cursor: number): CompletionContext {
  const before = text.slice(0, cursor);
  const words = splitWords(before);
  const endsInSpace = before.length === 0 || /\s$/.test(before) || /[|;&(]$/.test(before);

  let segmentStart = 0;
  words.forEach((w, i) => {
    if (w.separator) segmentStart = i + 1;
  });
  const segment = words.slice(segmentStart).filter((w) => !w.separator);
  const current = endsInSpace ? undefined : segment[segment.length - 1];
  const args = (current ? segment.slice(0, -1) : segment).map((w) => w.value);

  return {
    rawWord: current?.raw ?? '',
    word: current?.value ?? '',
    wordStart: current ? current.start : cursor,
    args,
  };
}

interface Word {
  raw: string;
  value: string;
  start: number;
  separator: boolean;
}

function splitWords(text: string): Word[] {
  const words: Word[] = [];
  let i = 0;
  while (i < text.length) {
    const ch = text[i]!;
    if (ch === '\n' || ch === ';' || ch === '|' || ch === '&' || ch === '(') {
      let end = i + 1;
      while (end < text.length && '|&'.includes(text[end]!)) end++;
      words.push({ raw: text.slice(i, end), value: '', start: i, separator: true });
      i = end;
      continue;
    }
    if (/\s/.test(ch)) {
      i++;
      continue;
    }
    const start = i;
    let value = '';
    while (i < text.length && !/[\s;|&(]/.test(text[i]!)) {
      const c = text[i]!;
      if (c === '\\' && i + 1 < text.length) {
        value += text[i + 1];
        i += 2;
      } else if (c === '"' || c === "'") {
        const close = text.indexOf(c, i + 1);
        const end = close < 0 ? text.length : close;
        value += text.slice(i + 1, end);
        i = close < 0 ? text.length : close + 1;
      } else {
        value += c;
        i++;
      }
    }
    words.push({ raw: text.slice(start, i), value, start, separator: false });
  }
  return words;
}

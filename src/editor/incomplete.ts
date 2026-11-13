export function isIncomplete(text: string): boolean {
  let quote: string | null = null;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (quote) {
      if (ch === '\\' && quote === '"') i++;
      else if (ch === quote) quote = null;
      continue;
    }
    if (ch === '\\') i++;
    else if (ch === '"' || ch === "'") quote = ch;
    else if (ch === '#' && (i === 0 || /\s/.test(text[i - 1]!))) {
      const end = text.indexOf('\n', i);
      if (end < 0) break;
      i = end;
    }
  }
  if (quote) return true;
  const trimmed = text.replace(/[ \t]+$/, '');
  return /(?:^|[^\\])\\$/.test(trimmed) || /(\|\||&&|\|)$/.test(trimmed);
}

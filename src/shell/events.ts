export type ShellEvent =
  | { type: 'promptStart' }
  | { type: 'commandInputStart' }
  | { type: 'commandStart' }
  | { type: 'commandEnd'; exitCode: number | null }
  | { type: 'cwd'; path: string; host: string }
  | { type: 'meta'; key: string; value: string };

/** OSC 133 (semantic prompts, the FinalTerm protocol). */
export function parsePromptMarker(data: string): ShellEvent | null {
  const [kind, ...args] = data.split(';');
  switch (kind) {
    case 'A':
      return { type: 'promptStart' };
    case 'B':
      return { type: 'commandInputStart' };
    case 'C':
      return { type: 'commandStart' };
    case 'D': {
      const code = args[0] === undefined || args[0] === '' ? null : Number.parseInt(args[0], 10);
      return { type: 'commandEnd', exitCode: Number.isNaN(code) ? null : code };
    }
    default:
      return null;
  }
}

/** OSC 7: `file://host/percent-encoded/path`. */
export function parseWorkingDirectory(data: string): ShellEvent | null {
  const match = /^file:\/\/([^/]*)(\/.*)$/.exec(data);
  if (!match) return null;
  try {
    return { type: 'cwd', host: match[1] ?? '', path: decodeURIComponent(match[2]!) };
  } catch {
    return null;
  }
}

/** OSC 7777: `key=value` metadata specific to Hush. */
export function parseMeta(data: string): ShellEvent | null {
  const eq = data.indexOf('=');
  if (eq <= 0) return null;
  return { type: 'meta', key: data.slice(0, eq), value: data.slice(eq + 1) };
}

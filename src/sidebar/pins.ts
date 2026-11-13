export function togglePin(pinned: readonly string[], path: string): string[] {
  return pinned.includes(path) ? pinned.filter((p) => p !== path) : [...pinned, path];
}

/** Moves the pin at `index` up (-1) or down (1); out-of-range moves change nothing. */
export function movePin(pinned: readonly string[], index: number, delta: -1 | 1): string[] {
  const target = index + delta;
  if (index < 0 || index >= pinned.length || target < 0 || target >= pinned.length) return [...pinned];
  const next = [...pinned];
  [next[index], next[target]] = [next[target]!, next[index]!];
  return next;
}

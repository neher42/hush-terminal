export function togglePin(pinned: readonly string[], path: string): string[] {
  return pinned.includes(path) ? pinned.filter((p) => p !== path) : [...pinned, path];
}

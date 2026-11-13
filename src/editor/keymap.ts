import type { Direction, Unit } from './LineEditor';

export interface KeyLike {
  key: string;
  metaKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
}

export type EditorCommand =
  | { type: 'submit' }
  | { type: 'newline' }
  | { type: 'complete' }
  | { type: 'cancel' }
  | { type: 'clearScreen' }
  | { type: 'historySearch' }
  | { type: 'escape' }
  | { type: 'undo' }
  | { type: 'redo' }
  | { type: 'move'; unit: Unit; direction: Direction; extend: boolean }
  | { type: 'vertical'; direction: Direction; extend: boolean }
  | { type: 'delete'; unit: Unit; direction: Direction }
  | { type: 'deleteShellWord' }
  | { type: 'menu'; action: 'next' | 'previous' | 'accept' | 'close' };

// null = let xterm handle it. Shortcuts owned by the app menu (⌘A, ⌘K, ⌥⌘←/→)
// must stay null so the menu receives them.
export function resolveKey(e: KeyLike, menuOpen: boolean): EditorCommand | null {
  const { key, metaKey: cmd, altKey: alt, ctrlKey: ctrl, shiftKey: shift } = e;

  if (menuOpen) {
    if (key === 'ArrowDown' || (ctrl && key === 'n')) return { type: 'menu', action: 'next' };
    if (key === 'ArrowUp' || (ctrl && key === 'p') || (shift && key === 'Tab')) return { type: 'menu', action: 'previous' };
    if ((key === 'Tab' && !shift) || (key === 'Enter' && !shift)) return { type: 'menu', action: 'accept' };
    if (key === 'Escape' || (ctrl && (key === 'c' || key === 'g'))) return { type: 'menu', action: 'close' };
  }

  if (cmd && !ctrl && !alt) {
    switch (key) {
      case 'z':
      case 'Z':
        return shift ? { type: 'redo' } : { type: 'undo' };
      case 'Backspace':
        return { type: 'delete', unit: 'line', direction: -1 };
      case 'Delete':
        return { type: 'delete', unit: 'line', direction: 1 };
      case 'ArrowLeft':
        return { type: 'move', unit: 'line', direction: -1, extend: shift };
      case 'ArrowRight':
        return { type: 'move', unit: 'line', direction: 1, extend: shift };
      case 'ArrowUp':
        return { type: 'move', unit: 'all', direction: -1, extend: shift };
      case 'ArrowDown':
        return { type: 'move', unit: 'all', direction: 1, extend: shift };
      case 'Enter':
        return { type: 'newline' };
      default:
        return null;
    }
  }

  if (ctrl && !cmd && !alt) {
    switch (key.toLowerCase()) {
      case 'a':
        return { type: 'move', unit: 'line', direction: -1, extend: shift };
      case 'e':
        return { type: 'move', unit: 'line', direction: 1, extend: shift };
      case 'b':
        return { type: 'move', unit: 'char', direction: -1, extend: shift };
      case 'f':
        return { type: 'move', unit: 'char', direction: 1, extend: shift };
      case 'p':
        return { type: 'vertical', direction: -1, extend: false };
      case 'n':
        return { type: 'vertical', direction: 1, extend: false };
      case 'k':
        return { type: 'delete', unit: 'line', direction: 1 };
      case 'u':
        return { type: 'delete', unit: 'line', direction: -1 };
      case 'w':
        return { type: 'deleteShellWord' };
      case 'h':
        return { type: 'delete', unit: 'char', direction: -1 };
      case 'd':
        return { type: 'delete', unit: 'char', direction: 1 };
      case 'c':
        return { type: 'cancel' };
      case 'l':
        return { type: 'clearScreen' };
      case 'r':
        return { type: 'historySearch' };
      case 'j':
        return { type: 'newline' };
      default:
        return null;
    }
  }

  if (cmd || ctrl) return null;

  switch (key) {
    case 'Enter':
      return shift || alt ? { type: 'newline' } : { type: 'submit' };
    case 'Tab':
      return shift ? null : { type: 'complete' };
    case 'Escape':
      return { type: 'escape' };
    case 'Backspace':
      return { type: 'delete', unit: alt ? 'word' : 'char', direction: -1 };
    case 'Delete':
      return { type: 'delete', unit: alt ? 'word' : 'char', direction: 1 };
    case 'ArrowLeft':
      return { type: 'move', unit: alt ? 'word' : 'char', direction: -1, extend: shift };
    case 'ArrowRight':
      return { type: 'move', unit: alt ? 'word' : 'char', direction: 1, extend: shift };
    case 'ArrowUp':
      return { type: 'vertical', direction: -1, extend: shift };
    case 'ArrowDown':
      return { type: 'vertical', direction: 1, extend: shift };
    case 'Home':
      return { type: 'move', unit: 'line', direction: -1, extend: shift };
    case 'End':
      return { type: 'move', unit: 'line', direction: 1, extend: shift };
    default:
      return null;
  }
}

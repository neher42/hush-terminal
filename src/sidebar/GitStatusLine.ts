import type { GitStatus } from '../backend';
import { icons } from '../ui/icons';

/** One line under the current folder: branch, ahead/behind, and changed lines. */
export class GitStatusLine {
  readonly element: HTMLElement;

  constructor() {
    this.element = document.createElement('div');
    this.element.className = 'git-status';
    this.element.hidden = true;
  }

  render(status: GitStatus | null): void {
    this.element.hidden = status === null;
    if (!status) return;

    const branch = document.createElement('span');
    branch.className = 'git-branch';
    const icon = document.createElement('span');
    icon.className = 'git-icon';
    icon.innerHTML = icons.branch;
    const name = document.createElement('span');
    name.className = 'git-branch-name';
    name.textContent = status.branch ?? status.commit ?? 'no commits';
    branch.append(icon, name);
    if (status.ahead) branch.append(count(`↑${status.ahead}`, 'git-sync'));
    if (status.behind) branch.append(count(`↓${status.behind}`, 'git-sync'));

    const changes = document.createElement('span');
    changes.className = 'git-changes';
    if (status.files) {
      changes.append(count(`${status.files} ${status.files === 1 ? 'file' : 'files'}`, 'git-files'));
      if (status.added) changes.append(count(`+${status.added}`, 'git-added'));
      if (status.removed) changes.append(count(`−${status.removed}`, 'git-removed'));
    }

    this.element.replaceChildren(branch, changes);
    this.element.title = describe(status);
  }
}

function count(text: string, className: string): HTMLElement {
  const el = document.createElement('span');
  el.className = className;
  el.textContent = text;
  return el;
}

function describe(s: GitStatus): string {
  const parts = [s.branch ? `On ${s.branch}` : `Detached at ${s.commit ?? 'no commit'}`];
  if (s.upstream) {
    const sync = [s.ahead && plural(s.ahead, 'commit') + ' ahead', s.behind && plural(s.behind, 'commit') + ' behind'].filter(Boolean);
    parts.push(sync.length ? `${sync.join(', ')} of ${s.upstream}` : `up to date with ${s.upstream}`);
  }
  parts.push(s.files ? `${plural(s.files, 'changed file')}, ${s.added} lines added, ${s.removed} removed` : 'no changes');
  return parts.join(' · ');
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? '' : 's'}`;
}

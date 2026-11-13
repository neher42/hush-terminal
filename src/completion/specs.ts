export interface Suggestion {
  name: string;
  description: string;
}

const s = (name: string, description: string): Suggestion => ({ name, description });

export const SUBCOMMANDS: Record<string, Suggestion[]> = {
  git: [
    s('status', 'Show the working tree status'),
    s('add', 'Stage changes'),
    s('commit', 'Record staged changes'),
    s('diff', 'Show changes'),
    s('log', 'Show commit history'),
    s('checkout', 'Switch branches or restore files'),
    s('switch', 'Switch branches'),
    s('branch', 'List, create or delete branches'),
    s('pull', 'Fetch and integrate changes'),
    s('push', 'Update the remote'),
    s('fetch', 'Download from the remote'),
    s('merge', 'Join histories'),
    s('rebase', 'Reapply commits on top of another base'),
    s('stash', 'Set changes aside'),
    s('restore', 'Restore files'),
    s('reset', 'Reset HEAD'),
    s('clone', 'Clone a repository'),
    s('remote', 'Manage remotes'),
    s('tag', 'Manage tags'),
    s('show', 'Show an object'),
    s('cherry-pick', 'Apply a commit'),
  ],
  npm: [
    s('run', 'Run a package script'),
    s('install', 'Install dependencies'),
    s('test', 'Run the test script'),
    s('start', 'Run the start script'),
    s('ci', 'Clean install'),
    s('outdated', 'List outdated packages'),
    s('update', 'Update packages'),
    s('uninstall', 'Remove a package'),
    s('publish', 'Publish the package'),
  ],
  pnpm: [s('install', 'Install dependencies'), s('add', 'Add a package'), s('run', 'Run a script'), s('test', 'Run tests'), s('dev', 'Run the dev script')],
  yarn: [s('install', 'Install dependencies'), s('add', 'Add a package'), s('run', 'Run a script'), s('test', 'Run tests')],
  cargo: [s('build', 'Compile the package'), s('run', 'Build and run'), s('test', 'Run tests'), s('check', 'Check for errors'), s('clippy', 'Run lints'), s('fmt', 'Format the code'), s('add', 'Add a dependency')],
  brew: [s('install', 'Install a formula'), s('upgrade', 'Upgrade formulae'), s('update', 'Update Homebrew'), s('search', 'Search formulae'), s('list', 'List installed formulae'), s('uninstall', 'Remove a formula'), s('info', 'Show formula details')],
  docker: [s('ps', 'List containers'), s('run', 'Run a container'), s('build', 'Build an image'), s('compose', 'Run multi-container apps'), s('exec', 'Run a command in a container'), s('logs', 'Show container logs'), s('images', 'List images'), s('stop', 'Stop containers')],
};

export const FLAGS: Record<string, Suggestion[]> = {
  ls: [s('-l', 'Long listing'), s('-a', 'Include hidden files'), s('-la', 'Long listing with hidden files'), s('-h', 'Human-readable sizes'), s('-t', 'Sort by time')],
  'git commit': [s('-m', 'Message'), s('-a', 'Stage tracked files'), s('--amend', 'Change the last commit')],
  'git checkout': [s('-b', 'Create a branch')],
  'git switch': [s('-c', 'Create a branch')],
  'git log': [s('--oneline', 'One line per commit'), s('--graph', 'Draw the history graph')],
  'git push': [s('-u', 'Set upstream'), s('--force-with-lease', 'Force safely')],
  'git branch': [s('-d', 'Delete a merged branch'), s('-D', 'Delete a branch'), s('-a', 'Include remote branches')],
  grep: [s('-i', 'Ignore case'), s('-n', 'Show line numbers'), s('-r', 'Search folders'), s('-v', 'Invert match')],
  rm: [s('-r', 'Remove folders'), s('-f', 'Never prompt'), s('-rf', 'Remove folders without prompting')],
  mkdir: [s('-p', 'Create parent folders')],
  cp: [s('-r', 'Copy folders')],
};

export const FOLDER_COMMANDS = new Set(['cd', 'pushd', 'rmdir']);

export const BRANCH_SUBCOMMANDS = new Set(['checkout', 'switch', 'merge', 'rebase']);

export const SHELL_BUILTINS = [
  'alias', 'autoload', 'bg', 'bindkey', 'break', 'builtin', 'case', 'cd', 'command', 'continue', 'declare', 'dirs', 'disown',
  'do', 'done', 'echo', 'elif', 'else', 'emulate', 'esac', 'eval', 'exec', 'exit', 'export', 'false', 'fc', 'fg', 'fi', 'for',
  'function', 'functions', 'getopts', 'hash', 'history', 'if', 'jobs', 'kill', 'let', 'local', 'popd', 'print', 'printf',
  'pushd', 'pwd', 'read', 'readonly', 'rehash', 'return', 'select', 'set', 'setopt', 'shift', 'source', 'test', 'then', 'time',
  'trap', 'true', 'type', 'typeset', 'ulimit', 'umask', 'unalias', 'unset', 'unsetopt', 'until', 'wait', 'whence', 'where',
  'which', 'while', 'zle', 'zmodload',
];

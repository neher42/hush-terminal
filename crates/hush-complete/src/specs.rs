//! Built-in knowledge about common commands: subcommands, flags and argument kinds.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suggestion {
    pub name: &'static str,
    pub description: &'static str,
}

const fn s(name: &'static str, description: &'static str) -> Suggestion {
    Suggestion { name, description }
}

pub fn subcommands(command: &str) -> Option<&'static [Suggestion]> {
    Some(match command {
        "git" => GIT,
        "npm" => NPM,
        "pnpm" => PNPM,
        "yarn" => YARN,
        "cargo" => CARGO,
        "brew" => BREW,
        "docker" => DOCKER,
        _ => return None,
    })
}

/// Flags for a command, or for `"command subcommand"` when the command has subcommands.
pub fn flags(key: &str) -> Option<&'static [Suggestion]> {
    Some(match key {
        "ls" => LS_FLAGS,
        "git commit" => GIT_COMMIT_FLAGS,
        "git checkout" => GIT_CHECKOUT_FLAGS,
        "git switch" => GIT_SWITCH_FLAGS,
        "git log" => GIT_LOG_FLAGS,
        "git push" => GIT_PUSH_FLAGS,
        "git branch" => GIT_BRANCH_FLAGS,
        "grep" => GREP_FLAGS,
        "rm" => RM_FLAGS,
        "mkdir" => MKDIR_FLAGS,
        "cp" => CP_FLAGS,
        _ => return None,
    })
}

/// Commands whose arguments are folders.
pub const FOLDER_COMMANDS: &[&str] = &["cd", "pushd", "rmdir"];

/// Git subcommands whose argument is a branch.
pub const BRANCH_SUBCOMMANDS: &[&str] = &["checkout", "switch", "merge", "rebase"];

pub const SHELL_BUILTINS: &[&str] = &[
    "alias",
    "autoload",
    "bg",
    "bindkey",
    "break",
    "builtin",
    "case",
    "cd",
    "command",
    "continue",
    "declare",
    "dirs",
    "disown",
    "do",
    "done",
    "echo",
    "elif",
    "else",
    "emulate",
    "esac",
    "eval",
    "exec",
    "exit",
    "export",
    "false",
    "fc",
    "fg",
    "fi",
    "for",
    "function",
    "functions",
    "getopts",
    "hash",
    "history",
    "if",
    "jobs",
    "kill",
    "let",
    "local",
    "popd",
    "print",
    "printf",
    "pushd",
    "pwd",
    "read",
    "readonly",
    "rehash",
    "return",
    "select",
    "set",
    "setopt",
    "shift",
    "source",
    "test",
    "then",
    "time",
    "trap",
    "true",
    "type",
    "typeset",
    "ulimit",
    "umask",
    "unalias",
    "unset",
    "unsetopt",
    "until",
    "wait",
    "whence",
    "where",
    "which",
    "while",
    "zle",
    "zmodload",
];

const GIT: &[Suggestion] = &[
    s("status", "Show the working tree status"),
    s("add", "Stage changes"),
    s("commit", "Record staged changes"),
    s("diff", "Show changes"),
    s("log", "Show commit history"),
    s("checkout", "Switch branches or restore files"),
    s("switch", "Switch branches"),
    s("branch", "List, create or delete branches"),
    s("pull", "Fetch and integrate changes"),
    s("push", "Update the remote"),
    s("fetch", "Download from the remote"),
    s("merge", "Join histories"),
    s("rebase", "Reapply commits on top of another base"),
    s("stash", "Set changes aside"),
    s("restore", "Restore files"),
    s("reset", "Reset HEAD"),
    s("clone", "Clone a repository"),
    s("remote", "Manage remotes"),
    s("tag", "Manage tags"),
    s("show", "Show an object"),
    s("cherry-pick", "Apply a commit"),
];

const NPM: &[Suggestion] = &[
    s("run", "Run a package script"),
    s("install", "Install dependencies"),
    s("test", "Run the test script"),
    s("start", "Run the start script"),
    s("ci", "Clean install"),
    s("outdated", "List outdated packages"),
    s("update", "Update packages"),
    s("uninstall", "Remove a package"),
    s("publish", "Publish the package"),
];

const PNPM: &[Suggestion] = &[
    s("install", "Install dependencies"),
    s("add", "Add a package"),
    s("run", "Run a script"),
    s("test", "Run tests"),
    s("dev", "Run the dev script"),
];

const YARN: &[Suggestion] = &[
    s("install", "Install dependencies"),
    s("add", "Add a package"),
    s("run", "Run a script"),
    s("test", "Run tests"),
];

const CARGO: &[Suggestion] = &[
    s("build", "Compile the package"),
    s("run", "Build and run"),
    s("test", "Run tests"),
    s("check", "Check for errors"),
    s("clippy", "Run lints"),
    s("fmt", "Format the code"),
    s("add", "Add a dependency"),
];

const BREW: &[Suggestion] = &[
    s("install", "Install a formula"),
    s("upgrade", "Upgrade formulae"),
    s("update", "Update Homebrew"),
    s("search", "Search formulae"),
    s("list", "List installed formulae"),
    s("uninstall", "Remove a formula"),
    s("info", "Show formula details"),
];

const DOCKER: &[Suggestion] = &[
    s("ps", "List containers"),
    s("run", "Run a container"),
    s("build", "Build an image"),
    s("compose", "Run multi-container apps"),
    s("exec", "Run a command in a container"),
    s("logs", "Show container logs"),
    s("images", "List images"),
    s("stop", "Stop containers"),
];

const LS_FLAGS: &[Suggestion] = &[
    s("-l", "Long listing"),
    s("-a", "Include hidden files"),
    s("-la", "Long listing with hidden files"),
    s("-h", "Human-readable sizes"),
    s("-t", "Sort by time"),
];

const GIT_COMMIT_FLAGS: &[Suggestion] = &[
    s("-m", "Message"),
    s("-a", "Stage tracked files"),
    s("--amend", "Change the last commit"),
];

const GIT_CHECKOUT_FLAGS: &[Suggestion] = &[s("-b", "Create a branch")];

const GIT_SWITCH_FLAGS: &[Suggestion] = &[s("-c", "Create a branch")];

const GIT_LOG_FLAGS: &[Suggestion] = &[
    s("--oneline", "One line per commit"),
    s("--graph", "Draw the history graph"),
];

const GIT_PUSH_FLAGS: &[Suggestion] = &[
    s("-u", "Set upstream"),
    s("--force-with-lease", "Force safely"),
];

const GIT_BRANCH_FLAGS: &[Suggestion] = &[
    s("-d", "Delete a merged branch"),
    s("-D", "Delete a branch"),
    s("-a", "Include remote branches"),
];

const GREP_FLAGS: &[Suggestion] = &[
    s("-i", "Ignore case"),
    s("-n", "Show line numbers"),
    s("-r", "Search folders"),
    s("-v", "Invert match"),
];

const RM_FLAGS: &[Suggestion] = &[
    s("-r", "Remove folders"),
    s("-f", "Never prompt"),
    s("-rf", "Remove folders without prompting"),
];

const MKDIR_FLAGS: &[Suggestion] = &[s("-p", "Create parent folders")];

const CP_FLAGS: &[Suggestion] = &[s("-r", "Copy folders")];

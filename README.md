# Hush

A macOS terminal with an editor-like command line, built with Tauri and xterm.js.

- Edit commands with normal macOS shortcuts (⌘←/→, ⌥←/→, ⌘⌫, shift-select, ⌘Z)
- One selectable surface: select across output and the command you're typing
- History suggestions (→ to accept), tab completion, syntax highlighting
- Tabs (⌘T, ⌘W, ⌥⌘←/→, ⌘1-9)
- Sidebar with the current folder, pinned folders and a folder tree
- Per-command actions on hover: copy output, copy command, re-run, send to Claude
- `TERM=xterm-256color`, so ssh works everywhere

## Building

Requires Node and Rust.

```sh
npm install
npm run app:dev     # development
npm run app:build   # Hush.app and .dmg in src-tauri/target/release/bundle
```

Tests: `npm test` and `cargo test` (in `src-tauri`).

## Releasing

The version lives in `package.json`. To publish a release:

```sh
npm version 0.2.0        # updates package.json, commits and tags v0.2.0
git push --follow-tags
```

The release workflow builds the `.dmg` on GitHub and attaches it to the release.

## How it works

Hush runs your normal zsh. A small integration script (`src-tauri/shell/zsh/hush.zsh`)
is loaded after your own startup files and marks prompts with OSC 133 escape
sequences. While zsh waits at a prompt, Hush handles the keyboard itself and draws the
command into the terminal grid. On Enter it writes the command to a per-session file
and sends a private key sequence; a ZLE widget loads it into `BUFFER` and runs it.
While a command runs, input goes straight to it, so vim, htop and ssh behave as usual.

Other shells work too, without the editor features.

## Notes

- Send to Claude needs the Accessibility permission. It looks for the message box in
  the Claude app and only pastes when it's sure; otherwise the text is just copied.
  `cargo test claude_dry_run -- --ignored --nocapture` checks this against a running
  Claude app without changing anything.
- Keys typed before the next prompt appears go to zsh, not to Hush's editor.
- The folder tree only shows local folders.

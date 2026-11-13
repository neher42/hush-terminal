//! The command line: edited in Hush, drawn into the terminal grid right after the
//! prompt, and handed to zsh when it runs.

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Boundary, Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use hush_complete::{
    common_prefix, complete, completion_context, fits_here, CommandCatalog, FolderCheck,
    HistoryBrowser, HistoryStore, Place, SystemSources,
};
use hush_core::pty::CommandAction;
use hush_editor::{
    highlight, is_incomplete, move_to_command_start, render_command_line, zsh_highlights, Cell,
    Direction, EditorCommand, Layout, LineEditor, RenderInput, Unit,
};

use crate::terminal::Terminal;

/// What the editor needs from the rest of the app.
pub struct Env<'a> {
    pub terminal: &'a mut Terminal,
    pub history: &'a HistoryStore,
    pub catalog: &'a CommandCatalog,
    pub folders: &'a FolderCheck,
    pub place: Place<'a>,
}

#[derive(Default)]
pub struct PromptEditor {
    editor: LineEditor,
    active: bool,
    start_col: usize,
    /// Terminal cursor position relative to the start of the command.
    at: Cell,
    drawn: bool,
    layout: Option<Layout>,
    ghost: String,
    browser: Option<HistoryBrowser>,
    /// Restored at the next prompt after `run_command` replaced what was typed.
    stash: Option<String>,
    owns_selection: bool,
}

impl PromptEditor {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// zsh printed its prompt and waits for a command (OSC 133;B).
    pub fn begin_input(&mut self, cursor: Point, env: &mut Env) {
        if env
            .terminal
            .term
            .lock()
            .mode()
            .contains(TermMode::ALT_SCREEN)
        {
            return;
        }
        self.start_col = cursor.column.0;
        self.at = Cell {
            row: 0,
            col: self.start_col,
        };
        self.drawn = false;
        // A redrawn prompt (resize, ^L) keeps the current command.
        if !self.active {
            self.editor.reset(&self.stash.take().unwrap_or_default());
            self.browser = None;
        }
        self.active = true;
        self.render(env);
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.drawn = false;
    }

    /// Runs a key command. Returns the command line if one was started.
    pub fn execute(&mut self, command: EditorCommand, env: &mut Env) -> Option<String> {
        if !matches!(command, EditorCommand::Vertical { .. }) {
            self.browser = None;
        }
        let e = &mut self.editor;
        match command {
            EditorCommand::Submit if is_incomplete(e.text()) => e.insert("\n"),
            EditorCommand::Submit => {
                let text = e.text().to_owned();
                return self.finish(&text, CommandAction::Run, env);
            }
            EditorCommand::Newline => e.insert("\n"),
            EditorCommand::Cancel => {
                let text = e.text().to_owned();
                return self.finish(&text, CommandAction::Cancel, env);
            }
            EditorCommand::Complete => self.complete(env),
            EditorCommand::ClearScreen => self.clear_screen(env),
            EditorCommand::Escape => {
                e.clear_selection();
                env.terminal.term.lock().selection = None;
                self.owns_selection = false;
            }
            EditorCommand::Undo => {
                e.undo();
            }
            EditorCommand::Redo => {
                e.redo();
            }
            EditorCommand::Move {
                unit,
                direction,
                extend,
            } => {
                if direction == Direction::Forward
                    && !extend
                    && e.cursor_at_end()
                    && !self.ghost.is_empty()
                {
                    self.accept_ghost(unit == Unit::Word);
                } else {
                    self.clear_foreign_selection(env);
                    self.editor.move_by(unit, direction, extend);
                }
            }
            EditorCommand::Vertical { direction, extend } => {
                self.move_vertical(direction, extend, env)
            }
            EditorCommand::Delete { unit, direction } => {
                self.clear_foreign_selection(env);
                self.editor.delete(unit, direction);
            }
            EditorCommand::DeleteShellWord => {
                e.delete_shell_word();
            }
            // The history search and completion menus come with the menu UI.
            EditorCommand::HistorySearch | EditorCommand::Menu(_) => {}
        }
        self.render(env);
        None
    }

    /// Typed or pasted text.
    pub fn insert(&mut self, text: &str, env: &mut Env) {
        let text: String = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .chars()
            .filter(|&c| c == '\n' || c == '\t' || !c.is_control())
            .collect();
        if text.is_empty() {
            return;
        }
        self.clear_foreign_selection(env);
        self.editor.insert(&text);
        self.browser = None;
        self.render(env);
    }

    /// Replaces what was typed with `command` and runs it.
    pub fn run_command(&mut self, command: &str, env: &mut Env) -> Option<String> {
        if !self.active {
            return None;
        }
        if !self.editor.text().trim().is_empty() {
            self.stash = Some(self.editor.text().to_owned());
        }
        self.finish(command, CommandAction::Run, env)
    }

    /// First ⌘A selects the command; returns false when the whole buffer should be.
    pub fn select_all(&mut self, env: &mut Env) -> bool {
        let e = &mut self.editor;
        let everything = e.selection() == Some(0..e.text().len());
        if !self.active || e.is_empty() || everything {
            e.clear_selection();
            self.render(env);
            self.owns_selection = false;
            return false;
        }
        e.select_all();
        self.render(env);
        true
    }

    /// Removes the selected text and returns it for the clipboard.
    pub fn cut(&mut self, env: &mut Env) -> Option<String> {
        let range = self.editor.selection().filter(|_| self.active)?;
        let text = self.editor.text()[range].to_owned();
        self.editor.delete(Unit::Char, Direction::Backward);
        self.render(env);
        Some(text)
    }

    pub fn clear_screen(&mut self, env: &mut Env) {
        let terminal = &mut *env.terminal;
        if self.active {
            if self.drawn {
                terminal.draw(&hush_editor::erase_command_line(self.at, self.start_col));
                self.drawn = false;
            }
            terminal.draw("\x1b[H\x1b[2J\x1b[3J");
            // zsh redraws its prompt on ^L, which draws the command again.
            terminal.write(b"\x0c");
        } else {
            terminal.draw("\x1b[H\x1b[2J\x1b[3J");
        }
    }

    /// After the grid reflowed: put the cursor where zsh expects it (the command
    /// start) so its redraw on SIGWINCH lines up.
    pub fn after_resize(&mut self, env: &mut Env) {
        if !self.active || !self.drawn {
            return;
        }
        let cols = env.terminal.size().cols as usize;
        let layout = hush_editor::layout_text(self.editor.text(), self.start_col, cols);
        let at = layout.cell_at(self.editor.cursor());
        env.terminal
            .draw(&move_to_command_start(at, self.start_col));
        self.at = Cell {
            row: 0,
            col: self.start_col,
        };
    }

    /// Redraws the command; also the fallback when zsh doesn't redraw after a resize.
    pub fn render(&mut self, env: &mut Env) {
        if !self.active {
            return;
        }
        let ghost = self.compute_ghost(env);
        let cols = env.terminal.size().cols as usize;
        let e = &self.editor;
        // Leave the line alone (e.g. RPROMPT) until there's something to draw.
        if !self.drawn && e.is_empty() && ghost.is_empty() {
            self.layout = Some(hush_editor::layout_text("", self.start_col, cols));
            return;
        }
        let spans = highlight(
            e.text(),
            |name| env.catalog.is_known(name),
            Some(e.cursor()),
        );
        let out = render_command_line(&RenderInput {
            text: e.text(),
            cursor: e.cursor(),
            spans: &spans,
            ghost: &ghost,
            start_col: self.start_col,
            cols,
            from: self.at,
        });
        env.terminal.draw(&out.sequence);
        self.at = out.cursor;
        self.layout = Some(out.layout);
        self.ghost = ghost;
        self.drawn = true;
        self.sync_selection_to_terminal(env);
        env.terminal.term.lock().scroll_display(Scroll::Bottom);
    }

    /// A click inside the command moves the cursor there.
    pub fn click(&mut self, point: Point, env: &mut Env) {
        let Some(cell) = self.cell_for(point, env) else {
            return;
        };
        let Some(layout) = &self.layout else {
            return;
        };
        self.editor.move_to(layout.offset_at(cell), false);
        self.browser = None;
        self.render(env);
    }

    /// The mouse changed the terminal selection: select the same text in the editor
    /// when it lies within the command.
    pub fn terminal_selection_changed(&mut self, env: &mut Env) {
        if !self.active {
            return;
        }
        self.owns_selection = false;
        let range = {
            let term = env.terminal.term.lock();
            term.selection.as_ref().and_then(|s| s.to_range(&*term))
        };
        let offsets = range.and_then(|range| {
            let layout = self.layout.as_ref()?;
            let a = layout.offset_at(self.cell_for(range.start, env)?);
            let end = self.cell_for(range.end, env)?;
            let b = layout.offset_at(Cell {
                col: end.col + 1,
                ..end
            });
            (a != b).then_some((a, b))
        });
        match offsets {
            Some((a, b)) => {
                self.editor.select(a, b);
                self.owns_selection = true;
            }
            None => self.editor.clear_selection(),
        }
        // Hide the suggestion while something is selected.
        if !self.ghost.is_empty() || (range.is_none() && self.drawn) {
            self.render(env);
        }
    }

    fn finish(&mut self, command: &str, action: CommandAction, env: &mut Env) -> Option<String> {
        self.release_selection(env);
        let spans = highlight(command, |name| env.catalog.is_known(name), None);
        if self.drawn || !command.is_empty() {
            let out = render_command_line(&RenderInput {
                text: command,
                cursor: 0,
                spans: &spans,
                ghost: "",
                start_col: self.start_col,
                cols: env.terminal.size().cols as usize,
                from: self.at,
            });
            env.terminal.draw(&out.sequence);
            self.at = out.cursor;
        }
        self.drawn = false;
        self.ghost.clear();
        self.active = false;
        env.terminal
            .submit(command, &zsh_highlights(command, &spans), action);
        self.editor.reset("");
        matches!(action, CommandAction::Run).then(|| command.to_owned())
    }

    fn move_vertical(&mut self, direction: Direction, extend: bool, env: &mut Env) {
        let e = &mut self.editor;
        if e.move_vertical(direction, extend) {
            return;
        }
        if extend {
            e.move_by(Unit::All, direction, true);
            return;
        }
        let browser = self
            .browser
            .get_or_insert_with(|| HistoryBrowser::new(env.history, e.text()));
        let entry = match direction {
            Direction::Backward => browser.older(),
            Direction::Forward => browser.newer(),
        };
        if let Some(entry) = entry {
            e.set_text(entry);
        }
    }

    fn accept_ghost(&mut self, word: bool) {
        let piece = if word {
            next_word(&self.ghost)
        } else {
            &self.ghost
        };
        let piece = piece.to_owned();
        self.editor.insert(&piece);
    }

    fn compute_ghost(&self, env: &Env) -> String {
        let e = &self.editor;
        let foreign_selection =
            !self.owns_selection && env.terminal.term.lock().selection.is_some();
        if e.selection().is_some() || !e.cursor_at_end() || e.is_empty() || foreign_selection {
            return String::new();
        }
        let is_dir = |path: &str| Some(env.folders.is_dir(path, &SystemSources));
        env.history
            .suggest(e.text(), |entry| fits_here(entry, env.place, is_dir))
            .map(|entry| entry[e.text().len()..].to_owned())
            .unwrap_or_default()
    }

    /// Tab: completes a single match right away, or the part all matches share.
    /// The menu for choosing between matches comes with the menu UI.
    fn complete(&mut self, env: &mut Env) {
        let e = &mut self.editor;
        let ctx = completion_context(e.text(), e.cursor());
        let items = complete(&ctx, env.place, &env.catalog.names(), &SystemSources);
        let insert = match items.as_slice() {
            [] => return,
            [item] => format!(
                "{}{}",
                item.insert,
                if item.append_space { " " } else { "" }
            ),
            _ => {
                let prefix = common_prefix(&items);
                if prefix.len() <= ctx.raw_word.len() {
                    return;
                }
                prefix.to_owned()
            }
        };
        e.select(ctx.word_start, e.cursor());
        e.insert(&insert);
    }

    /// Grid line of the command's first row.
    fn start_line(&self, env: &Env) -> i32 {
        env.terminal.cursor().line.0 - self.at.row as i32
    }

    fn cell_for(&self, point: Point, env: &Env) -> Option<Cell> {
        let row = point.line.0 - self.start_line(env);
        (row >= 0).then_some(Cell {
            row: row as usize,
            col: point.column.0,
        })
    }

    fn point_for(&self, cell: Cell, env: &Env) -> Point {
        Point::new(
            Line(self.start_line(env) + cell.row as i32),
            Column(cell.col),
        )
    }

    /// Mirrors the editor selection as the terminal selection, so it's drawn and
    /// ⌘C copies it like any other selection.
    fn sync_selection_to_terminal(&mut self, env: &mut Env) {
        let (Some(range), Some(layout)) = (self.editor.selection(), &self.layout) else {
            self.release_selection(env);
            return;
        };
        let start = self.point_for(layout.cell_at(range.start), env);
        let end = self.point_for(layout.cell_at(range.end), env);
        let mut term = env.terminal.term.lock();
        let last = end.sub(&*term, Boundary::Grid, 1);
        let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
        selection.update(last, Side::Right);
        term.selection = Some(selection);
        self.owns_selection = true;
    }

    fn release_selection(&mut self, env: &mut Env) {
        if self.owns_selection {
            env.terminal.term.lock().selection = None;
            self.owns_selection = false;
        }
    }

    fn clear_foreign_selection(&mut self, env: &mut Env) {
        if !self.owns_selection {
            env.terminal.term.lock().selection = None;
        }
    }
}

/// The ghost text up to and including the next word (and a trailing slash).
fn next_word(ghost: &str) -> &str {
    let rest = ghost.trim_start();
    let lead = ghost.len() - rest.len();
    let word = rest
        .find(|c: char| c.is_whitespace() || c == '/')
        .map_or(rest.len(), |i| {
            if rest[i..].starts_with('/') {
                i + 1
            } else {
                i
            }
        });
    let piece = &ghost[..lead + word];
    if piece.is_empty() {
        ghost
    } else {
        piece
    }
}

#[cfg(test)]
mod tests {
    use super::next_word;

    #[test]
    fn ghost_words_end_at_spaces_and_slashes() {
        assert_eq!(next_word(" status --short"), " status");
        assert_eq!(next_word("rc/main.rs"), "rc/");
        assert_eq!(next_word("ö ä"), "ö");
        assert_eq!(next_word(""), "");
    }
}

//! The command line: edited in Hush, drawn into the terminal grid right after the
//! prompt, and handed to zsh when it runs.

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Boundary, Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use hush_complete::{
    common_prefix, complete, completion_context, fits_here, CommandCatalog, CompletionItem,
    CompletionKind, FolderCheck, HistoryBrowser, HistoryStore, Place, SystemSources,
};
use hush_core::pty::CommandAction;
use hush_editor::{
    highlight, is_incomplete, move_to_command_start, render_command_line, zsh_highlights, Cell,
    Direction, EditorCommand, Layout, LineEditor, MenuAction, RenderInput, Unit,
};

use crate::blocks::Tag;
use crate::icons::Icon;
use crate::terminal::Terminal;

/// Rows the menu shows at once.
pub const MENU_ROWS: usize = 8;
const HISTORY_RESULTS: usize = 100;

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
    menu: Option<Menu>,
    notice: Option<Notice>,
    notices_shown: u64,
    /// The block this prompt belongs to.
    block: u64,
}

/// A command handed to the shell.
pub struct Submitted {
    pub block: u64,
    pub command: String,
}

enum Choices {
    Complete {
        word_start: usize,
        items: Vec<CompletionItem>,
    },
    History(Vec<String>),
}

impl Choices {
    fn len(&self) -> usize {
        match self {
            Choices::Complete { items, .. } => items.len(),
            Choices::History(items) => items.len(),
        }
    }
}

struct Menu {
    choices: Choices,
    selected: usize,
    /// First visible row.
    top: usize,
}

impl Menu {
    fn new(choices: Choices) -> Self {
        Self {
            choices,
            selected: 0,
            top: 0,
        }
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        if index < self.top {
            self.top = index;
        } else if index >= self.top + MENU_ROWS {
            self.top = index + 1 - MENU_ROWS;
        }
    }

    fn step(&mut self, direction: Direction) {
        let len = self.choices.len();
        if len > 0 {
            let delta = if direction == Direction::Forward {
                1
            } else {
                len - 1
            };
            self.select((self.selected + delta) % len);
        }
    }

    fn replace(&mut self, choices: Choices) {
        let len = choices.len();
        self.choices = choices;
        self.top = self.top.min(len.saturating_sub(MENU_ROWS));
        self.select(self.selected.min(len.saturating_sub(1)));
    }
}

/// A short message under the command, like "No matches".
struct Notice {
    text: String,
    offset: usize,
    id: u64,
}

/// The menu or notice as it should be drawn.
pub struct Popup<'a> {
    pub title: Option<String>,
    pub rows: Vec<PopupRow<'a>>,
    /// Shown instead of rows when there are none.
    pub empty: &'a str,
    pub hints: &'static [&'static str],
    /// The grid cell it hangs from.
    pub anchor: Point,
}

pub struct PopupRow<'a> {
    pub index: usize,
    pub label: &'a str,
    /// Bytes at the start of `label` that match what was typed.
    pub matched: usize,
    pub detail: Option<&'a str>,
    pub icon: Icon,
    pub selected: bool,
}

impl PromptEditor {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// zsh printed its prompt and waits for a command (OSC 133;B).
    pub fn begin_input(&mut self, cursor: Point, block: u64, env: &mut Env) {
        if env
            .terminal
            .term
            .lock()
            .mode()
            .contains(TermMode::ALT_SCREEN)
        {
            return;
        }
        self.block = block;
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
        self.close_menu();
    }

    pub fn menu_open(&self) -> bool {
        self.menu.is_some()
    }

    /// Runs a key command. Returns the command if one was handed to the shell.
    pub fn execute(&mut self, command: EditorCommand, env: &mut Env) -> Option<Submitted> {
        if !matches!(command, EditorCommand::Vertical { .. }) {
            self.browser = None;
        }
        self.notice = None;
        let before = (self.editor.text().to_owned(), self.editor.cursor());
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
            EditorCommand::HistorySearch => {
                let items = self.search_history(env);
                self.menu = Some(Menu::new(Choices::History(items)));
            }
            EditorCommand::Menu(action) => self.menu_action(action),
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
        }
        if (self.editor.text(), self.editor.cursor()) != (before.0.as_str(), before.1) {
            self.refresh_menu(env);
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
        self.notice = None;
        self.refresh_menu(env);
        self.render(env);
    }

    /// Replaces what was typed with `command` and runs it.
    pub fn run_command(&mut self, command: &str, env: &mut Env) -> Option<Submitted> {
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
        let Some(cell) = self.cell_for(point, env.terminal) else {
            return;
        };
        let Some(layout) = &self.layout else {
            return;
        };
        self.editor.move_to(layout.offset_at(cell), false);
        self.browser = None;
        self.refresh_menu(env);
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
            let a = layout.offset_at(self.cell_for(range.start, env.terminal)?);
            let end = self.cell_for(range.end, env.terminal)?;
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

    fn finish(&mut self, command: &str, action: CommandAction, env: &mut Env) -> Option<Submitted> {
        self.release_selection(env);
        self.close_menu();
        let run = matches!(action, CommandAction::Run);
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
            let tag = run.then(|| Tag::Command.link(self.block));
            env.terminal.draw_tagged(&out.sequence, tag);
            self.at = out.cursor;
        }
        self.drawn = false;
        self.ghost.clear();
        self.active = false;
        env.terminal
            .submit(command, &zsh_highlights(command, &spans), action);
        self.editor.reset("");
        run.then(|| Submitted {
            block: self.block,
            command: command.to_owned(),
        })
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
        if e.selection().is_some()
            || !e.cursor_at_end()
            || e.is_empty()
            || foreign_selection
            || self.menu.is_some()
        {
            return String::new();
        }
        let is_dir = |path: &str| Some(env.folders.is_dir(path, &SystemSources));
        env.history
            .suggest(e.text(), |entry| fits_here(entry, env.place, is_dir))
            .map(|entry| entry[e.text().len()..].to_owned())
            .unwrap_or_default()
    }

    /// Tab: completes a single match right away, otherwise inserts the part all
    /// matches share and opens the menu.
    fn complete(&mut self, env: &mut Env) {
        let e = &mut self.editor;
        let ctx = completion_context(e.text(), e.cursor());
        let items = complete(&ctx, env.place, &env.catalog.names(), &SystemSources);
        match items.as_slice() {
            [] => {
                let text = if ctx.word.is_empty() {
                    "Nothing to complete here".to_owned()
                } else {
                    format!("No matches for '{}'", ctx.word)
                };
                self.show_notice(text, ctx.word_start);
            }
            [item] => self.apply_completion(item, ctx.word_start),
            _ => {
                let prefix = common_prefix(&items);
                if prefix.len() > ctx.raw_word.len() {
                    let prefix = prefix.to_owned();
                    e.select(ctx.word_start, e.cursor());
                    e.insert(&prefix);
                }
                self.menu = Some(Menu::new(Choices::Complete {
                    word_start: ctx.word_start,
                    items,
                }));
            }
        }
    }

    fn apply_completion(&mut self, item: &CompletionItem, word_start: usize) {
        let e = &mut self.editor;
        e.select(word_start, e.cursor());
        e.insert(&item.insert);
        if item.append_space {
            e.insert(" ");
        }
    }

    fn search_history(&self, env: &Env) -> Vec<String> {
        env.history
            .search(self.editor.text(), HISTORY_RESULTS)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    /// Follows what was typed while the menu is open.
    fn refresh_menu(&mut self, env: &mut Env) {
        let Some(menu) = &self.menu else {
            return;
        };
        let e = &self.editor;
        let choices = match &menu.choices {
            Choices::History(_) => Choices::History(self.search_history(env)),
            Choices::Complete { word_start, .. } => {
                let typed = e.text().get(*word_start..e.cursor()).unwrap_or(" ");
                if typed.replace("\\ ", "").contains(char::is_whitespace) {
                    self.close_menu();
                    return;
                }
                let ctx = completion_context(e.text(), e.cursor());
                let items = complete(&ctx, env.place, &env.catalog.names(), &SystemSources);
                if items.is_empty() {
                    self.close_menu();
                    return;
                }
                Choices::Complete {
                    word_start: ctx.word_start,
                    items,
                }
            }
        };
        if let Some(menu) = &mut self.menu {
            menu.replace(choices);
        }
    }

    fn menu_action(&mut self, action: MenuAction) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        match action {
            MenuAction::Next => menu.step(Direction::Forward),
            MenuAction::Previous => menu.step(Direction::Backward),
            MenuAction::Accept => {
                let index = menu.selected;
                self.accept(index);
            }
            MenuAction::Close => self.close_menu(),
        }
    }

    /// A menu item was chosen with the mouse.
    pub fn pick(&mut self, index: usize, env: &mut Env) {
        self.accept(index);
        self.refresh_menu(env);
        self.render(env);
    }

    fn accept(&mut self, index: usize) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        match menu.choices {
            Choices::History(items) => {
                if let Some(entry) = items.get(index) {
                    self.editor.set_text(entry);
                }
            }
            Choices::Complete { word_start, items } => {
                if let Some(item) = items.get(index) {
                    self.apply_completion(item, word_start);
                }
            }
        }
    }

    pub fn close_menu(&mut self) {
        self.menu = None;
        self.notice = None;
    }

    fn show_notice(&mut self, text: String, offset: usize) {
        self.notices_shown += 1;
        self.notice = Some(Notice {
            text,
            offset,
            id: self.notices_shown,
        });
    }

    /// Identifies the notice on screen, so a timer can hide that one.
    pub fn notice_id(&self) -> Option<u64> {
        self.notice.as_ref().map(|n| n.id)
    }

    pub fn dismiss_notice(&mut self, id: u64) {
        if self.notice_id() == Some(id) {
            self.notice = None;
        }
    }

    /// The menu or notice to draw, if any.
    pub fn popup(&self, terminal: &Terminal) -> Option<Popup<'_>> {
        if !self.active {
            return None;
        }
        if let Some(notice) = &self.notice {
            return Some(Popup {
                title: None,
                rows: Vec::new(),
                empty: &notice.text,
                hints: &[],
                anchor: self.anchor(notice.offset, terminal)?,
            });
        }
        let menu = self.menu.as_ref()?;
        let visible = menu.top..(menu.top + MENU_ROWS).min(menu.choices.len());
        let row = |index: usize, label, matched, detail, icon| PopupRow {
            index,
            label,
            matched,
            detail,
            icon,
            selected: index == menu.selected,
        };
        Some(match &menu.choices {
            Choices::History(items) => {
                let query = self.editor.text().trim();
                Popup {
                    title: Some(if query.is_empty() {
                        "History".to_owned()
                    } else {
                        format!("History matching '{query}'")
                    }),
                    rows: visible
                        .map(|i| row(i, items[i].as_str(), 0, None, Icon::History))
                        .collect(),
                    empty: "No matching commands in your history",
                    hints: &["type to filter", "↑↓ choose", "↩ use", "esc close"],
                    anchor: self.anchor(0, terminal)?,
                }
            }
            Choices::Complete { word_start, items } => Popup {
                title: None,
                rows: visible
                    .map(|i| {
                        let item = &items[i];
                        row(
                            i,
                            &item.label,
                            item.matched,
                            item.detail,
                            icon_for(item.kind),
                        )
                    })
                    .collect(),
                empty: "No completions",
                hints: &["↑↓ choose", "⇥ or ↩ accept", "esc close"],
                anchor: self.anchor(*word_start, terminal)?,
            },
        })
    }

    fn anchor(&self, offset: usize, terminal: &Terminal) -> Option<Point> {
        let layout = self.layout.as_ref()?;
        Some(self.point_for(
            layout.cell_at(offset.min(self.editor.text().len())),
            terminal,
        ))
    }

    /// Grid line of the command's first row.
    fn start_line(&self, terminal: &Terminal) -> i32 {
        terminal.cursor().line.0 - self.at.row as i32
    }

    fn cell_for(&self, point: Point, terminal: &Terminal) -> Option<Cell> {
        let row = point.line.0 - self.start_line(terminal);
        (row >= 0).then_some(Cell {
            row: row as usize,
            col: point.column.0,
        })
    }

    fn point_for(&self, cell: Cell, terminal: &Terminal) -> Point {
        Point::new(
            Line(self.start_line(terminal) + cell.row as i32),
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
        let start = self.point_for(layout.cell_at(range.start), env.terminal);
        let end = self.point_for(layout.cell_at(range.end), env.terminal);
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

fn icon_for(kind: CompletionKind) -> Icon {
    match kind {
        CompletionKind::Command | CompletionKind::Subcommand => Icon::Prompt,
        CompletionKind::Folder => Icon::Folder,
        CompletionKind::File => Icon::File,
        CompletionKind::Branch => Icon::Branch,
        CompletionKind::Flag => Icon::Flag,
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

use std::collections::VecDeque;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Char,
    Word,
    Line,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Backward,
    Forward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Type,
    Space,
    Other,
}

#[derive(Debug, Clone)]
struct Snapshot {
    text: String,
    cursor: usize,
}

const MAX_UNDO: usize = 500;

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The command being edited: text, cursor, selection and undo history.
/// Offsets are byte offsets into `text()` and always sit on char boundaries.
#[derive(Debug, Clone, Default)]
pub struct LineEditor {
    text: String,
    cursor: usize,
    anchor: Option<usize>,
    undo_stack: VecDeque<Snapshot>,
    redo_stack: Vec<Snapshot>,
    last_edit: Option<EditKind>,
}

impl LineEditor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.anchor.filter(|&anchor| anchor != self.cursor)?;
        Some(anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn is_multiline(&self) -> bool {
        self.text.contains('\n')
    }

    pub fn cursor_at_end(&self) -> bool {
        self.cursor == self.text.len()
    }

    pub fn insert(&mut self, input: &str) {
        let mut chars = input.chars();
        let kind = match (chars.next(), chars.next()) {
            (None, _) => return,
            (Some(c), None) if c.is_whitespace() => EditKind::Space,
            (Some(_), None) => EditKind::Type,
            _ => EditKind::Other,
        };
        let range = self.selection().unwrap_or(self.cursor..self.cursor);
        let cursor = range.start + input.len();
        let mut text = self.text.clone();
        text.replace_range(range, input);
        self.edit(text, cursor, kind);
    }

    pub fn delete(&mut self, unit: Unit, direction: Direction) -> bool {
        if let Some(sel) = self.selection() {
            return self.delete_range(sel.start, sel.end);
        }
        self.delete_range(self.boundary(unit, direction), self.cursor)
    }

    /// Deletes back to the previous whitespace, like ctrl-w in a shell.
    pub fn delete_shell_word(&mut self) -> bool {
        if let Some(sel) = self.selection() {
            return self.delete_range(sel.start, sel.end);
        }
        let start = self.text[..self.cursor]
            .trim_end_matches(char::is_whitespace)
            .trim_end_matches(|c: char| !c.is_whitespace())
            .len();
        self.delete_range(start, self.cursor)
    }

    pub fn delete_range(&mut self, start: usize, end: usize) -> bool {
        let from = self.clamp(start.min(end));
        let to = self.clamp(start.max(end));
        if from == to {
            return false;
        }
        let mut text = self.text.clone();
        text.replace_range(from..to, "");
        self.edit(text, from, EditKind::Other);
        true
    }

    /// Without `extend`, an existing selection collapses to its edge (macOS behavior).
    pub fn move_by(&mut self, unit: Unit, direction: Direction, extend: bool) {
        if !extend && unit == Unit::Char {
            if let Some(sel) = self.selection() {
                let edge = match direction {
                    Direction::Backward => sel.start,
                    Direction::Forward => sel.end,
                };
                self.move_to(edge, false);
                return;
            }
        }
        self.move_to(self.boundary(unit, direction), extend);
    }

    pub fn move_to(&mut self, offset: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = self.clamp(offset);
        self.last_edit = None;
    }

    /// Returns false on the first/last line so the caller can browse history instead.
    pub fn move_vertical(&mut self, direction: Direction, extend: bool) -> bool {
        let start = self.line_start(self.cursor);
        let end = self.line_end(self.cursor);
        let target_start = match direction {
            Direction::Backward if start == 0 => return false,
            Direction::Forward if end == self.text.len() => return false,
            Direction::Backward => self.line_start(start - 1),
            Direction::Forward => end + 1,
        };
        let column = self.text[start..self.cursor].chars().count();
        let target_end = self.line_end(target_start);
        let target = self.text[target_start..target_end]
            .char_indices()
            .nth(column)
            .map_or(target_end, |(i, _)| target_start + i);
        self.move_to(target, extend);
        true
    }

    pub fn select(&mut self, anchor: usize, cursor: usize) {
        self.anchor = Some(self.clamp(anchor));
        self.cursor = self.clamp(cursor);
        self.last_edit = None;
    }

    pub fn select_all(&mut self) {
        self.select(0, self.text.len());
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Replaces the text as one undoable edit and puts the cursor at the end.
    pub fn set_text(&mut self, text: &str) {
        self.set_text_at(text, text.len());
    }

    pub fn set_text_at(&mut self, text: &str, cursor: usize) {
        if text == self.text {
            self.move_to(cursor, false);
        } else {
            self.edit(text.to_string(), cursor, EditKind::Other);
        }
    }

    /// Replaces the text and clears the undo history.
    pub fn reset(&mut self, text: &str) {
        *self = Self {
            text: text.to_string(),
            cursor: text.len(),
            ..Self::default()
        };
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo_stack.pop_back() else {
            return false;
        };
        let current = self.restore(snapshot);
        self.redo_stack.push(current);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo_stack.pop() else {
            return false;
        };
        let current = self.restore(snapshot);
        self.undo_stack.push_back(current);
        true
    }

    /// Where a move by `unit` in `direction` from the cursor lands.
    pub fn boundary(&self, unit: Unit, direction: Direction) -> usize {
        let i = self.cursor;
        match (unit, direction) {
            (Unit::Char, Direction::Backward) => self.text[..i]
                .chars()
                .next_back()
                .map_or(i, |c| i - c.len_utf8()),
            (Unit::Char, Direction::Forward) => self.text[i..]
                .chars()
                .next()
                .map_or(i, |c| i + c.len_utf8()),
            (Unit::Word, Direction::Backward) => self.word_start(i),
            (Unit::Word, Direction::Forward) => self.word_end(i),
            (Unit::Line, Direction::Backward) => self.line_start(i),
            (Unit::Line, Direction::Forward) => self.line_end(i),
            (Unit::All, Direction::Backward) => 0,
            (Unit::All, Direction::Forward) => self.text.len(),
        }
    }

    pub fn line_start(&self, offset: usize) -> usize {
        let offset = self.clamp(offset);
        self.text[..offset].rfind('\n').map_or(0, |i| i + 1)
    }

    pub fn line_end(&self, offset: usize) -> usize {
        let offset = self.clamp(offset);
        self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |i| offset + i)
    }

    fn word_start(&self, i: usize) -> usize {
        self.text[..i]
            .trim_end_matches(|c| !is_word_char(c))
            .trim_end_matches(is_word_char)
            .len()
    }

    fn word_end(&self, i: usize) -> usize {
        let rest = self.text[i..]
            .trim_start_matches(|c| !is_word_char(c))
            .trim_start_matches(is_word_char);
        self.text.len() - rest.len()
    }

    fn edit(&mut self, text: String, cursor: usize, kind: EditKind) {
        let previous = std::mem::replace(&mut self.text, text);
        // Consecutive typed characters undo together, like in a text editor.
        let coalesce = kind == EditKind::Type && self.last_edit == Some(EditKind::Type);
        if !coalesce {
            self.undo_stack.push_back(Snapshot {
                text: previous,
                cursor: self.cursor,
            });
            if self.undo_stack.len() > MAX_UNDO {
                self.undo_stack.pop_front();
            }
        }
        self.redo_stack.clear();
        self.cursor = self.clamp(cursor);
        self.anchor = None;
        self.last_edit = Some(kind);
    }

    /// Restores `snapshot` and returns the state it replaced.
    fn restore(&mut self, snapshot: Snapshot) -> Snapshot {
        let previous = Snapshot {
            text: std::mem::replace(&mut self.text, snapshot.text),
            cursor: self.cursor,
        };
        self.cursor = self.clamp(snapshot.cursor);
        self.anchor = None;
        self.last_edit = None;
        previous
    }

    /// Keeps an offset inside the text and moves it back onto a char boundary.
    fn clamp(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::{Backward, Forward};

    const END: usize = usize::MAX;

    fn editor(text: &str, cursor: usize) -> LineEditor {
        let mut e = LineEditor::new();
        e.reset(text);
        e.move_to(cursor, false);
        e
    }

    #[test]
    fn inserts_at_the_cursor_and_replaces_a_selection() {
        let mut e = editor("git status", 3);
        e.insert("!");
        assert_eq!(e.text(), "git! status");
        e.select(5, 11);
        e.insert("log");
        assert_eq!(e.text(), "git! log");
        assert_eq!(e.cursor(), 8);
    }

    #[test]
    fn deletes_by_character_word_and_line() {
        let mut e = editor("git checkout -b fix-tests", END);
        e.delete(Unit::Word, Backward);
        assert_eq!(e.text(), "git checkout -b fix-");
        e.delete(Unit::Char, Backward);
        assert_eq!(e.text(), "git checkout -b fix");
        e.delete(Unit::Line, Backward);
        assert_eq!(e.text(), "");
    }

    #[test]
    fn deletes_a_shell_word_back_to_whitespace() {
        let mut e = editor("cd ~/workspace/food-planner", END);
        e.delete_shell_word();
        assert_eq!(e.text(), "cd ");
    }

    #[test]
    fn jumps_words_over_punctuation_like_macos() {
        let mut e = editor("npm run build:prod", 18);
        e.move_by(Unit::Word, Backward, false);
        assert_eq!(e.cursor(), 14);
        e.move_by(Unit::Word, Backward, false);
        assert_eq!(e.cursor(), 8);
        e.move_by(Unit::Word, Forward, false);
        assert_eq!(e.cursor(), 13);
    }

    #[test]
    fn extends_and_collapses_selections() {
        let mut e = editor("hello world", 5);
        e.move_by(Unit::Word, Forward, true);
        assert_eq!(e.selection(), Some(5..11));
        e.move_by(Unit::Char, Backward, false);
        assert_eq!(e.cursor(), 5);
        assert_eq!(e.selection(), None);
    }

    #[test]
    fn moves_between_lines_and_reports_the_edges() {
        let mut e = editor("one\ntwo three\nfour", 6);
        assert!(e.move_vertical(Backward, false));
        assert_eq!(e.cursor(), 2);
        assert!(!e.move_vertical(Backward, false));
        e.move_to(16, false);
        assert!(!e.move_vertical(Forward, false));
        assert!(e.move_vertical(Backward, false));
        assert_eq!(e.cursor(), 6);
    }

    #[test]
    fn works_on_the_current_line_for_line_operations() {
        let mut e = editor("echo one\necho two", 13);
        e.delete(Unit::Line, Backward);
        assert_eq!(e.text(), "echo one\n two");
    }

    #[test]
    fn undoes_typing_as_one_step_and_redoes_it() {
        let mut e = editor("", END);
        for ch in ["g", "i", "t"] {
            e.insert(ch);
        }
        e.insert(" ");
        for ch in ["l", "o", "g"] {
            e.insert(ch);
        }
        e.undo();
        assert_eq!(e.text(), "git ");
        e.undo();
        assert_eq!(e.text(), "git");
        e.undo();
        assert_eq!(e.text(), "");
        e.redo();
        assert_eq!(e.text(), "git");
    }

    #[test]
    fn never_splits_a_multibyte_char() {
        let mut e = editor("ok 👍", END);
        e.delete(Unit::Char, Backward);
        assert_eq!(e.text(), "ok ");
        let mut f = editor("👍", 0);
        f.move_by(Unit::Char, Forward, false);
        assert_eq!(f.cursor(), 4);
    }

    #[test]
    fn selects_all_and_resets() {
        let mut e = editor("ls -la", END);
        e.select_all();
        assert_eq!(e.selection(), Some(0..6));
        e.reset("");
        assert!(e.is_empty());
        assert!(!e.undo());
    }

    #[test]
    fn moves_by_word_and_char_over_multibyte_text() {
        let mut e = editor("ls ä😀", END);
        e.move_by(Unit::Word, Backward, false);
        assert_eq!(e.cursor(), 3);
        e.move_by(Unit::Word, Forward, false);
        assert_eq!(e.cursor(), 5);
        e.move_by(Unit::Char, Forward, false);
        assert_eq!(e.cursor(), 9);
        e.move_by(Unit::Char, Backward, false);
        assert_eq!(e.cursor(), 5);
        e.delete(Unit::Char, Forward);
        assert_eq!(e.text(), "ls ä");
        e.delete(Unit::Char, Backward);
        assert_eq!(e.text(), "ls ");
    }

    #[test]
    fn deletes_words_over_multibyte_text() {
        let mut e = editor("ls ä😀", END);
        e.delete(Unit::Word, Backward);
        assert_eq!(e.text(), "ls ");
        let mut f = editor("ls ä😀", END);
        f.delete_shell_word();
        assert_eq!(f.text(), "ls ");
        let mut g = editor("ls ä😀", 3);
        g.delete(Unit::Word, Forward);
        assert_eq!(g.text(), "ls 😀");
    }

    #[test]
    fn snaps_offsets_inside_a_char_back_to_its_start() {
        let mut e = editor("ls ä😀", 4);
        assert_eq!(e.cursor(), 3);
        e.select(4, 7);
        assert_eq!(e.selection(), Some(3..5));
        assert!(e.delete_range(4, 6));
        assert_eq!(e.text(), "ls 😀");
    }

    #[test]
    fn keeps_the_column_in_chars_when_moving_between_lines() {
        let mut e = editor("abcd\näöü", 2);
        assert!(e.move_vertical(Forward, false));
        assert_eq!(e.cursor(), 9);
        e.move_to(END, false);
        assert!(e.move_vertical(Backward, false));
        assert_eq!(e.cursor(), 3);
    }

    #[test]
    fn undoes_typed_multibyte_chars_as_one_step() {
        let mut e = editor("", END);
        for ch in ["ä", "😀", "ö"] {
            e.insert(ch);
        }
        assert_eq!(e.text(), "ä😀ö");
        e.undo();
        assert_eq!(e.text(), "");
    }
}

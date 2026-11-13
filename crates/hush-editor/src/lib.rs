//! Hush's command line editor: editing, key bindings, syntax highlighting and
//! drawing the command into the terminal grid. Text offsets are byte offsets
//! into UTF-8 strings and always sit on char boundaries.

pub mod highlight;
pub mod incomplete;
pub mod keymap;
pub mod layout;
pub mod line_editor;
pub mod render;

pub use highlight::{highlight, unquote, zsh_highlights, Span, Style};
pub use incomplete::is_incomplete;
pub use keymap::{resolve_key, EditorCommand, Key, MenuAction};
pub use layout::{display_width, layout_text, Cell, Layout};
pub use line_editor::{Direction, LineEditor, Unit};
pub use render::{
    erase_command_line, move_to_command_start, render_command_line, RenderInput, RenderOutput,
};

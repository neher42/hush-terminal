//! Command blocks: a prompt, the command run from it and the output it printed.
//!
//! Hush tags the prompt, the command and the shell's end-of-output padding in the
//! grid itself, as hyperlinks only it uses. Blocks then scroll, reflow and get
//! cleared together with their text.

use std::collections::BTreeMap;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Line, Point};
use alacritty_terminal::term::cell::Hyperlink;
use alacritty_terminal::term::Term;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Prompt,
    Command,
    /// What the shell prints between the end of the output and the next prompt
    /// (zsh's PROMPT_SP mark for output without a final newline).
    End,
}

impl Tag {
    fn prefix(self) -> &'static str {
        match self {
            Tag::Prompt => "hush-prompt:",
            Tag::Command => "hush-command:",
            Tag::End => "hush-end:",
        }
    }

    pub fn link(self, block: u64) -> Hyperlink {
        Hyperlink::new(Some("hush"), format!("{}{block}", self.prefix()))
    }

    fn parse(link: &Hyperlink) -> Option<(Tag, u64)> {
        [Tag::Prompt, Tag::Command, Tag::End]
            .into_iter()
            .find_map(|tag| Some((tag, link.uri().strip_prefix(tag.prefix())?.parse().ok()?)))
    }
}

fn tag_at<T>(term: &Term<T>, point: Point) -> Option<(Tag, u64)> {
    Tag::parse(&term.grid()[point].hyperlink()?)
}

/// The block a line starts with, if it starts with its prompt or command.
fn head_of<T>(term: &Term<T>, line: Line) -> Option<u64> {
    match tag_at(term, Point::new(line, Column(0)))? {
        (Tag::Prompt | Tag::Command, id) => Some(id),
        (Tag::End, _) => None,
    }
}

/// Where a block is in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub id: u64,
    pub start: Line,
    /// Last line of the prompt and command.
    pub head_end: Line,
    pub end: Line,
}

/// The block that `line` belongs to.
pub fn block_at<T>(term: &Term<T>, line: Line) -> Option<Block> {
    let (top, bottom) = (term.topmost_line(), term.bottommost_line());
    if line < top || line > bottom {
        return None;
    }
    let mut found = line;
    let id = loop {
        if let Some(id) = head_of(term, found) {
            break id;
        }
        if found == top {
            return None;
        }
        found -= 1;
    };
    let is_head = |line: Line| head_of(term, line) == Some(id);
    let mut start = found;
    while start > top && is_head(start - 1) {
        start -= 1;
    }
    let mut head_end = found;
    while head_end < bottom && is_head(head_end + 1) {
        head_end += 1;
    }
    // The output runs to the next prompt, or to the cursor while it's the last block.
    let cursor = term.grid().cursor.point.line;
    let mut end = head_end;
    while end < bottom && end < cursor && head_of(term, end + 1).is_none() {
        end += 1;
    }
    (line <= end).then_some(Block {
        id,
        start,
        head_end,
        end,
    })
}

/// The text a block's command printed.
pub fn output_text<T>(term: &Term<T>, block: &Block) -> String {
    if block.end <= block.head_end {
        return String::new();
    }
    let start: Point = Point::new(block.head_end + 1, Column(0));
    let mut end = Point::new(block.end, term.last_column());
    let padding = (start.line.max(block.end - 1).0..=block.end.0)
        .flat_map(|line| (0..term.columns()).map(move |col| Point::new(Line(line), Column(col))))
        .find(|&point| tag_at(term, point) == Some((Tag::End, block.id)));
    if let Some(padding) = padding {
        if padding <= start {
            return String::new();
        }
        end = padding.sub(term, Boundary::Grid, 1);
    }
    term.bounds_to_string(start, end).trim_end().to_owned()
}

/// What ran in a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub command: String,
    pub exit_code: Option<i32>,
}

/// Commands by block, for the blocks that may still be on screen or in scrollback.
#[derive(Default)]
pub struct BlockLog {
    ran: BTreeMap<u64, Ran>,
}

const KEEP: usize = 5000;

impl BlockLog {
    pub fn ran(&mut self, block: u64, command: String) {
        self.ran.insert(
            block,
            Ran {
                command,
                exit_code: None,
            },
        );
        while self.ran.len() > KEEP {
            self.ran.pop_first();
        }
    }

    pub fn finished(&mut self, block: u64, exit_code: Option<i32>) {
        if let Some(ran) = self.ran.get_mut(&block) {
            ran.exit_code = exit_code;
        }
    }

    pub fn get(&self, block: u64) -> Option<&Ran> {
        self.ran.get(&block)
    }
}

/// A block as a Markdown snippet to send to Claude.
pub fn transcript(ran: &Ran, output: &str) -> String {
    let mut text = format!("```\n$ {}\n", ran.command);
    if !output.is_empty() {
        text += output;
        text += "\n";
    }
    text += "```\n";
    if let Some(code) = ran.exit_code.filter(|&code| code != 0) {
        text += &format!("(exit code {code})\n");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;

    use crate::terminal::GridSize;

    struct Screen {
        term: Term<VoidListener>,
        parser: Processor,
    }

    impl Screen {
        fn new(cols: u16, rows: u16) -> Self {
            let size = GridSize {
                cols,
                rows,
                cell_width: 8,
                cell_height: 16,
            };
            Self {
                term: Term::new(Config::default(), &size, VoidListener),
                parser: Processor::new(),
            }
        }

        fn print(&mut self, tag: Option<(Tag, u64)>, text: &str) {
            let link = tag.map(|(tag, id)| tag.link(id));
            self.term.grid_mut().cursor.template.set_hyperlink(link);
            self.parser.advance(&mut self.term, text.as_bytes());
            self.term.grid_mut().cursor.template.set_hyperlink(None);
        }

        fn block(&mut self, id: u64, command: &str, output: &str) {
            self.print(Some((Tag::Prompt, id)), "$ ");
            self.print(Some((Tag::Command, id)), command);
            self.print(None, &format!("\r\n{output}"));
        }
    }

    #[test]
    fn finds_the_block_of_an_output_line() {
        let mut s = Screen::new(20, 10);
        s.block(1, "ls", "a\r\nb\r\n");
        s.block(2, "pwd", "/tmp\r\n");
        s.print(Some((Tag::Prompt, 3)), "$ ");

        let first = block_at(&s.term, Line(1)).unwrap();
        assert_eq!((first.id, first.start, first.end), (1, Line(0), Line(2)));
        assert_eq!(output_text(&s.term, &first), "a\nb");
        let second = block_at(&s.term, Line(3)).unwrap();
        assert_eq!((second.id, second.end), (2, Line(4)));
        assert_eq!(output_text(&s.term, &second), "/tmp");
        assert_eq!(block_at(&s.term, Line(5)).map(|b| b.id), Some(3));
        assert_eq!(block_at(&s.term, Line(7)), None);
    }

    #[test]
    fn a_wrapped_command_is_part_of_the_head() {
        let mut s = Screen::new(10, 10);
        s.block(1, "echo 1234567890", "x\r\n");
        s.print(Some((Tag::Prompt, 2)), "$ ");
        let block = block_at(&s.term, Line(2)).unwrap();
        assert_eq!((block.head_end, block.end), (Line(1), Line(2)));
        assert_eq!(output_text(&s.term, &block), "x");
    }

    #[test]
    fn leaves_out_the_end_of_output_mark() {
        let mut s = Screen::new(20, 10);
        s.print(Some((Tag::Prompt, 1)), "$ ");
        s.print(Some((Tag::Command, 1)), "printf hi");
        s.print(None, "\r\nhi");
        s.print(Some((Tag::End, 1)), "%                   \r \r");
        s.print(Some((Tag::Prompt, 2)), "$ ");
        let block = block_at(&s.term, Line(1)).unwrap();
        assert_eq!(output_text(&s.term, &block), "hi");
    }

    #[test]
    fn transcript_mentions_failures_only() {
        let ran = Ran {
            command: "false".into(),
            exit_code: Some(1),
        };
        assert_eq!(transcript(&ran, ""), "```\n$ false\n```\n(exit code 1)\n");
        let ok = Ran {
            exit_code: Some(0),
            ..ran
        };
        assert_eq!(transcript(&ok, "out"), "```\n$ false\nout\n```\n");
    }
}

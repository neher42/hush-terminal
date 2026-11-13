use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Point;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use hush_core::events::{OscScanner, ShellEvent};
use hush_core::pty::{CommandAction, PtyManager};

/// What the PTY thread reports to the UI.
pub enum Output {
    /// New output was parsed into the grid.
    Wakeup,
    /// A shell marker and the cursor position at the moment it was printed.
    Shell(ShellEvent, Point),
    /// Replies, title changes and the like from the terminal emulator.
    Term(Event),
    Exit,
}

#[derive(Clone)]
pub struct Listener(UnboundedSender<Output>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.unbounded_send(Output::Term(event));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

impl From<GridSize> for WindowSize {
    fn from(s: GridSize) -> Self {
        WindowSize {
            num_lines: s.rows,
            num_cols: s.cols,
            cell_width: s.cell_width,
            cell_height: s.cell_height,
        }
    }
}

/// Parses shell output on the PTY thread, stopping at each shell marker so the
/// cursor position that goes with it is exact. Programs often write line by line,
/// so this runs for every few bytes and must stay cheap: the UI hears about new
/// output once per frame, not once per read.
struct Parser {
    term: Arc<FairMutex<Term<Listener>>>,
    tx: UnboundedSender<Output>,
    /// Set while a wakeup is on its way to the UI, so there is only ever one.
    awake: Arc<AtomicBool>,
    parser: Processor,
    scanner: OscScanner,
}

impl Parser {
    fn feed(&mut self, bytes: &[u8]) {
        let mut markers = Vec::new();
        self.scanner
            .scan(bytes, |end, event| markers.push((end, event)));

        let mut term = self.term.lock();
        let mut done = 0;
        for (end, event) in markers {
            self.parser.advance(&mut *term, &bytes[done..end]);
            done = end;
            let cursor = term.grid().cursor.point;
            let _ = self.tx.unbounded_send(Output::Shell(event, cursor));
        }
        self.parser.advance(&mut *term, &bytes[done..]);
        // A program that starts a synchronized update and never ends it shouldn't
        // freeze the screen.
        if self
            .parser
            .sync_timeout()
            .sync_timeout()
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            self.parser.stop_sync(&mut *term);
        }
        drop(term);
        if !self.awake.swap(true, Ordering::AcqRel) {
            let _ = self.tx.unbounded_send(Output::Wakeup);
        }
    }
}

/// A shell session: the PTY, and the grid its output is parsed into.
pub struct Terminal {
    pub term: Arc<FairMutex<Term<Listener>>>,
    pty: Arc<PtyManager>,
    id: u32,
    /// Parses what Hush itself draws (the command line), separate from the shell's
    /// stream which may stop in the middle of a sequence.
    local: Processor,
    awake: Arc<AtomicBool>,
    size: GridSize,
}

impl Terminal {
    pub fn spawn(
        pty: Arc<PtyManager>,
        cwd: Option<&Path>,
        size: GridSize,
    ) -> hush_core::Result<(Self, UnboundedReceiver<Output>)> {
        let (tx, rx) = unbounded();
        let term = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &size,
            Listener(tx.clone()),
        )));
        let awake = Arc::new(AtomicBool::new(false));
        let mut parser = Parser {
            term: Arc::clone(&term),
            tx,
            awake: Arc::clone(&awake),
            parser: Processor::new(),
            scanner: OscScanner::default(),
        };
        let exit = parser.tx.clone();
        let id = pty.spawn(
            size.cols,
            size.rows,
            cwd,
            move |bytes| {
                parser.feed(bytes);
                true
            },
            move |_| {
                let _ = exit.unbounded_send(Output::Exit);
            },
        )?;
        let terminal = Self {
            term,
            pty,
            id,
            local: Processor::new(),
            awake,
            size,
        };
        Ok((terminal, rx))
    }

    /// The UI took in the latest output; the next output sends a new wakeup.
    pub fn woke(&self) {
        self.awake.store(false, Ordering::Release);
    }

    pub fn size(&self) -> GridSize {
        self.size
    }

    pub fn write(&self, bytes: &[u8]) {
        let _ = self.pty.write(self.id, bytes);
    }

    pub fn submit(&self, command: &str, highlights: &str, action: CommandAction) {
        let _ = self
            .pty
            .submit_command(self.id, command, highlights, action);
    }

    /// Draws into the grid as if the shell had printed `sequence`.
    pub fn draw(&mut self, sequence: &str) {
        let mut term = self.term.lock();
        self.local.advance(&mut *term, sequence.as_bytes());
    }

    pub fn cursor(&self) -> Point {
        self.term.lock().grid().cursor.point
    }

    pub fn resize(&mut self, size: GridSize) {
        if size == self.size || size.cols == 0 || size.rows == 0 {
            return;
        }
        self.size = size;
        self.term.lock().resize(size);
        let _ = self.pty.resize(self.id, size.cols, size.rows);
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.pty.kill(self.id);
    }
}

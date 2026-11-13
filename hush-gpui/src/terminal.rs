use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener, OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Notifier};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::tty;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};

/// Forwards terminal events (new output, replies, exit) to the UI thread.
#[derive(Clone)]
pub struct Listener(UnboundedSender<Event>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.unbounded_send(event);
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

/// A shell running in a pseudo-terminal. Parsing happens on alacritty's PTY thread;
/// the UI only locks the grid to draw it.
pub struct Terminal {
    pub term: Arc<FairMutex<Term<Listener>>>,
    notifier: Notifier,
    size: GridSize,
}

impl Terminal {
    /// `command` replaces the login shell, e.g. for benchmarks (`hush-gpui -e cmd args`).
    pub fn spawn(
        command: Option<(String, Vec<String>)>,
        size: GridSize,
    ) -> io::Result<(Self, UnboundedReceiver<Event>)> {
        let (tx, rx) = unbounded();
        let listener = Listener(tx);
        let env = HashMap::from([
            ("TERM".to_owned(), "xterm-256color".to_owned()),
            ("COLORTERM".to_owned(), "truecolor".to_owned()),
            ("TERM_PROGRAM".to_owned(), "Hush".to_owned()),
        ]);
        // Start the user's shell directly as a login shell, like the web version, rather
        // than through /usr/bin/login (alacritty_terminal's default).
        let (program, args) = command.unwrap_or_else(|| {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
            (shell, vec!["-l".to_owned()])
        });
        let options = tty::Options {
            shell: Some(tty::Shell::new(program, args)),
            working_directory: std::env::var_os("HOME").map(Into::into),
            env,
            ..Default::default()
        };
        let pty = tty::new(&options, size.into(), 0)?;
        let term = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &size,
            listener.clone(),
        )));
        let event_loop = EventLoop::new(Arc::clone(&term), listener, pty, false, false)?;
        let notifier = Notifier(event_loop.channel());
        event_loop.spawn();
        Ok((
            Self {
                term,
                notifier,
                size,
            },
            rx,
        ))
    }

    pub fn write(&self, bytes: impl Into<std::borrow::Cow<'static, [u8]>>) {
        use alacritty_terminal::event::Notify;
        self.notifier.notify(bytes);
    }

    pub fn resize(&mut self, size: GridSize) {
        if size == self.size || size.cols == 0 || size.rows == 0 {
            return;
        }
        self.size = size;
        self.term.lock().resize(size);
        self.notifier.on_resize(size.into());
    }
}

mod colors;
mod element;
mod keys;
mod terminal;
mod view;

use std::sync::Arc;

use gpui::{
    px, size, App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions,
};
use hush_core::pty::PtyManager;
use hush_core::shell::ShellIntegration;

/// Same folder as the Tauri app, so both share settings and session files.
const APP_ID: &str = "dev.hush.terminal";

fn main() {
    // `hush-gpui --run "command"` runs a command at the first prompt (for benchmarks).
    let args: Vec<String> = std::env::args().collect();
    let run = args
        .iter()
        .position(|arg| arg == "--run")
        .and_then(|i| args.get(i + 1).cloned());

    let data_dir = dirs::data_dir()
        .expect("no application support folder")
        .join(APP_ID);
    let integration = ShellIntegration::install(&data_dir.join("shell-integration"))
        .expect("failed to install the shell integration");
    let pty = Arc::new(PtyManager::new(integration, data_dir.join("sessions")));

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1040.), px(640.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Hush".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            cx.new(|cx| view::TerminalView::new(pty, run, window, cx))
        })
        .expect("failed to open the window");
        cx.activate(true);
    });
}

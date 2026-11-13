mod colors;
mod keys;
mod terminal;
mod view;

use gpui::{
    px, size, App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions,
};

fn main() {
    // `hush-gpui -e program args...` runs a command instead of the login shell.
    let args: Vec<String> = std::env::args().collect();
    let command = args.iter().position(|arg| arg == "-e").and_then(|i| {
        args.get(i + 1)
            .map(|program| (program.clone(), args[i + 2..].to_vec()))
    });

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
            cx.new(|cx| view::TerminalView::new(command, window, cx))
        })
        .expect("failed to open the window");
        cx.activate(true);
    });
}

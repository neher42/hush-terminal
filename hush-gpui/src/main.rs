mod actions;
mod block_bar;
mod blocks;
mod colors;
mod element;
mod icons;
mod keys;
mod panes;
mod popup;
mod prompt;
mod settings;
mod settings_panel;
mod sidebar;
mod terminal;
mod tree;
mod ui;
mod view;
mod workspace;

use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, Application};
use hush_core::pty::PtyManager;
use hush_core::shell::ShellIntegration;

use crate::settings::{CurrentSettings, SettingsFile};
use crate::workspace::Shared;

/// The bundle identifier, also the name of the settings and sessions folder.
const APP_ID: &str = "dev.hush.terminal";

const FONTS: [&[u8]; 2] = [
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
];

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
    let settings_file = SettingsFile::new(data_dir.join("settings.json"));

    Application::new()
        .with_assets(icons::Assets)
        .run(move |cx: &mut App| {
            cx.text_system()
                .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
                .expect("bundled fonts load");
            actions::bind_keys(cx);
            cx.set_menus(actions::menus());
            cx.on_action(|_: &actions::Quit, cx| cx.quit());
            cx.on_action(|_: &actions::Hide, cx| cx.hide());
            cx.on_action(|_: &actions::HideOthers, cx| cx.hide_other_apps());
            cx.on_action(|_: &actions::ShowAll, cx| cx.unhide_other_apps());
            let quitting = pty.clone();
            cx.on_app_quit(move |_| {
                quitting.kill_all();
                async {}
            })
            .detach();

            cx.set_global(CurrentSettings(settings_file.load()));
            let shared = Shared {
                pty,
                settings_file: Rc::new(settings_file),
                history: Rc::default(),
            };
            let home = dirs::home_dir()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".to_owned());
            workspace::open_window(shared, home, run, None, cx);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}

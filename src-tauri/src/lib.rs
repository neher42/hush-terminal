mod claude;
mod commands;
mod completion;
mod error;
mod files;
mod fsutil;
mod history;
mod menu;
mod pty;
mod settings;
mod shell;

use tauri::{Manager, RunEvent};

use crate::pty::PtyManager;
use crate::shell::ShellIntegration;

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .menu(menu::build)
        .on_menu_event(|app, event| menu::forward(app, &event))
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            let integration = ShellIntegration::install(&data_dir.join("shell-integration"))?;
            app.manage(PtyManager::new(integration, data_dir.join("sessions")));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pty_spawn,
            commands::pty_write,
            commands::pty_submit_command,
            commands::pty_resize,
            commands::pty_kill,
            commands::home_dir,
            commands::list_dir,
            commands::is_dir,
            commands::read_history,
            commands::list_executables,
            commands::git_branches,
            commands::send_to_claude,
            commands::read_settings,
            commands::write_settings,
            commands::set_appearance_menu,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the Hush app");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            if let Some(pty) = app.try_state::<PtyManager>() {
                pty.kill_all();
            }
        }
    });
}

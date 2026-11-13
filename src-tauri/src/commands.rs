use std::path::PathBuf;

use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, Manager, State, Wry};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::menu::AppearanceMenu;
use hush_core::error::{Error, Result};
use hush_core::files::DirEntry;
use hush_core::git::GitStatus;
use hush_core::pty::{CommandAction, PtyManager};
use hush_core::{claude, executables, files, git, history, settings};

#[tauri::command]
pub fn pty_spawn(
    pty: State<'_, PtyManager>,
    cols: u16,
    rows: u16,
    cwd: Option<PathBuf>,
    on_data: Channel<Response>,
    on_exit: Channel<u32>,
) -> Result<u32> {
    pty.spawn(
        cols,
        rows,
        cwd.as_deref(),
        move |bytes| on_data.send(Response::new(bytes.to_vec())).is_ok(),
        move |code| {
            let _ = on_exit.send(code);
        },
    )
}

#[tauri::command]
pub fn pty_write(pty: State<'_, PtyManager>, id: u32, data: String) -> Result<()> {
    pty.write(id, data.as_bytes())
}

#[tauri::command]
pub fn pty_submit_command(
    pty: State<'_, PtyManager>,
    id: u32,
    command: String,
    highlights: String,
    action: CommandAction,
) -> Result<()> {
    pty.submit_command(id, &command, &highlights, action)
}

#[tauri::command]
pub fn pty_resize(pty: State<'_, PtyManager>, id: u32, cols: u16, rows: u16) -> Result<()> {
    pty.resize(id, cols, rows)
}

#[tauri::command]
pub fn pty_kill(pty: State<'_, PtyManager>, id: u32) -> Result<()> {
    pty.kill(id)
}

#[tauri::command]
pub fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| Error::Message("Couldn't find your home folder.".into()))
}

#[tauri::command]
pub async fn list_dir(path: PathBuf) -> Result<Vec<DirEntry>> {
    files::list_dir(&path)
}

#[tauri::command]
pub async fn is_dir(path: PathBuf) -> bool {
    path.is_dir()
}

#[tauri::command]
pub async fn read_history(path: PathBuf) -> Vec<String> {
    history::read_zsh_history(&path)
}

#[tauri::command]
pub async fn list_executables(path_var: String) -> Vec<String> {
    executables::list_executables(&path_var)
}

#[tauri::command]
pub async fn git_branches(cwd: PathBuf) -> Vec<String> {
    git::branches(&cwd)
}

#[tauri::command]
pub async fn git_status(cwd: PathBuf) -> Option<GitStatus> {
    git::status(&cwd)
}

#[tauri::command]
pub async fn read_settings(app: AppHandle) -> Result<Option<String>> {
    settings::read(&settings_path(&app)?)
}

#[tauri::command]
pub async fn write_settings(app: AppHandle, json: String) -> Result<()> {
    settings::write(&settings_path(&app)?, &json)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("settings.json"))
        .map_err(|err| Error::Message(err.to_string()))
}

#[tauri::command]
pub fn claude_permission() -> bool {
    claude::has_permission()
}

#[tauri::command]
pub fn open_claude_permission_settings() {
    claude::open_permission_settings();
}

#[tauri::command]
pub fn set_appearance_menu(menu: State<'_, AppearanceMenu<Wry>>, mode: String) {
    menu.select(&mode);
}

#[tauri::command]
pub async fn send_to_claude(app: AppHandle, text: String) -> Result<claude::Delivery> {
    app.clipboard()
        .write_text(text)
        .map_err(|err| Error::Message(err.to_string()))?;
    Ok(claude::deliver())
}

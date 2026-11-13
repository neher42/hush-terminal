use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuEvent, MenuItem, MenuItemBuilder,
    SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub const MENU_EVENT: &str = "menu";

const APPEARANCES: [(&str, &str); 3] = [
    ("system", "Match System"),
    ("light", "Light"),
    ("dark", "Dark"),
];

/// The View > Appearance items, kept so the check mark can follow the setting.
pub struct AppearanceMenu<R: Runtime>(Vec<(&'static str, CheckMenuItem<R>)>);

impl<R: Runtime> AppearanceMenu<R> {
    pub fn select(&self, mode: &str) {
        for (item_mode, item) in &self.0 {
            let _ = item.set_checked(*item_mode == mode);
        }
    }
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let item = |id: &str, text: &str, accelerator: &str| -> tauri::Result<MenuItem<R>> {
        MenuItemBuilder::with_id(id, text)
            .accelerator(accelerator)
            .build(app)
    };

    let hush = SubmenuBuilder::new(app, "Hush")
        .about(None)
        .separator()
        .item(&item("settings", "Settings…", "CmdOrCtrl+,")?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let shell = SubmenuBuilder::new(app, "Shell")
        .item(&item("new_tab", "New Tab", "CmdOrCtrl+T")?)
        .item(&item("close_tab", "Close Tab", "CmdOrCtrl+W")?)
        .separator()
        .item(&item("clear", "Clear", "CmdOrCtrl+K")?)
        .build()?;
    // Copy/Paste stay native; Cut and Select All need the command line editor.
    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&item("cut", "Cut", "CmdOrCtrl+X")?)
        .copy()
        .paste()
        .item(&item("select_all", "Select All", "CmdOrCtrl+A")?)
        .separator()
        .item(&item(
            "copy_path",
            "Copy Current Folder Path",
            "CmdOrCtrl+Shift+C",
        )?)
        .build()?;
    let appearance_items = APPEARANCES
        .iter()
        .map(|&(mode, label)| {
            CheckMenuItemBuilder::with_id(format!("appearance_{mode}"), label)
                .build(app)
                .map(|item| (mode, item))
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let mut appearance = SubmenuBuilder::new(app, "Appearance");
    for (_, item) in &appearance_items {
        appearance = appearance.item(item);
    }
    let appearance = appearance.build()?;
    app.manage(AppearanceMenu(appearance_items));

    let view = SubmenuBuilder::new(app, "View")
        .item(&item(
            "toggle_sidebar",
            "Show or Hide Folder Tree",
            "CmdOrCtrl+B",
        )?)
        .item(&appearance)
        .separator()
        .item(&item("font_bigger", "Bigger Text", "CmdOrCtrl+=")?)
        .item(&item("font_smaller", "Smaller Text", "CmdOrCtrl+-")?)
        .item(&item("font_reset", "Actual Size", "CmdOrCtrl+0")?)
        .separator()
        .fullscreen()
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .item(&item(
            "previous_tab",
            "Show Previous Tab",
            "Alt+CmdOrCtrl+ArrowLeft",
        )?)
        .item(&item(
            "next_tab",
            "Show Next Tab",
            "Alt+CmdOrCtrl+ArrowRight",
        )?)
        .build()?;

    MenuBuilder::new(app)
        .items(&[&hush, &shell, &edit, &view, &window])
        .build()
}

pub fn forward<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) {
    // Clicking a check item toggles only that item; keep exactly one checked.
    if let Some(mode) = event.id().as_ref().strip_prefix("appearance_") {
        app.state::<AppearanceMenu<R>>().select(mode);
    }
    if let Err(err) = app.emit(MENU_EVENT, event.id().as_ref()) {
        eprintln!("Couldn't forward menu event: {err}");
    }
}

//! Menu commands and their shortcuts.

use gpui::{actions, App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType};

actions!(
    hush,
    [
        Quit,
        Hide,
        HideOthers,
        ShowAll,
        OpenSettings,
        NewWindow,
        NewTab,
        CloseTab,
        Clear,
        Cut,
        Copy,
        Paste,
        SelectAll,
        CopyPath,
        ToggleSidebar,
        AppearanceSystem,
        AppearanceLight,
        AppearanceDark,
        BiggerText,
        SmallerText,
        ActualSize,
        Minimize,
        Zoom,
        PreviousTab,
        NextTab,
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-n", NewWindow, None),
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-k", Clear, None),
        KeyBinding::new("cmd-x", Cut, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-v", Paste, None),
        KeyBinding::new("cmd-a", SelectAll, None),
        KeyBinding::new("shift-cmd-c", CopyPath, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-=", BiggerText, None),
        KeyBinding::new("cmd-+", BiggerText, None),
        KeyBinding::new("cmd--", SmallerText, None),
        KeyBinding::new("cmd-0", ActualSize, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("alt-cmd-left", PreviousTab, None),
        KeyBinding::new("alt-cmd-right", NextTab, None),
        KeyBinding::new("shift-cmd-[", PreviousTab, None),
        KeyBinding::new("shift-cmd-]", NextTab, None),
    ]);
}

pub fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Hush".into(),
            items: vec![
                MenuItem::action("Settings…", OpenSettings),
                MenuItem::separator(),
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Hide Hush", Hide),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit Hush", Quit),
            ],
        },
        Menu {
            name: "Shell".into(),
            items: vec![
                MenuItem::action("New Window", NewWindow),
                MenuItem::action("New Tab", NewTab),
                MenuItem::action("Close Tab", CloseTab),
                MenuItem::separator(),
                MenuItem::action("Clear", Clear),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::action("Copy Current Folder Path", CopyPath),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Show or Hide Folder Tree", ToggleSidebar),
                MenuItem::submenu(Menu {
                    name: "Appearance".into(),
                    items: vec![
                        MenuItem::action("Match System", AppearanceSystem),
                        MenuItem::action("Light", AppearanceLight),
                        MenuItem::action("Dark", AppearanceDark),
                    ],
                }),
                MenuItem::separator(),
                MenuItem::action("Bigger Text", BiggerText),
                MenuItem::action("Smaller Text", SmallerText),
                MenuItem::action("Actual Size", ActualSize),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
                MenuItem::separator(),
                MenuItem::action("Show Previous Tab", PreviousTab),
                MenuItem::action("Show Next Tab", NextTab),
            ],
        },
    ]
}

use crate::line_editor::{Direction, Unit};

/// A key press with gpui key names: "enter", "backspace", "left", "a", ...
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Key<'a> {
    pub key: &'a str,
    pub key_char: Option<&'a str>,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Next,
    Previous,
    Accept,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand {
    Submit,
    Newline,
    Complete,
    Cancel,
    ClearScreen,
    HistorySearch,
    Escape,
    Undo,
    Redo,
    Move {
        unit: Unit,
        direction: Direction,
        extend: bool,
    },
    Vertical {
        direction: Direction,
        extend: bool,
    },
    Delete {
        unit: Unit,
        direction: Direction,
    },
    DeleteShellWord,
    Menu(MenuAction),
}

/// Returns `None` for keys the editor leaves to the terminal. Shortcuts owned
/// by the app menu (cmd-a, cmd-k, alt-cmd-left/right) must stay `None` so the
/// menu receives them.
pub fn resolve_key(key: &Key, menu_open: bool) -> Option<EditorCommand> {
    use Direction::{Backward, Forward};
    use EditorCommand::*;

    let Key {
        key,
        ctrl,
        alt,
        shift,
        cmd,
        ..
    } = *key;
    let move_by = |unit, direction| Move {
        unit,
        direction,
        extend: shift,
    };

    if menu_open {
        let action = match key {
            "down" => Some(MenuAction::Next),
            "n" if ctrl => Some(MenuAction::Next),
            "up" => Some(MenuAction::Previous),
            "p" if ctrl => Some(MenuAction::Previous),
            "tab" if shift => Some(MenuAction::Previous),
            "tab" | "enter" if !shift => Some(MenuAction::Accept),
            "escape" => Some(MenuAction::Close),
            "c" | "g" if ctrl => Some(MenuAction::Close),
            _ => None,
        };
        if let Some(action) = action {
            return Some(Menu(action));
        }
    }

    if cmd && !ctrl && !alt {
        return match key {
            "z" if shift => Some(Redo),
            "z" => Some(Undo),
            "backspace" => Some(Delete {
                unit: Unit::Line,
                direction: Backward,
            }),
            "delete" => Some(Delete {
                unit: Unit::Line,
                direction: Forward,
            }),
            "left" => Some(move_by(Unit::Line, Backward)),
            "right" => Some(move_by(Unit::Line, Forward)),
            "up" => Some(move_by(Unit::All, Backward)),
            "down" => Some(move_by(Unit::All, Forward)),
            "enter" => Some(Newline),
            _ => None,
        };
    }

    if ctrl && !cmd && !alt {
        let delete = |unit, direction| Some(Delete { unit, direction });
        return match key {
            "a" => Some(move_by(Unit::Line, Backward)),
            "e" => Some(move_by(Unit::Line, Forward)),
            "b" => Some(move_by(Unit::Char, Backward)),
            "f" => Some(move_by(Unit::Char, Forward)),
            "p" => Some(Vertical {
                direction: Backward,
                extend: false,
            }),
            "n" => Some(Vertical {
                direction: Forward,
                extend: false,
            }),
            "k" => delete(Unit::Line, Forward),
            "u" => delete(Unit::Line, Backward),
            "w" => Some(DeleteShellWord),
            "h" => delete(Unit::Char, Backward),
            "d" => delete(Unit::Char, Forward),
            "c" => Some(Cancel),
            "l" => Some(ClearScreen),
            "r" => Some(HistorySearch),
            "j" => Some(Newline),
            _ => None,
        };
    }

    if cmd || ctrl {
        return None;
    }

    let unit = if alt { Unit::Word } else { Unit::Char };
    match key {
        "enter" if shift || alt => Some(Newline),
        "enter" => Some(Submit),
        "tab" if shift => None,
        "tab" => Some(Complete),
        "escape" => Some(Escape),
        "backspace" => Some(Delete {
            unit,
            direction: Backward,
        }),
        "delete" => Some(Delete {
            unit,
            direction: Forward,
        }),
        "left" => Some(move_by(unit, Backward)),
        "right" => Some(move_by(unit, Forward)),
        "up" => Some(Vertical {
            direction: Backward,
            extend: shift,
        }),
        "down" => Some(Vertical {
            direction: Forward,
            extend: shift,
        }),
        "home" => Some(move_by(Unit::Line, Backward)),
        "end" => Some(move_by(Unit::Line, Forward)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::Backward;

    fn key(name: &str) -> Key<'_> {
        Key {
            key: name,
            ..Key::default()
        }
    }

    fn resolve(key: Key, menu_open: bool) -> Option<EditorCommand> {
        resolve_key(&key, menu_open)
    }

    #[test]
    fn leaves_typing_copy_and_paste_to_the_terminal() {
        assert_eq!(resolve(key("a"), false), None);
        assert_eq!(
            resolve(
                Key {
                    cmd: true,
                    ..key("c")
                },
                false
            ),
            None
        );
        assert_eq!(
            resolve(
                Key {
                    cmd: true,
                    ..key("v")
                },
                false
            ),
            None
        );
    }

    #[test]
    fn uses_macos_text_shortcuts() {
        assert_eq!(
            resolve(
                Key {
                    cmd: true,
                    ..key("backspace")
                },
                false
            ),
            Some(EditorCommand::Delete {
                unit: Unit::Line,
                direction: Backward
            })
        );
        assert_eq!(
            resolve(
                Key {
                    alt: true,
                    ..key("backspace")
                },
                false
            ),
            Some(EditorCommand::Delete {
                unit: Unit::Word,
                direction: Backward
            })
        );
        assert_eq!(
            resolve(
                Key {
                    alt: true,
                    shift: true,
                    ..key("left")
                },
                false
            ),
            Some(EditorCommand::Move {
                unit: Unit::Word,
                direction: Backward,
                extend: true
            })
        );
        assert_eq!(
            resolve(
                Key {
                    cmd: true,
                    shift: true,
                    ..key("z")
                },
                false
            ),
            Some(EditorCommand::Redo)
        );
    }

    #[test]
    fn leaves_app_menu_shortcuts_to_the_menu() {
        assert_eq!(
            resolve(
                Key {
                    cmd: true,
                    ..key("a")
                },
                false
            ),
            None
        );
        assert_eq!(
            resolve(
                Key {
                    cmd: true,
                    ..key("k")
                },
                false
            ),
            None
        );
        for name in ["left", "right"] {
            let k = Key {
                cmd: true,
                alt: true,
                ..key(name)
            };
            assert_eq!(resolve(k, false), None);
        }
    }

    #[test]
    fn keeps_shell_control_keys() {
        let ctrl = |name| Key {
            ctrl: true,
            ..key(name)
        };
        assert_eq!(
            resolve(ctrl("a"), false),
            Some(EditorCommand::Move {
                unit: Unit::Line,
                direction: Backward,
                extend: false
            })
        );
        assert_eq!(resolve(ctrl("c"), false), Some(EditorCommand::Cancel));
        assert_eq!(
            resolve(ctrl("r"), false),
            Some(EditorCommand::HistorySearch)
        );
        assert_eq!(
            resolve(ctrl("w"), false),
            Some(EditorCommand::DeleteShellWord)
        );
    }

    #[test]
    fn submits_on_enter_and_adds_a_line_on_shift_enter() {
        assert_eq!(resolve(key("enter"), false), Some(EditorCommand::Submit));
        assert_eq!(
            resolve(
                Key {
                    shift: true,
                    ..key("enter")
                },
                false
            ),
            Some(EditorCommand::Newline)
        );
    }

    #[test]
    fn routes_navigation_keys_to_an_open_menu() {
        let menu = |action| Some(EditorCommand::Menu(action));
        assert_eq!(resolve(key("down"), true), menu(MenuAction::Next));
        assert_eq!(resolve(key("enter"), true), menu(MenuAction::Accept));
        assert_eq!(resolve(key("escape"), true), menu(MenuAction::Close));
        assert_eq!(
            resolve(key("left"), true),
            Some(EditorCommand::Move {
                unit: Unit::Char,
                direction: Backward,
                extend: false
            })
        );
    }
}

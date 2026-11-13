//! Translates key presses into the bytes a terminal program expects.

#[derive(Debug, Clone, Copy, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Bytes for a key press, or `None` for keys the terminal ignores.
/// `app_cursor` is the DECCKM mode some programs (vim, less) switch on.
pub fn to_bytes(
    key: &str,
    key_char: Option<&str>,
    mods: Mods,
    app_cursor: bool,
) -> Option<Vec<u8>> {
    let csi = |code: &str| {
        let prefix = if app_cursor && !mods.shift && !mods.alt && !mods.ctrl {
            "\x1bO"
        } else {
            "\x1b["
        };
        Some(format!("{prefix}{code}").into_bytes())
    };
    let named: Option<Vec<u8>> = match key {
        "enter" => Some(b"\r".to_vec()),
        "backspace" => Some(if mods.alt {
            b"\x1b\x7f".to_vec()
        } else {
            b"\x7f".to_vec()
        }),
        "tab" => Some(if mods.shift {
            b"\x1b[Z".to_vec()
        } else {
            b"\t".to_vec()
        }),
        "escape" => Some(b"\x1b".to_vec()),
        "up" => csi("A"),
        "down" => csi("B"),
        "right" => csi("C"),
        "left" => csi("D"),
        "home" => csi("H"),
        "end" => csi("F"),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        "delete" => Some(b"\x1b[3~".to_vec()),
        _ => None,
    };
    if named.is_some() {
        return named;
    }

    if mods.ctrl {
        let c = key.chars().next()?;
        if key.len() == 1 && c.is_ascii_alphabetic() {
            return Some(vec![(c.to_ascii_lowercase() as u8) - b'a' + 1]);
        }
        return match key {
            "space" | "@" => Some(vec![0]),
            "[" => Some(vec![0x1b]),
            "\\" => Some(vec![0x1c]),
            "]" => Some(vec![0x1d]),
            _ => None,
        };
    }

    let text = key_char
        .map(str::to_owned)
        .or_else(|| (key == "space").then(|| " ".to_owned()))?;
    let mut bytes = Vec::with_capacity(text.len() + 1);
    if mods.alt {
        bytes.push(0x1b);
    }
    bytes.extend_from_slice(text.as_bytes());
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: false,
    };

    #[test]
    fn typed_text_and_special_keys() {
        assert_eq!(to_bytes("a", Some("a"), NONE, false), Some(b"a".to_vec()));
        assert_eq!(to_bytes("enter", None, NONE, false), Some(b"\r".to_vec()));
        assert_eq!(
            to_bytes("backspace", None, NONE, false),
            Some(b"\x7f".to_vec())
        );
        assert_eq!(
            to_bytes("space", Some(" "), NONE, false),
            Some(b" ".to_vec())
        );
    }

    #[test]
    fn control_keys() {
        let ctrl = Mods { ctrl: true, ..NONE };
        assert_eq!(to_bytes("c", None, ctrl, false), Some(vec![3]));
        assert_eq!(to_bytes("r", None, ctrl, false), Some(vec![18]));
    }

    #[test]
    fn arrows_follow_cursor_key_mode() {
        assert_eq!(to_bytes("up", None, NONE, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(to_bytes("up", None, NONE, true), Some(b"\x1bOA".to_vec()));
    }

    #[test]
    fn option_sends_escape_prefix() {
        let alt = Mods { alt: true, ..NONE };
        assert_eq!(
            to_bytes("b", Some("b"), alt, false),
            Some(b"\x1bb".to_vec())
        );
    }
}

//! Shell integration markers: OSC 133 (semantic prompts), OSC 7 (working directory)
//! and OSC 7777 (Hush metadata as `key=value`).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    PromptStart,
    CommandInputStart,
    CommandStart,
    CommandEnd { exit_code: Option<i32> },
    Cwd { host: String, path: String },
    Meta { key: String, value: String },
}

impl ShellEvent {
    /// Parses the payload of an OSC sequence (between `ESC ]` and the terminator).
    pub fn parse_osc(payload: &[u8]) -> Option<Self> {
        let payload = std::str::from_utf8(payload).ok()?;
        let (code, data) = payload.split_once(';')?;
        match code {
            "133" => parse_prompt_marker(data),
            "7" => parse_working_directory(data),
            "7777" => parse_meta(data),
            _ => None,
        }
    }
}

fn parse_prompt_marker(data: &str) -> Option<ShellEvent> {
    let mut parts = data.split(';');
    match parts.next()? {
        "A" => Some(ShellEvent::PromptStart),
        "B" => Some(ShellEvent::CommandInputStart),
        "C" => Some(ShellEvent::CommandStart),
        "D" => Some(ShellEvent::CommandEnd {
            exit_code: parts.next().and_then(|code| code.parse().ok()),
        }),
        _ => None,
    }
}

fn parse_working_directory(data: &str) -> Option<ShellEvent> {
    let rest = data.strip_prefix("file://")?;
    let slash = rest.find('/')?;
    Some(ShellEvent::Cwd {
        host: rest[..slash].to_owned(),
        path: percent_decode(&rest[slash..])?,
    })
}

fn parse_meta(data: &str) -> Option<ShellEvent> {
    let (key, value) = data.split_once('=')?;
    (!key.is_empty()).then(|| ShellEvent::Meta {
        key: key.to_owned(),
        value: value.to_owned(),
    })
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

const MAX_OSC_LEN: usize = 64 * 1024;

#[derive(Debug, Default)]
enum State {
    #[default]
    Ground,
    Escape,
    Osc,
    OscEscape,
}

/// Finds shell events in terminal output. Sequences may be split across reads, so
/// the scanner keeps its state between calls. The output itself is left untouched.
#[derive(Debug, Default)]
pub struct OscScanner {
    state: State,
    payload: Vec<u8>,
}

impl OscScanner {
    /// Calls `on_event` with each event and the offset just past its sequence.
    pub fn scan(&mut self, bytes: &[u8], mut on_event: impl FnMut(usize, ShellEvent)) {
        for (i, &byte) in bytes.iter().enumerate() {
            self.state = match (&self.state, byte) {
                (State::Ground, 0x1b) => State::Escape,
                (State::Ground, _) => State::Ground,
                (State::Escape, b']') => {
                    self.payload.clear();
                    State::Osc
                }
                (State::Escape, 0x1b) => State::Escape,
                (State::Escape, _) => State::Ground,
                (State::Osc | State::OscEscape, 0x07) | (State::OscEscape, b'\\') => {
                    if let Some(event) = ShellEvent::parse_osc(&self.payload) {
                        on_event(i + 1, event);
                    }
                    State::Ground
                }
                (State::Osc, 0x1b) => State::OscEscape,
                (State::Osc, _) if self.payload.len() < MAX_OSC_LEN => {
                    self.payload.push(byte);
                    State::Osc
                }
                (State::Osc, _) => State::Ground,
                // Any other escape ends the OSC (like a terminal would) and starts anew.
                (State::OscEscape, b']') => {
                    self.payload.clear();
                    State::Osc
                }
                (State::OscEscape, _) => State::Ground,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(payload: &str) -> Option<ShellEvent> {
        ShellEvent::parse_osc(payload.as_bytes())
    }

    #[test]
    fn parses_prompt_markers() {
        assert_eq!(parse("133;A"), Some(ShellEvent::PromptStart));
        assert_eq!(parse("133;B"), Some(ShellEvent::CommandInputStart));
        assert_eq!(
            parse("133;D;127"),
            Some(ShellEvent::CommandEnd {
                exit_code: Some(127)
            })
        );
        assert_eq!(
            parse("133;D"),
            Some(ShellEvent::CommandEnd { exit_code: None })
        );
        assert_eq!(parse("133;Z"), None);
    }

    #[test]
    fn decodes_working_directory_urls() {
        assert_eq!(
            parse("7;file://mac.local/Users/me/My%20Stuff"),
            Some(ShellEvent::Cwd {
                host: "mac.local".into(),
                path: "/Users/me/My Stuff".into()
            })
        );
        assert_eq!(parse("7;http://x"), None);
        assert_eq!(parse("7;file://h/bad%zz"), None);
    }

    #[test]
    fn splits_metadata_at_the_first_equals_sign() {
        assert_eq!(
            parse("7777;a=b=c"),
            Some(ShellEvent::Meta {
                key: "a".into(),
                value: "b=c".into()
            })
        );
        assert_eq!(parse("7777;nothing"), None);
        assert_eq!(parse("7777;=x"), None);
    }

    #[test]
    fn scanner_reports_offsets_and_handles_split_sequences() {
        let mut scanner = OscScanner::default();
        let mut events = Vec::new();
        scanner.scan(b"hi\x1b]133;A\x07$ \x1b]13", |at, e| events.push((at, e)));
        scanner.scan(b"3;B\x1b\\ls", |at, e| events.push((at, e)));
        assert_eq!(
            events,
            vec![
                (10, ShellEvent::PromptStart),
                (5, ShellEvent::CommandInputStart)
            ]
        );
    }

    #[test]
    fn scanner_ignores_other_sequences() {
        let mut scanner = OscScanner::default();
        let mut count = 0;
        scanner.scan(
            b"\x1b[31mred\x1b]0;title\x07\x1b]8;;http://x\x1b\\",
            |_, _| count += 1,
        );
        assert_eq!(count, 0);
    }
}

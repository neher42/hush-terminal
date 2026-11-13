use std::fs;
use std::path::Path;

/// zsh stores bytes >= 0x83 "metafied": a 0x83 marker followed by the byte XOR 0x20.
const ZSH_META: u8 = 0x83;

const MAX_ENTRIES: usize = 10_000;

/// Oldest first, deduplicated (newest copy wins). Missing file = empty history.
pub fn read_zsh_history(path: &Path) -> Vec<String> {
    fs::read(path)
        .map(|bytes| parse_zsh_history(&bytes))
        .unwrap_or_default()
}

pub fn parse_zsh_history(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(&unmetafy(bytes)).into_owned();
    let mut entries: Vec<String> = Vec::new();
    let mut pending: Option<String> = None;

    for line in text.lines() {
        let line = match pending.take() {
            Some(mut previous) => {
                previous.push('\n');
                previous.push_str(line);
                previous
            }
            None => strip_extended_prefix(line).to_owned(),
        };
        // A trailing backslash means the command continues on the next line.
        if let Some(continued) = line.strip_suffix('\\') {
            pending = Some(continued.to_owned());
            continue;
        }
        if !line.trim().is_empty() {
            entries.push(line);
        }
    }
    if let Some(last) = pending.filter(|line| !line.trim().is_empty()) {
        entries.push(last);
    }
    dedupe_keep_latest(entries)
}

fn unmetafy(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut iter = bytes.iter();
    while let Some(&byte) = iter.next() {
        if byte == ZSH_META {
            if let Some(&next) = iter.next() {
                out.push(next ^ 0x20);
            }
        } else {
            out.push(byte);
        }
    }
    out
}

/// Extended history lines look like `: 1731491234:0;git status`.
fn strip_extended_prefix(line: &str) -> &str {
    let Some(rest) = line.strip_prefix(": ") else {
        return line;
    };
    let Some((meta, command)) = rest.split_once(';') else {
        return line;
    };
    let is_meta = meta
        .split(':')
        .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    if is_meta {
        command
    } else {
        line
    }
}

fn dedupe_keep_latest(entries: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut newest_first: Vec<String> = entries
        .into_iter()
        .rev()
        .filter(|entry| seen.insert(entry.clone()))
        .collect();
    newest_first.truncate(MAX_ENTRIES);
    newest_first.reverse();
    newest_first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_extended_lines() {
        let input = b": 1731491234:0;git status\nls -la\n: 1731491240:3;npm test\n";
        assert_eq!(
            parse_zsh_history(input),
            vec!["git status", "ls -la", "npm test"]
        );
    }

    #[test]
    fn joins_multiline_commands() {
        let input = b": 1731491234:0;for f in *; do\\\n  echo $f\\\ndone\n";
        assert_eq!(
            parse_zsh_history(input),
            vec!["for f in *; do\n  echo $f\ndone"]
        );
    }

    #[test]
    fn keeps_the_newest_duplicate() {
        let input = b"ls\ngit status\nls\n";
        assert_eq!(parse_zsh_history(input), vec!["git status", "ls"]);
    }

    #[test]
    fn decodes_metafied_bytes() {
        // "café" with é (0xC3 0xA9) stored as 0xC3 then META + (0xA9 ^ 0x20).
        let input = [
            b'e',
            b'c',
            b'h',
            b'o',
            b' ',
            b'c',
            b'a',
            b'f',
            0xC3,
            ZSH_META,
            0xA9 ^ 0x20,
            b'\n',
        ];
        assert_eq!(parse_zsh_history(&input), vec!["echo café"]);
    }

    #[test]
    fn leaves_colon_commands_alone() {
        assert_eq!(
            parse_zsh_history(b": not-a-timestamp;thing\n"),
            vec![": not-a-timestamp;thing"]
        );
    }
}

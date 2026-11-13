#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Plain,
    Command,
    UnknownCommand,
    Flag,
    String,
    Variable,
    Operator,
    Comment,
}

impl Style {
    /// ANSI palette index (8 = bright black), so colors follow the theme and
    /// zsh can match them.
    pub fn color(self) -> Option<u8> {
        match self {
            Style::Plain => None,
            Style::Command => Some(4),
            Style::UnknownCommand => Some(1),
            Style::Flag => Some(5),
            Style::String => Some(2),
            Style::Variable => Some(3),
            Style::Operator | Style::Comment => Some(8),
        }
    }
}

/// A styled byte range of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

const OPERATOR_CHARS: &str = "|&;<>()";
/// Words after which the next word is still a command, e.g. `sudo git push`.
const PREFIX_COMMANDS: &[&str] = &[
    "sudo", "time", "nohup", "exec", "command", "builtin", "env", "nice", "xargs", "watch",
];

fn is_operator(c: char) -> bool {
    OPERATOR_CHARS.contains(c)
}

/// Splits a command into styled spans. `is_known` returns `None` while the
/// available commands are unknown, which counts as known. A command that ends
/// at `cursor` is never marked unknown because it is still being typed.
pub fn highlight(
    text: &str,
    is_known: impl Fn(&str) -> Option<bool>,
    cursor: Option<usize>,
) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut i = 0;
    let mut command_position = true;

    while let Some(ch) = text[i..].chars().next() {
        if ch.is_whitespace() {
            if ch == '\n' {
                command_position = true;
            }
            i += ch.len_utf8();
            continue;
        }

        if ch == '#'
            && text[..i]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace)
        {
            let end = text[i..].find('\n').map_or(text.len(), |n| i + n);
            push(&mut spans, i, end, Style::Comment);
            i = end;
            continue;
        }

        if is_operator(ch) {
            let end = text[i..]
                .find(|c| !is_operator(c))
                .map_or(text.len(), |n| i + n);
            push(&mut spans, i, end, Style::Operator);
            if text[i..end].contains(['|', '&', ';', '(']) {
                command_position = true;
            }
            i = end;
            continue;
        }

        let end = word_end(text, i);
        let word = &text[i..end];
        if !command_position {
            highlight_argument(&mut spans, word, i);
        } else if is_assignment(word) {
            push(&mut spans, i, end, Style::Variable);
        } else {
            let name = unquote(word);
            let known = is_known(&name).unwrap_or(true);
            let still_typing = cursor == Some(end);
            let style = if known || still_typing || name.contains('/') {
                Style::Command
            } else {
                Style::UnknownCommand
            };
            push(&mut spans, i, end, style);
            command_position = PREFIX_COMMANDS.contains(&name.as_str());
        }
        i = end;
    }
    spans
}

fn push(spans: &mut Vec<Span>, start: usize, end: usize, style: Style) {
    if end > start {
        spans.push(Span { start, end, style });
    }
}

/// Marks quoted sections and $variables inside the word; the rest stays plain.
fn highlight_argument(spans: &mut Vec<Span>, word: &str, offset: usize) {
    if word.starts_with('-') {
        push(spans, offset, offset + word.len(), Style::Flag);
        return;
    }
    let bytes = word.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            quote @ (b'"' | b'\'') => {
                let end = word[i + 1..]
                    .find(quote as char)
                    .map_or(word.len(), |n| i + n + 2);
                push(spans, offset + i, offset + end, Style::String);
                i = end;
            }
            b'$' => {
                let end = i + variable_len(&word[i..]);
                push(spans, offset + i, offset + end, Style::Variable);
                i = end;
            }
            _ => i += 1,
        }
    }
}

/// Length of the `$NAME`, `${...}` or `$?` style reference at the start of `s`.
fn variable_len(s: &str) -> usize {
    let rest = &s[1..];
    match rest.chars().next() {
        Some('{') => 1 + rest.find('}').map_or(rest.len(), |n| n + 1),
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            1 + rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len())
        }
        Some(c) if c.is_ascii_digit() || "?!#@*$-".contains(c) => 2,
        _ => 1,
    }
}

fn is_assignment(word: &str) -> bool {
    let name_len = word
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(word.len());
    name_len > 0 && !word.as_bytes()[0].is_ascii_digit() && word[name_len..].starts_with('=')
}

fn word_end(text: &str, start: usize) -> usize {
    let mut i = start;
    while let Some(ch) = text[i..].chars().next() {
        if ch.is_whitespace() || is_operator(ch) {
            break;
        }
        i = match ch {
            '\\' => text[i + 1..]
                .chars()
                .next()
                .map_or(i + 1, |c| i + 1 + c.len_utf8()),
            '"' | '\'' => text[i + 1..].find(ch).map_or(text.len(), |n| i + n + 2),
            _ => i + ch.len_utf8(),
        };
    }
    i
}

/// Characters a JavaScript regex `.` does not match.
fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Drops backslash escapes and one pair of surrounding quotes.
pub fn unquote(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut chars = word.chars().peekable();
    while let Some(c) = chars.next() {
        match chars.peek() {
            Some(&next) if c == '\\' && !is_line_terminator(next) => {
                out.push(next);
                chars.next();
            }
            _ => out.push(c),
        }
    }
    let bytes = out.as_bytes();
    let quoted = bytes.len() >= 2
        && matches!(bytes[0], b'"' | b'\'')
        && bytes[0] == bytes[bytes.len() - 1]
        && !out[1..out.len() - 1].contains(is_line_terminator);
    if quoted {
        out[1..out.len() - 1].to_string()
    } else {
        out
    }
}

/// zsh `region_highlight` entries for the colored spans, one per line. zsh
/// counts characters, so the byte offsets are converted to char offsets.
pub fn zsh_highlights(text: &str, spans: &[Span]) -> String {
    let chars = |offset: usize| text.char_indices().take_while(|&(i, _)| i < offset).count();
    spans
        .iter()
        .filter_map(|span| {
            let color = span.style.color()?;
            Some(format!(
                "{} {} fg={color}",
                chars(span.start),
                chars(span.end)
            ))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<bool> {
        Some(["git", "ls", "grep", "sudo", "echo", "npm"].contains(&name))
    }

    fn styles(text: &str, cursor: Option<usize>) -> Vec<(&str, Style)> {
        highlight(text, lookup, cursor)
            .into_iter()
            .map(|s| (&text[s.start..s.end], s.style))
            .collect()
    }

    #[test]
    fn marks_the_command_flags_strings_and_variables() {
        assert_eq!(
            styles("git commit -m \"fix it\" $HOME", None),
            [
                ("git", Style::Command),
                ("-m", Style::Flag),
                ("\"fix it\"", Style::String),
                ("$HOME", Style::Variable),
            ]
        );
    }

    #[test]
    fn finds_commands_after_pipes_and_prefix_commands() {
        assert_eq!(
            styles("sudo ls | grep x && nope", None),
            [
                ("sudo", Style::Command),
                ("ls", Style::Command),
                ("|", Style::Operator),
                ("grep", Style::Command),
                ("&&", Style::Operator),
                ("nope", Style::UnknownCommand),
            ]
        );
    }

    #[test]
    fn does_not_flag_a_command_while_it_is_still_being_typed() {
        assert_eq!(styles("gi", Some(2)), [("gi", Style::Command)]);
        assert_eq!(styles("gi x", Some(2)), [("gi", Style::Command)]);
        assert_eq!(styles("gi x", Some(4)), [("gi", Style::UnknownCommand)]);
    }

    #[test]
    fn treats_unknown_availability_as_known() {
        let spans = highlight("anything", |_| None, None);
        assert_eq!(
            spans.iter().map(|s| s.style).collect::<Vec<_>>(),
            [Style::Command]
        );
    }

    #[test]
    fn handles_assignments_paths_comments_and_new_lines() {
        assert_eq!(
            styles("FOO=1 ./run.sh # go\necho hi", None),
            [
                ("FOO=1", Style::Variable),
                ("./run.sh", Style::Command),
                ("# go", Style::Comment),
                ("echo", Style::Command),
            ]
        );
    }

    #[test]
    fn converts_spans_to_zsh_highlights_in_characters_not_bytes() {
        let text = "echo 😀 | grep x";
        let spans = [
            Span {
                start: 0,
                end: 4,
                style: Style::Command,
            },
            Span {
                start: 10,
                end: 11,
                style: Style::Operator,
            },
            Span {
                start: 12,
                end: 16,
                style: Style::Command,
            },
        ];
        assert_eq!(
            zsh_highlights(text, &spans),
            "0 4 fg=4\n7 8 fg=8\n9 13 fg=4"
        );
    }

    #[test]
    fn highlights_multibyte_text_with_char_offsets_for_zsh() {
        let text = "echo ä 😀 | grep x";
        let spans = highlight(text, lookup, None);
        assert_eq!(
            styles(text, None),
            [
                ("echo", Style::Command),
                ("|", Style::Operator),
                ("grep", Style::Command),
            ]
        );
        assert_eq!(
            zsh_highlights(text, &spans),
            "0 4 fg=4\n9 10 fg=8\n11 15 fg=4"
        );
    }

    #[test]
    fn steps_over_escaped_and_quoted_multibyte_chars() {
        assert_eq!(
            styles("echo \\ä \"ö\" $Ü ${ä}", None),
            [
                ("echo", Style::Command),
                ("\"ö\"", Style::String),
                ("$", Style::Variable),
                ("${ä}", Style::Variable),
            ]
        );
    }

    #[test]
    fn unquotes_words() {
        assert_eq!(unquote("'ä b'"), "ä b");
        assert_eq!(unquote("a\\ä\\ b"), "aä b");
        assert_eq!(unquote("\"git\""), "git");
        assert_eq!(unquote("'a\nb'"), "'a\nb'");
    }
}

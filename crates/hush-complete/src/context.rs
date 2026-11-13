#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionContext {
    /// The word before the cursor as typed, with quotes and escapes.
    pub raw_word: String,
    /// The same word with quotes and escapes removed.
    pub word: String,
    /// Byte offset where the word starts (the cursor when there is no word yet).
    pub word_start: usize,
    /// Earlier words of the current command, unquoted.
    pub args: Vec<String>,
}

/// Looks at the text before `cursor` (a byte offset, rounded down to a char boundary).
pub fn completion_context(text: &str, cursor: usize) -> CompletionContext {
    let cursor = text.floor_char_boundary(cursor);
    let before = &text[..cursor];
    let ends_in_space = before
        .chars()
        .next_back()
        .is_none_or(|ch| ch.is_whitespace() || matches!(ch, '|' | ';' | '&' | '('));

    let words = split_words(before);
    let segment_start = words.iter().rposition(|w| w.separator).map_or(0, |i| i + 1);
    let mut segment: Vec<Word> = words.into_iter().skip(segment_start).collect();
    let current = if ends_in_space { None } else { segment.pop() };

    CompletionContext {
        raw_word: current.as_ref().map_or("", |w| w.raw).to_owned(),
        word_start: current.as_ref().map_or(cursor, |w| w.start),
        word: current.map(|w| w.value).unwrap_or_default(),
        args: segment.into_iter().map(|w| w.value).collect(),
    }
}

struct Word<'a> {
    raw: &'a str,
    value: String,
    start: usize,
    separator: bool,
}

fn is_separator(ch: char) -> bool {
    matches!(ch, '\n' | ';' | '|' | '&' | '(')
}

fn split_words(text: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        if is_separator(ch) {
            chars.next();
            while chars.next_if(|&(_, c)| c == '|' || c == '&').is_some() {}
            let end = chars.peek().map_or(text.len(), |&(i, _)| i);
            words.push(Word {
                raw: &text[start..end],
                value: String::new(),
                start,
                separator: true,
            });
            continue;
        }
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        let mut value = String::new();
        while let Some((_, c)) = chars.next_if(|&(_, c)| !c.is_whitespace() && !is_separator(c)) {
            match c {
                '\\' => value.push(chars.next().map_or('\\', |(_, escaped)| escaped)),
                '"' | '\'' => {
                    for (_, inner) in chars.by_ref() {
                        if inner == c {
                            break;
                        }
                        value.push(inner);
                    }
                }
                _ => value.push(c),
            }
        }
        let end = chars.peek().map_or(text.len(), |&(i, _)| i);
        words.push(Word {
            raw: &text[start..end],
            value,
            start,
            separator: false,
        });
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(raw_word: &str, word: &str, word_start: usize, args: &[&str]) -> CompletionContext {
        CompletionContext {
            raw_word: raw_word.into(),
            word: word.into(),
            word_start,
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn finds_the_word_at_the_cursor_and_earlier_arguments() {
        assert_eq!(
            completion_context("git checkout ma", 15),
            ctx("ma", "ma", 13, &["git", "checkout"])
        );
        assert_eq!(completion_context("ls | gr", 7), ctx("gr", "gr", 5, &[]));
        assert_eq!(completion_context("cd ", 3), ctx("", "", 3, &["cd"]));
        assert_eq!(completion_context("cd Work\\ N", 10).word, "Work N");
    }

    #[test]
    fn unquotes_words_and_starts_over_after_separators() {
        assert_eq!(
            completion_context("echo 'a b' && cat \"x y", 22),
            ctx("\"x y", "x y", 18, &["cat"])
        );
        assert_eq!(completion_context("ls &&", 5), ctx("", "", 5, &[]));
    }

    #[test]
    fn uses_byte_offsets_for_multibyte_text() {
        let text = "cd 日本/ä";
        assert_eq!(
            completion_context(text, text.len()),
            ctx("日本/ä", "日本/ä", 3, &["cd"])
        );
        assert_eq!(
            completion_context("ls é\\ ü x", 9),
            ctx("é\\ ü", "é ü", 3, &["ls"])
        );
    }

    #[test]
    fn rounds_a_cursor_inside_a_char_down() {
        // Byte 4 is inside "é" (bytes 3..5).
        assert_eq!(completion_context("ls é", 4), ctx("", "", 3, &["ls"]));
        assert_eq!(completion_context("ls", 99), ctx("ls", "ls", 0, &[]));
    }
}

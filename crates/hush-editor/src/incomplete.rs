/// True when the command can't run yet: an open quote, or a trailing pipe,
/// `&&`, `||` or line continuation backslash.
pub fn is_incomplete(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut quote = None;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        match quote {
            Some(q) if ch == '\\' && q == '"' => i += 1,
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '\\' => i += 1,
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '#' && (i == 0 || chars[i - 1].is_whitespace()) => {
                match chars[i..].iter().position(|&c| c == '\n') {
                    Some(n) => i += n,
                    None => break,
                }
            }
            None => {}
        }
        i += 1;
    }
    if quote.is_some() {
        return true;
    }
    let trimmed = text.trim_end_matches([' ', '\t']);
    let continued = trimmed
        .strip_suffix('\\')
        .is_some_and(|rest| !rest.ends_with('\\'));
    continued || trimmed.ends_with("&&") || trimmed.ends_with('|')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_unclosed_quotes() {
        assert!(is_incomplete("echo \"hi"));
        assert!(is_incomplete("echo 'it\"s"));
        assert!(!is_incomplete("echo \"a \\\" b\""));
    }

    #[test]
    fn detects_continuation_at_the_end_of_the_line() {
        assert!(is_incomplete("ls |"));
        assert!(is_incomplete("make &&  "));
        assert!(is_incomplete("echo a \\"));
        assert!(!is_incomplete("echo a \\\\"));
    }

    #[test]
    fn ignores_quotes_inside_comments() {
        assert!(!is_incomplete("ls # don't"));
    }

    #[test]
    fn accepts_complete_commands() {
        assert!(!is_incomplete("git commit -m \"done\""));
        assert!(!is_incomplete(""));
    }

    #[test]
    fn handles_multibyte_text() {
        assert!(is_incomplete("echo \"ä😀"));
        assert!(is_incomplete("echo ä😀 \\"));
        assert!(!is_incomplete("echo \\ä 'ö' # ü'"));
    }
}

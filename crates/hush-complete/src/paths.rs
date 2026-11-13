//! Helpers for the plain `/`-separated path strings the shell reports.

use std::borrow::Cow;

pub fn join_path(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

pub fn parent_path(path: &str) -> &str {
    if path == "/" {
        return "/";
    }
    match path.trim_end_matches('/').rfind('/') {
        Some(i) if i > 0 => &path[..i],
        _ => "/",
    }
}

pub fn base_name(path: &str) -> &str {
    if path == "/" {
        return "/";
    }
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

pub fn is_inside(path: &str, dir: &str) -> bool {
    relative_to(path, dir).is_some()
}

/// The part of `path` below `dir`: empty when they are equal, `None` when `path` is outside.
fn relative_to<'a>(path: &'a str, dir: &str) -> Option<&'a str> {
    if path == dir {
        return Some("");
    }
    let rest = path.strip_prefix(dir)?;
    if dir.ends_with('/') {
        Some(rest)
    } else {
        rest.strip_prefix('/')
    }
}

pub fn display_path(path: &str, home: &str) -> String {
    if home.is_empty() {
        return path.to_owned();
    }
    match relative_to(path, home) {
        Some("") => "~".to_owned(),
        Some(rest) => format!("~/{rest}"),
        None => path.to_owned(),
    }
}

pub fn expand_tilde<'a>(path: &'a str, home: &str) -> Cow<'a, str> {
    if path == "~" {
        Cow::Owned(home.to_owned())
    } else if let Some(rest) = path.strip_prefix("~/") {
        Cow::Owned(join_path(home, rest))
    } else {
        Cow::Borrowed(path)
    }
}

/// An absolute path with `~`, `.` and `..` resolved. Does not touch the filesystem.
pub fn resolve_path(path: &str, cwd: &str, home: &str) -> String {
    let expanded = expand_tilde(path, home);
    let absolute = if expanded.starts_with('/') {
        expanded
    } else {
        Cow::Owned(join_path(cwd, &expanded))
    };
    let mut parts = Vec::new();
    for part in absolute.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}

const SHELL_SPECIAL: &str = "'\"\\$`!&*()|;<>?[]{}#^,=";

pub fn escape_shell_word(word: &str) -> String {
    let mut escaped = String::with_capacity(word.len());
    for ch in word.chars() {
        if ch.is_whitespace() || SHELL_SPECIAL.contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Relative inside cwd, `~/...` inside home, absolute otherwise.
pub fn shell_path(path: &str, cwd: &str, home: &str) -> String {
    if path == cwd {
        return ".".to_owned();
    }
    if cwd != "/" {
        if let Some(rest) = relative_to(path, cwd) {
            return escape_shell_word(rest);
        }
    }
    if !home.is_empty() {
        match relative_to(path, home) {
            Some("") => return "~".to_owned(),
            Some(rest) => return format!("~/{}", escape_shell_word(rest)),
            None => {}
        }
    }
    escape_shell_word(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/Users/me";

    #[test]
    fn shows_home_as_a_tilde() {
        assert_eq!(display_path("/Users/me/src", HOME), "~/src");
        assert_eq!(display_path("/Users/me", HOME), "~");
        assert_eq!(display_path("/Users/meg", HOME), "/Users/meg");
    }

    #[test]
    fn resolves_relative_home_and_parent_paths() {
        assert_eq!(resolve_path("../b", "/Users/me/a", HOME), "/Users/me/b");
        assert_eq!(resolve_path("~/x/./y", "/tmp", HOME), "/Users/me/x/y");
        assert_eq!(resolve_path("/", "/tmp", HOME), "/");
    }

    #[test]
    fn escapes_special_characters_for_the_shell() {
        assert_eq!(escape_shell_word("My Docs (old)"), "My\\ Docs\\ \\(old\\)");
    }

    #[test]
    fn writes_the_shortest_shell_path() {
        assert_eq!(
            shell_path("/Users/me/src/app", "/Users/me/src", HOME),
            "app"
        );
        assert_eq!(
            shell_path("/Users/me/Movies", "/Users/me/src", HOME),
            "~/Movies"
        );
        assert_eq!(shell_path("/Users/me/src", "/Users/me/src", HOME), ".");
        assert_eq!(shell_path("/etc/hosts", "/Users/me", HOME), "/etc/hosts");
    }

    #[test]
    fn splits_names_and_parents() {
        assert_eq!(base_name("/a/b/"), "b");
        assert_eq!(parent_path("/a/b"), "/a");
        assert_eq!(parent_path("/a"), "/");
    }

    #[test]
    fn handles_multibyte_names() {
        assert_eq!(base_name("/Users/me/Größe/"), "Größe");
        assert_eq!(parent_path("/Users/me/Größe/日本"), "/Users/me/Größe");
        assert_eq!(shell_path("/Users/me/日本 語", "/tmp", HOME), "~/日本\\ 語");
        assert_eq!(escape_shell_word("café\u{3000}x"), "café\\\u{3000}x");
    }
}

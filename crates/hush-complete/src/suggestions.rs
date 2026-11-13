use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use regex::{Captures, Regex};

use crate::paths::resolve_path;
use crate::sources::{CompletionSources, Place};

static PLAIN_CD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*cd\s+((?:\\.|'[^']*'|"[^"$`]*"|[^\s;&|<>()`$'"])+)\s*$"#).unwrap()
});

static QUOTING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\\(.)|'([^']*)'|"([^"]*)""#).unwrap());

/// The folder of a plain `cd <folder>` command, unquoted; `None` for anything else.
pub fn cd_target(command: &str) -> Option<String> {
    let raw = PLAIN_CD.captures(command)?.get(1)?.as_str();
    if raw == "-" {
        return None;
    }
    let unquoted = QUOTING.replace_all(raw, |caps: &Captures| {
        (1..=3)
            .find_map(|group| caps.get(group))
            .map_or("", |m| m.as_str())
            .to_owned()
    });
    Some(unquoted.into_owned())
}

/// Whether a history entry makes sense here: a `cd` only when its folder exists.
/// `is_dir` returns `None` while the answer is unknown, which holds the `cd` back.
pub fn fits_here(entry: &str, place: Place<'_>, is_dir: impl Fn(&str) -> Option<bool>) -> bool {
    match cd_target(entry) {
        None => true,
        Some(target) => is_dir(&resolve_path(&target, place.cwd, place.home)) == Some(true),
    }
}

/// Remembers which paths are folders until [`FolderCheck::clear`], e.g. after a command ran.
#[derive(Debug, Default)]
pub struct FolderCheck {
    known: Mutex<HashMap<String, bool>>,
}

impl FolderCheck {
    /// The remembered answer, without touching the filesystem.
    pub fn known(&self, path: &str) -> Option<bool> {
        self.lock().get(path).copied()
    }

    /// The remembered answer, or asks `sources` and remembers it.
    pub fn is_dir(&self, path: &str, sources: &dyn CompletionSources) -> bool {
        if let Some(known) = self.known(path) {
            return known;
        }
        let is_dir = sources.is_dir(path);
        self.lock().insert(path.to_owned(), is_dir);
        is_dir
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, bool>> {
        self.known.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::fake::FakeSources;

    #[test]
    fn finds_the_folder_of_a_plain_cd() {
        assert_eq!(cd_target("cd starliner").as_deref(), Some("starliner"));
        assert_eq!(cd_target("cd ~/My\\ Docs/").as_deref(), Some("~/My Docs/"));
        assert_eq!(cd_target("cd 'two words'").as_deref(), Some("two words"));
        assert_eq!(
            cd_target("cd \"Größe\"/日本").as_deref(),
            Some("Größe/日本")
        );
    }

    #[test]
    fn ignores_everything_else() {
        assert_eq!(cd_target("cd"), None);
        assert_eq!(cd_target("cd -"), None);
        assert_eq!(cd_target("cd app && npm test"), None);
        assert_eq!(cd_target("cd $(git rev-parse --show-toplevel)"), None);
        assert_eq!(cd_target("ls starliner"), None);
    }

    fn fits(entry: &str, cwd: &str) -> bool {
        let folders: HashSet<&str> = ["/Users/me/starliner", "/Users/me/日本"].into();
        let place = Place {
            cwd,
            home: "/Users/me",
        };
        fits_here(entry, place, |path| Some(folders.contains(path)))
    }

    #[test]
    fn only_suggests_cd_into_folders_that_exist_from_here() {
        assert!(fits("cd starliner", "/Users/me"));
        assert!(!fits("cd starliner", "/Users/me/workspace"));
        assert!(fits("cd ~/starliner", "/Users/me/workspace"));
        assert!(fits("cd ../日本", "/Users/me/workspace"));
    }

    #[test]
    fn suggests_other_commands_anywhere() {
        assert!(fits("git status", "/tmp"));
    }

    #[test]
    fn holds_back_a_cd_while_its_folder_is_still_being_checked() {
        let place = Place {
            cwd: "/",
            home: "/",
        };
        assert!(!fits_here("cd x", place, |_| None));
    }

    #[test]
    fn folder_check_remembers_until_cleared() {
        let sources = FakeSources::default().with_dir("/a", Vec::new());
        let folders = FolderCheck::default();
        assert_eq!(folders.known("/a"), None);
        assert!(folders.is_dir("/a", &sources));
        assert!(!folders.is_dir("/b", &sources));
        assert!(folders.is_dir("/a", &sources));
        assert_eq!(sources.is_dir_calls.get(), 2);
        assert_eq!(folders.known("/b"), Some(false));
        folders.clear();
        assert_eq!(folders.known("/a"), None);
    }
}

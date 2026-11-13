use std::collections::HashSet;

const MAX_ENTRIES: usize = 20_000;

/// The usual number of results for [`HistoryStore::search`].
pub const SEARCH_LIMIT: usize = 50;

#[derive(Debug, Clone, Default)]
pub struct HistoryStore {
    /// Oldest first, no duplicates.
    entries: Vec<String>,
}

impl HistoryStore {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adds entries read from the history file (oldest first) as older than this session's.
    pub fn load(&mut self, from_file: Vec<String>) {
        let session: HashSet<&str> = self.entries.iter().map(String::as_str).collect();
        let mut entries: Vec<String> = from_file
            .into_iter()
            .filter(|entry| !session.contains(entry.as_str()))
            .collect();
        entries.append(&mut self.entries);
        let excess = entries.len().saturating_sub(MAX_ENTRIES);
        entries.drain(..excess);
        self.entries = entries;
    }

    /// Records a command that ran, moving an earlier copy to the newest position.
    pub fn add(&mut self, command: &str) {
        let entry = command.trim_end();
        if entry.is_empty() {
            return;
        }
        if let Some(existing) = self.entries.iter().rposition(|e| e == entry) {
            self.entries.remove(existing);
        }
        self.entries.push(entry.to_owned());
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }

    /// The newest longer entry starting with `prefix` that `accept` allows.
    pub fn suggest(&self, prefix: &str, accept: impl Fn(&str) -> bool) -> Option<&str> {
        if prefix.trim().is_empty() || prefix.contains('\n') {
            return None;
        }
        self.entries
            .iter()
            .rev()
            .map(String::as_str)
            .find(|entry| entry.len() > prefix.len() && entry.starts_with(prefix) && accept(entry))
    }

    /// Newest first, ignoring case: substring matches, then subsequence matches ("gco" finds "git checkout").
    pub fn search(&self, query: &str, limit: usize) -> Vec<&str> {
        let query = query.trim().to_lowercase();
        let mut contains = Vec::new();
        let mut fuzzy = Vec::new();
        for entry in self.entries.iter().rev() {
            if contains.len() >= limit {
                break;
            }
            let lower = entry.to_lowercase();
            if query.is_empty() || lower.contains(&query) {
                contains.push(entry.as_str());
            } else if is_subsequence(&query, &lower) {
                fuzzy.push(entry.as_str());
            }
        }
        contains.extend(fuzzy);
        contains.truncate(limit);
        contains
    }

    /// Entries starting with `prefix`, newest first.
    pub fn walk_back<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> {
        self.entries
            .iter()
            .rev()
            .map(String::as_str)
            .filter(move |entry| entry.starts_with(prefix))
    }
}

/// Up and down through matching history, like zsh's up-line-or-beginning-search.
#[derive(Debug, Clone)]
pub struct HistoryBrowser {
    typed: String,
    matches: Vec<String>,
    /// Position in `matches`; `None` while showing what was typed.
    index: Option<usize>,
}

impl HistoryBrowser {
    pub fn new(store: &HistoryStore, typed: &str) -> Self {
        let matches = store
            .walk_back(typed)
            .filter(|&entry| entry != typed)
            .map(str::to_owned)
            .collect();
        Self {
            typed: typed.to_owned(),
            matches,
            index: None,
        }
    }

    pub fn older(&mut self) -> Option<&str> {
        let next = self.index.map_or(0, |i| i + 1);
        let entry = self.matches.get(next)?;
        self.index = Some(next);
        Some(entry)
    }

    /// Ends with what was typed, then `None`.
    pub fn newer(&mut self) -> Option<&str> {
        self.index = self.index?.checked_sub(1);
        Some(match self.index {
            Some(i) => &self.matches[i],
            None => &self.typed,
        })
    }
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut rest = haystack.chars();
    needle.chars().all(|ch| rest.any(|c| c == ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(entries: &[&str]) -> HistoryStore {
        let mut store = HistoryStore::default();
        store.load(entries.iter().map(|e| e.to_string()).collect());
        store
    }

    #[test]
    fn suggests_the_newest_continuation() {
        let s = store(&["git checkout main", "git status", "git checkout -b fix"]);
        assert_eq!(s.suggest("git ch", |_| true), Some("git checkout -b fix"));
        assert_eq!(s.suggest("git status", |_| true), None);
        assert_eq!(s.suggest("", |_| true), None);
    }

    #[test]
    fn skips_entries_the_filter_rejects_falling_back_to_older_ones() {
        let s = store(&["cd workspace", "cd starliner"]);
        assert_eq!(
            s.suggest("cd ", |entry| entry != "cd starliner"),
            Some("cd workspace")
        );
    }

    #[test]
    fn moves_re_run_commands_to_the_newest_position() {
        let mut s = store(&["ls", "pwd"]);
        s.add("ls");
        assert_eq!(s.search("", SEARCH_LIMIT), ["ls", "pwd"]);
    }

    #[test]
    fn keeps_session_commands_newer_than_ones_loaded_later() {
        let mut s = HistoryStore::default();
        s.add("npm test");
        s.load(vec!["ls".into(), "npm test".into()]);
        assert_eq!(s.search("", SEARCH_LIMIT), ["npm test", "ls"]);
    }

    #[test]
    fn searches_by_substring_first_then_by_subsequence() {
        let s = store(&["git checkout main", "echo gco", "grep -c o"]);
        assert_eq!(
            s.search("gco", SEARCH_LIMIT),
            ["echo gco", "grep -c o", "git checkout main"]
        );
    }

    #[test]
    fn ignores_blank_commands_and_trailing_space() {
        let mut s = HistoryStore::default();
        s.add("   ");
        s.add("ls  \n");
        assert_eq!(s.search("", SEARCH_LIMIT), ["ls"]);
    }

    #[test]
    fn handles_multibyte_entries() {
        let s = store(&["echo café", "cd ~/日本"]);
        assert_eq!(s.suggest("echo caf", |_| true), Some("echo café"));
        assert_eq!(s.suggest("cd ~/日", |_| true), Some("cd ~/日本"));
        assert_eq!(s.search("CAFÉ", SEARCH_LIMIT), ["echo café"]);
        assert_eq!(s.search("c日", SEARCH_LIMIT), ["cd ~/日本"]);
    }

    #[test]
    fn walks_matching_entries_and_returns_to_what_was_typed() {
        let mut b = HistoryBrowser::new(&store(&["git pull", "ls", "git push"]), "git");
        assert_eq!(b.older(), Some("git push"));
        assert_eq!(b.older(), Some("git pull"));
        assert_eq!(b.older(), None);
        assert_eq!(b.newer(), Some("git push"));
        assert_eq!(b.newer(), Some("git"));
        assert_eq!(b.newer(), None);
    }
}

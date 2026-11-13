//! Which rows the folder tree shows: everything from home (or /) down to the
//! current folder is open, plus folders the user opened.

use std::collections::{HashMap, HashSet};

use hush_complete::paths::{is_inside, join_path};
use hush_core::files::DirEntry;

/// Shown in a lighter color: big folders nobody browses.
const DIMMED: [&str; 7] = [
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".venv",
    "__pycache__",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
    pub is_open: bool,
}

impl Row {
    pub fn is_dimmed(&self) -> bool {
        DIMMED.contains(&self.name.as_str())
    }
}

#[derive(Default)]
pub struct Tree {
    pub cwd: String,
    pub home: String,
    listings: HashMap<String, Vec<DirEntry>>,
    expanded: HashSet<String>,
    collapsed: HashSet<String>,
}

impl Tree {
    pub fn new(cwd: &str, home: &str) -> Self {
        Self {
            cwd: cwd.to_owned(),
            home: home.to_owned(),
            ..Self::default()
        }
    }

    pub fn is_open(&self, path: &str) -> bool {
        if self.collapsed.contains(path) {
            return false;
        }
        self.expanded.contains(path) || is_inside(&self.cwd, path)
    }

    pub fn toggle(&mut self, path: &str) {
        if self.is_open(path) {
            self.expanded.remove(path);
            if is_inside(&self.cwd, path) {
                self.collapsed.insert(path.to_owned());
            }
        } else {
            self.collapsed.remove(path);
            self.expanded.insert(path.to_owned());
        }
    }

    pub fn set_listing(&mut self, path: String, entries: Vec<DirEntry>) {
        self.listings.insert(path, entries);
    }

    pub fn set_cwd(&mut self, cwd: &str) {
        self.cwd = cwd.to_owned();
    }

    /// Open folders whose listing is shown (to reload after a command).
    pub fn open_listings(&self) -> Vec<String> {
        self.listings
            .keys()
            .filter(|path| self.is_open(path))
            .cloned()
            .collect()
    }

    /// The rows to show, and the open folders whose listing isn't loaded yet.
    pub fn rows(&self) -> (Vec<Row>, Vec<String>) {
        let root = if !self.home.is_empty() && is_inside(&self.cwd, &self.home) {
            self.home.clone()
        } else {
            "/".to_owned()
        };
        let name = if root == self.home { "~" } else { "/" };
        let mut rows = Vec::new();
        let mut missing = Vec::new();
        self.walk(&root, name, 0, &mut rows, &mut missing);
        (rows, missing)
    }

    fn walk(
        &self,
        path: &str,
        name: &str,
        depth: usize,
        rows: &mut Vec<Row>,
        missing: &mut Vec<String>,
    ) {
        let is_open = self.is_open(path);
        rows.push(Row {
            path: path.to_owned(),
            name: name.to_owned(),
            depth,
            is_dir: true,
            is_open,
        });
        if !is_open {
            return;
        }
        let Some(entries) = self.listings.get(path) else {
            missing.push(path.to_owned());
            return;
        };
        let show_files = path == self.cwd || self.expanded.contains(path);
        for entry in entries.iter().filter(|e| !e.name.starts_with('.')) {
            let child = join_path(path, &entry.name);
            if entry.is_dir {
                self.walk(&child, &entry.name, depth + 1, rows, missing);
            } else if show_files {
                rows.push(Row {
                    path: child,
                    name: entry.name.clone(),
                    depth: depth + 1,
                    is_dir: false,
                    is_open: false,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(names: &[&str]) -> Vec<DirEntry> {
        names
            .iter()
            .map(|name| DirEntry {
                name: name.trim_end_matches('/').to_owned(),
                is_dir: name.ends_with('/'),
                is_symlink: false,
            })
            .collect()
    }

    fn names(tree: &Tree) -> Vec<String> {
        tree.rows()
            .0
            .iter()
            .map(|r| format!("{}{}", "  ".repeat(r.depth), r.name))
            .collect()
    }

    #[test]
    fn opens_the_way_to_the_current_folder() {
        let mut tree = Tree::new("/h/code", "/h");
        assert_eq!(tree.rows().1, ["/h"]);
        tree.set_listing(
            "/h".into(),
            entries(&["code/", "docs/", "notes.txt", ".hidden/"]),
        );
        assert_eq!(tree.rows().1, ["/h/code"]);
        tree.set_listing("/h/code".into(), entries(&["src/", "main.rs"]));
        assert_eq!(
            names(&tree),
            ["~", "  code", "    src", "    main.rs", "  docs"]
        );
        assert!(tree.rows().1.is_empty());
    }

    #[test]
    fn toggling_overrides_the_default() {
        let mut tree = Tree::new("/h/code", "/h");
        tree.set_listing("/h".into(), entries(&["code/", "docs/"]));
        tree.set_listing("/h/code".into(), entries(&[]));
        tree.toggle("/h/code");
        assert!(!tree.is_open("/h/code"));
        tree.toggle("/h/docs");
        assert_eq!(tree.rows().1, ["/h/docs"]);
        tree.set_listing("/h/docs".into(), entries(&["a.md"]));
        assert_eq!(names(&tree), ["~", "  code", "  docs", "    a.md"]);
    }

    #[test]
    fn starts_at_the_root_outside_home() {
        let tree = Tree::new("/tmp", "/h");
        assert_eq!(tree.rows().0[0].name, "/");
    }
}

use std::cell::Cell;
use std::collections::HashMap;
use std::io;

use crate::{CompletionSources, DirEntry};

/// In-memory sources: folders are the keys of `tree`.
#[derive(Default)]
pub struct FakeSources {
    pub tree: HashMap<String, Vec<DirEntry>>,
    pub branches: Vec<String>,
    pub executables: Vec<String>,
    pub home: Option<String>,
    pub is_dir_calls: Cell<usize>,
}

impl FakeSources {
    pub fn with_dir(mut self, path: &str, entries: Vec<DirEntry>) -> Self {
        self.tree.insert(path.to_owned(), entries);
        self
    }
}

pub fn dir(name: &str) -> DirEntry {
    DirEntry {
        name: name.to_owned(),
        is_dir: true,
        is_symlink: false,
    }
}

pub fn file(name: &str) -> DirEntry {
    DirEntry {
        name: name.to_owned(),
        is_dir: false,
        is_symlink: false,
    }
}

impl CompletionSources for FakeSources {
    fn list_dir(&self, path: &str) -> io::Result<Vec<DirEntry>> {
        self.tree
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn is_dir(&self, path: &str) -> bool {
        self.is_dir_calls.set(self.is_dir_calls.get() + 1);
        self.tree.contains_key(path)
    }

    fn executables(&self, _path_var: &str) -> Vec<String> {
        self.executables.clone()
    }

    fn home_dir(&self) -> Option<String> {
        self.home.clone()
    }

    fn git_branches(&self, _cwd: &str) -> Vec<String> {
        self.branches.clone()
    }
}

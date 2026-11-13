use std::io;
use std::path::Path;

pub use hush_core::files::DirEntry;

/// Everything completion and suggestions need from the outside world.
pub trait CompletionSources {
    /// Folders first, as `hush_core::files::list_dir` sorts them.
    fn list_dir(&self, path: &str) -> io::Result<Vec<DirEntry>>;
    fn is_dir(&self, path: &str) -> bool;
    /// Executable names found in a `PATH`-style list of folders.
    fn executables(&self, path_var: &str) -> Vec<String>;
    fn home_dir(&self) -> Option<String>;
    /// Local branch names of the repository at `cwd`; empty outside a repository.
    fn git_branches(&self, cwd: &str) -> Vec<String>;
}

/// The real filesystem, via `hush-core`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemSources;

impl CompletionSources for SystemSources {
    fn list_dir(&self, path: &str) -> io::Result<Vec<DirEntry>> {
        hush_core::files::list_dir(Path::new(path)).map_err(io::Error::other)
    }

    fn is_dir(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }

    fn executables(&self, path_var: &str) -> Vec<String> {
        hush_core::executables::list_executables(path_var)
    }

    fn home_dir(&self) -> Option<String> {
        std::env::home_dir().map(|path| path.to_string_lossy().into_owned())
    }

    fn git_branches(&self, cwd: &str) -> Vec<String> {
        hush_core::git::branches(Path::new(cwd))
    }
}

/// Where the shell is: relative paths resolve against `cwd`, `~` against `home`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place<'a> {
    pub cwd: &'a str,
    pub home: &'a str,
}

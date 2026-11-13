use std::collections::BTreeSet;

use crate::sources::CompletionSources;
use crate::specs::SHELL_BUILTINS;

/// The command names the shell knows: executables on `PATH`, builtins, functions and aliases.
#[derive(Debug, Clone, Default)]
pub struct CommandCatalog {
    path_var: String,
    executables: Option<Vec<String>>,
    shell_names: Vec<String>,
    known: Option<BTreeSet<String>>,
}

impl CommandCatalog {
    /// Reads `PATH` again when it changed. Lists folders, so keep it off the UI thread.
    pub fn set_path(&mut self, path_var: &str, sources: &dyn CompletionSources) {
        if path_var == self.path_var {
            return;
        }
        self.path_var = path_var.to_owned();
        self.set_executables(sources.executables(path_var));
    }

    /// For callers that list executables elsewhere, for example on a background thread.
    pub fn set_executables(&mut self, names: Vec<String>) {
        self.executables = Some(names);
        self.rebuild();
    }

    /// Functions and aliases the shell reported.
    pub fn set_shell_names(&mut self, names: Vec<String>) {
        self.shell_names = names;
        self.rebuild();
    }

    /// Whether `name` is a command; `None` until the executables are known.
    pub fn is_known(&self, name: &str) -> Option<bool> {
        self.known.as_ref().map(|known| known.contains(name))
    }

    /// Sorted names, or just the builtins until the executables are known.
    pub fn names(&self) -> Vec<String> {
        match &self.known {
            Some(known) => known.iter().cloned().collect(),
            None => SHELL_BUILTINS.iter().map(|&name| name.to_owned()).collect(),
        }
    }

    fn rebuild(&mut self) {
        let Some(executables) = &self.executables else {
            return;
        };
        let builtins = SHELL_BUILTINS.iter().map(|&name| name.to_owned());
        self.known = Some(
            executables
                .iter()
                .cloned()
                .chain(builtins)
                .chain(self.shell_names.iter().cloned())
                .collect(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeSources;

    #[test]
    fn knows_builtins_executables_and_shell_names() {
        let mut catalog = CommandCatalog::default();
        assert_eq!(catalog.is_known("ls"), None);
        assert!(catalog.names().contains(&"cd".to_owned()));

        let sources = FakeSources {
            executables: vec!["zz-tool".into(), "ls".into()],
            ..Default::default()
        };
        catalog.set_shell_names(vec!["gst".into()]);
        catalog.set_path("/bin", &sources);
        assert_eq!(catalog.is_known("ls"), Some(true));
        assert_eq!(catalog.is_known("gst"), Some(true));
        assert_eq!(catalog.is_known("nope"), Some(false));
        let names = catalog.names();
        assert_eq!(names.first().map(String::as_str), Some("alias"));
        assert_eq!(names.last().map(String::as_str), Some("zz-tool"));
    }
}

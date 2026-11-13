use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn list_executables(path_var: &str) -> Vec<String> {
    let mut names = BTreeSet::new();
    for dir in path_var.split(':').filter(|dir| !dir.is_empty()) {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(|entry| entry.ok()) {
            if is_executable(&entry.path()) {
                names.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.into_iter().collect()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

pub fn git_branches(cwd: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
        .current_dir(cwd)
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .filter(|line| !line.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn finds_only_executables() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("tool");
        fs::write(&tool, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(dir.path().join("notes.txt"), "").unwrap();
        let path_var = format!("{}::/does/not/exist", dir.path().display());
        assert_eq!(list_executables(&path_var), vec!["tool"]);
    }

    #[test]
    fn no_branches_outside_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git_branches(dir.path()).is_empty());
    }
}

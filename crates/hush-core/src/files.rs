use std::cmp::Ordering;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Folders first, case-insensitive. Symlinks to folders count as folders.
pub fn list_dir(path: &Path) -> Result<Vec<DirEntry>> {
    let mut entries: Vec<DirEntry> = fs::read_dir(path)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            let is_symlink = file_type.is_symlink();
            let is_dir = if is_symlink {
                entry.path().is_dir()
            } else {
                file_type.is_dir()
            };
            Some(DirEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir,
                is_symlink,
            })
        })
        .collect();
    entries.sort_by(compare_entries);
    Ok(entries)
}

fn compare_entries(a: &DirEntry, b: &DirEntry) -> Ordering {
    b.is_dir
        .cmp(&a.is_dir)
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then_with(|| a.name.cmp(&b.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_folders_first_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::create_dir(dir.path().join("Docs")).unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();
        fs::write(dir.path().join("A.md"), "").unwrap();
        let names: Vec<_> = list_dir(dir.path())
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.is_dir))
            .collect();
        assert_eq!(
            names,
            vec![
                ("Docs".into(), true),
                ("src".into(), true),
                ("A.md".into(), false),
                ("b.txt".into(), false)
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_folders_count_as_folders() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("real")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("link")).unwrap();
        let link = list_dir(dir.path())
            .unwrap()
            .into_iter()
            .find(|e| e.name == "link")
            .unwrap();
        assert!(link.is_dir && link.is_symlink);
    }

    #[test]
    fn missing_directory_is_an_error() {
        assert!(list_dir(Path::new("/definitely/not/here")).is_err());
    }
}

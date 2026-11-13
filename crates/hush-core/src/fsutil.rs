use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// Atomic write (temp file + rename) with 0600 permissions.
pub fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_path(path);
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_keeps_it_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/1.cmd");
        write_private_file(&path, b"first").unwrap();
        write_private_file(&path, b"git status").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "git status");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn temp_names_keep_the_extension() {
        assert_eq!(temp_path(Path::new("/a/1.cmd")), Path::new("/a/1.cmd.tmp"));
        assert_ne!(
            temp_path(Path::new("/a/1.cmd")),
            temp_path(Path::new("/a/1.hl"))
        );
    }
}

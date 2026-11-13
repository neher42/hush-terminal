use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::error::{Error, Result};
use crate::fsutil::write_private_file;

pub fn read(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn write(path: &Path, json: &str) -> Result<()> {
    serde_json::from_str::<serde_json::Value>(json)
        .map_err(|err| Error::Message(format!("Settings must be valid JSON: {err}")))?;
    write_private_file(path, json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_reports_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(read(&path).unwrap(), None);
        write(&path, r#"{"fontSize":14}"#).unwrap();
        assert_eq!(read(&path).unwrap().as_deref(), Some(r#"{"fontSize":14}"#));
    }

    #[test]
    fn rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert!(write(&path, "{nope").is_err());
        assert_eq!(read(&path).unwrap(), None);
    }
}

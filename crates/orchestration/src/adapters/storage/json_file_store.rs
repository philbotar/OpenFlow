//! Shared atomic JSON file persistence for adapter stores.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;

pub const OPENFLOW_DATA_DIR_SLUG: &str = "openflow";

/// Write `content` atomically via a sibling `.tmp` file.
///
/// # Errors
/// Returns an error if the parent directory or file cannot be written.
pub fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)
}

fn invalid_data(context: &str, error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{context}: {error}"))
}

fn serialize_error(context: &str, error: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{context} serialization failed: {error}"),
    )
}

/// Read and parse JSON from `path` when the file exists.
///
/// # Errors
/// Returns an error if the file cannot be read or parsed.
pub fn read_json_file<T: DeserializeOwned>(
    path: &Path,
    invalid_context: &str,
) -> io::Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| invalid_data(invalid_context, error))
}

/// Serialize `value` and atomically write it to `path`.
///
/// # Errors
/// Returns an error if serialization or the write fails.
pub fn write_json_file<T: Serialize>(
    path: &Path,
    value: &T,
    serialize_context: &str,
) -> io::Result<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| serialize_error(serialize_context, error))?;
    atomic_write(path, &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Sample {
        value: u32,
    }

    #[test]
    fn atomic_write_does_not_leave_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("store.json");

        atomic_write(&path, r#"{"value":1}"#).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn read_write_json_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("store.json");
        let sample = Sample { value: 42 };

        write_json_file(&path, &sample, "sample store").unwrap();
        let loaded: Sample = read_json_file(&path, "sample store").unwrap().unwrap();

        assert_eq!(loaded, sample);
    }

    #[test]
    fn read_json_file_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.json");

        let loaded = read_json_file::<Sample>(&path, "sample store").unwrap();

        assert!(loaded.is_none());
    }
}

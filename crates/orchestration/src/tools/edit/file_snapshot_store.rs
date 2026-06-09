//! Per-run file snapshot store for hashline section tags.

use std::path::Path;
use std::sync::Arc;

use super::hashline::snapshots::{InMemorySnapshotStore, SnapshotStore};
use super::normalize::{normalize_to_lf, strip_bom};
use super::path::{resolve_writable, PathEscapeError};

/// Upper bound on file size we snapshot (whole-file content-hash model).
pub const SNAPSHOT_MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Canonical path key shared by `read` snapshot recording and hashline lookup.
pub fn canonical_snapshot_path(cwd: &Path, user_path: &str) -> Result<String, PathEscapeError> {
    resolve_writable(cwd, user_path).map(|absolute| absolute.to_string_lossy().into_owned())
}

/// Read the full text of `canonical_path`, record a version snapshot, and return its tag.
pub fn record_file_snapshot(
    store: &InMemorySnapshotStore,
    canonical_path: &str,
    raw_text: &str,
) -> Option<String> {
    if raw_text.len() as u64 > SNAPSHOT_MAX_BYTES {
        return None;
    }
    let stripped = strip_bom(raw_text);
    let normalized = normalize_to_lf(&stripped.text);
    Some(store.record(canonical_path, &normalized))
}

impl SnapshotStore for Arc<InMemorySnapshotStore> {
    fn head(&self, path: &str) -> Option<super::hashline::snapshots::Snapshot> {
        self.as_ref().head(path)
    }

    fn by_hash(&self, path: &str, hash: &str) -> Option<super::hashline::snapshots::Snapshot> {
        self.as_ref().by_hash(path, hash)
    }

    fn record(&self, path: &str, full_text: &str) -> String {
        self.as_ref().record(path, full_text)
    }

    fn invalidate(&self, path: &str) {
        self.as_ref().invalidate(path);
    }

    fn clear(&self) {
        self.as_ref().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn canonical_snapshot_path_matches_resolve_writable() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let file = "note.txt";
        fs::write(temp.path().join(file), "hello").expect("seed");
        let canonical = canonical_snapshot_path(temp.path(), file).expect("canonical");
        let resolved = resolve_writable(temp.path(), file).expect("resolve");
        assert_eq!(canonical, resolved.to_string_lossy());
    }
}

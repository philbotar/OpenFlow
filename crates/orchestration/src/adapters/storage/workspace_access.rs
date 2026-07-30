use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

/// Default root for app-managed workflow and chat execution folders.
#[must_use]
pub fn default_managed_workspace_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(crate::adapters::storage::json_file_store::OPENFLOW_DATA_DIR_SLUG)
        .join("workspaces")
}

/// Creates `directory`, then proves create/write/rename/delete access inside it.
///
/// Metadata checks alone are insufficient on ACL- and sandbox-controlled filesystems.
pub fn ensure_writable_directory(directory: &Path) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let probe_id = uuid::Uuid::new_v4();
    let source = directory.join(format!(".openflow-access-{probe_id}"));
    let renamed = directory.join(format!(".openflow-access-{probe_id}.verified"));

    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&source)?;
        file.write_all(b"openflow workspace access probe\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&source, &renamed)?;
        fs::remove_file(&renamed)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&source);
        let _ = fs::remove_file(&renamed);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_and_probes_a_writable_directory_without_leaving_files() {
        let root = tempdir().expect("tempdir");
        let directory = root.path().join("workspace");

        ensure_writable_directory(&directory).expect("writable");

        assert!(directory.is_dir());
        assert_eq!(fs::read_dir(directory).expect("read workspace").count(), 0);
    }

    #[test]
    fn rejects_a_path_occupied_by_a_file() {
        let root = tempdir().expect("tempdir");
        let blocked = root.path().join("workspace");
        fs::write(&blocked, "file").expect("blocking file");

        assert!(ensure_writable_directory(&blocked).is_err());
    }
}

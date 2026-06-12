use crate::adapters::storage::json_file_store::{
    read_json_file, write_json_file, OPENFLOW_DATA_DIR_SLUG,
};
use crate::workflow::ports::WorkflowStore;
use engine::Workflow;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileWorkflowStore {
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct StoredWorkflows {
    workflows: Vec<Workflow>,
}

impl FileWorkflowStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(OPENFLOW_DATA_DIR_SLUG)
            .join("workflows.json")
    }

    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(&self) -> io::Result<Vec<Workflow>> {
        let stored: StoredWorkflows =
            read_json_file(&self.path, "workflow store JSON invalid")?.unwrap_or_default();
        Ok(stored.workflows)
    }

    /// # Errors
    /// Returns an error if the file cannot be serialized or written.
    pub fn save(&self, workflows: &[Workflow]) -> io::Result<()> {
        let stored = StoredWorkflows {
            workflows: workflows.to_vec(),
        };
        write_json_file(&self.path, &stored, "workflow store JSON")
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl WorkflowStore for FileWorkflowStore {
    fn load(&self) -> io::Result<Vec<Workflow>> {
        FileWorkflowStore::load(self)
    }

    fn save(&self, workflows: &[Workflow]) -> io::Result<()> {
        FileWorkflowStore::save(self, workflows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn missing_store_loads_empty() {
        let dir = tempdir().unwrap();
        let store = FileWorkflowStore::new(dir.path().join("workflows.json"));

        let workflows = store.load().unwrap();

        assert!(workflows.is_empty());
    }

    #[test]
    fn saves_and_loads_workflows() {
        let dir = tempdir().unwrap();
        let store = FileWorkflowStore::new(dir.path().join("nested").join("workflows.json"));
        let workflow = Workflow::new("Saved");

        store.save(std::slice::from_ref(&workflow)).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded, vec![workflow]);
    }

    #[test]
    fn invalid_store_json_returns_invalid_data_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("workflows.json");
        fs::write(&path, "{\"workflows\":").unwrap();
        let store = FileWorkflowStore::new(path);

        let error = store.load().unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("workflow store JSON invalid"));
    }

    #[test]
    fn saved_file_uses_workflows_wrapper_key() {
        let dir = tempdir().unwrap();
        let store = FileWorkflowStore::new(dir.path().join("workflows.json"));
        let workflow = Workflow::new("Wrapped");

        store.save(std::slice::from_ref(&workflow)).unwrap();
        let raw = fs::read_to_string(store.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();

        assert!(parsed.get("workflows").is_some());
        assert_eq!(parsed["workflows"][0]["name"], "Wrapped");
    }

    #[test]
    fn atomic_save_does_not_leave_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("workflows.json");
        let store = FileWorkflowStore::new(&path);
        let workflow = Workflow::new("Atomic");

        store.save(std::slice::from_ref(&workflow)).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists());
    }
}

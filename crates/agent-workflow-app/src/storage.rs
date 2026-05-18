use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use workflow_core::Workflow;

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

    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("step-through-agentic-workflow")
            .join("workflows.json")
    }

    pub fn load(&self) -> io::Result<Vec<Workflow>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let text = fs::read_to_string(&self.path)?;
        let stored: StoredWorkflows = serde_json::from_str(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("workflow store JSON invalid: {error}"),
            )
        })?;
        Ok(stored.workflows)
    }

    pub fn save(&self, workflows: &[Workflow]) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let stored = StoredWorkflows {
            workflows: workflows.to_vec(),
        };
        let text = serde_json::to_string_pretty(&stored).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("workflow store JSON serialization failed: {error}"),
            )
        })?;
        fs::write(&self.path, text)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        store.save(&[workflow.clone()]).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded, vec![workflow]);
    }
}

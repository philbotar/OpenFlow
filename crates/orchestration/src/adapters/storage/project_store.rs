use crate::project::ports::{Project, ProjectStore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CURRENT_DATA_DIR_SLUG: &str = "openflow";
const LEGACY_DATA_DIR_SLUG: &str = "step-through-agentic-workflow";
const PROJECTS_FILE_NAME: &str = "projects.json";

fn legacy_store_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let dir_name = parent.file_name()?;

    if dir_name != CURRENT_DATA_DIR_SLUG || path.file_name()? != PROJECTS_FILE_NAME {
        return None;
    }

    Some(
        parent
            .with_file_name(LEGACY_DATA_DIR_SLUG)
            .join(PROJECTS_FILE_NAME),
    )
}

#[derive(Debug, Clone)]
pub struct FileProjectStore {
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct StoredProjects {
    projects: Vec<Project>,
}

impl FileProjectStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CURRENT_DATA_DIR_SLUG)
            .join(PROJECTS_FILE_NAME)
    }

    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(&self) -> io::Result<Vec<Project>> {
        if !self.path.exists() {
            if let Some(legacy_path) = legacy_store_path(&self.path) {
                if legacy_path.exists() {
                    return Self::new(legacy_path).load();
                }
            }
            return Ok(Vec::new());
        }

        let text = fs::read_to_string(&self.path)?;
        let stored: StoredProjects = serde_json::from_str(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("project store JSON invalid: {error}"),
            )
        })?;
        Ok(stored.projects)
    }

    /// # Errors
    /// Returns an error if the file cannot be serialized or written.
    pub fn save(&self, projects: &[Project]) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let stored = StoredProjects {
            projects: projects.to_vec(),
        };
        let text = serde_json::to_string_pretty(&stored).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("project store JSON serialization failed: {error}"),
            )
        })?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ProjectStore for FileProjectStore {
    fn load(&self) -> io::Result<Vec<Project>> {
        FileProjectStore::load(self)
    }

    fn save(&self, projects: &[Project]) -> io::Result<()> {
        FileProjectStore::save(self, projects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_store_loads_empty() {
        let dir = tempdir().unwrap();
        let store = FileProjectStore::new(dir.path().join("projects.json"));

        let projects = store.load().unwrap();

        assert!(projects.is_empty());
    }

    #[test]
    fn saves_and_loads_projects() {
        let dir = tempdir().unwrap();
        let store = FileProjectStore::new(dir.path().join("nested").join("projects.json"));
        let project = Project::new("/tmp/repo", "Repo");

        store.save(std::slice::from_ref(&project)).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded, vec![project]);
    }
}

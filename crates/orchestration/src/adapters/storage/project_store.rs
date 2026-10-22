use crate::adapters::storage::json_file_store::{app_data_dir, read_json_file, write_json_file};
use crate::project::ports::{Project, ProjectStore};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

const PROJECTS_FILE_NAME: &str = "projects.json";

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
        app_data_dir().join(PROJECTS_FILE_NAME)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ProjectStore for FileProjectStore {
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    fn load(&self) -> io::Result<Vec<Project>> {
        let stored: StoredProjects =
            read_json_file(&self.path, "project store JSON invalid")?.unwrap_or_default();
        Ok(stored.projects)
    }

    /// # Errors
    /// Returns an error if the file cannot be serialized or written.
    fn save(&self, projects: &[Project]) -> io::Result<()> {
        let stored = StoredProjects {
            projects: projects.to_vec(),
        };
        write_json_file(&self.path, &stored, "project store JSON")
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

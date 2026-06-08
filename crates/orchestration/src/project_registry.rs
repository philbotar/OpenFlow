use crate::error::BackendError;
use crate::project_store::{
    cleanup_stored_projects, create_project_from_path, validate_unique_project_path,
    FileProjectStore, Project,
};

#[derive(Debug)]
pub struct ProjectRegistry {
    store: FileProjectStore,
}

impl ProjectRegistry {
    #[must_use]
    pub fn new(store: FileProjectStore) -> Self {
        Self { store }
    }

    /// # Errors
    /// Returns an error if the project store cannot be read or written during cleanup.
    pub fn load(&self) -> Result<Vec<Project>, BackendError> {
        let mut projects = self.store.load()?;
        if cleanup_stored_projects(&mut projects) {
            self.store.save(&projects)?;
        }
        Ok(projects)
    }

    /// # Errors
    /// Returns an error if the project store cannot be read.
    pub fn list(&self) -> Result<Vec<Project>, BackendError> {
        self.load()
    }

    /// # Errors
    /// Returns an error if the project store cannot be written.
    pub fn save(&self, projects: &[Project]) -> Result<(), BackendError> {
        self.store.save(projects).map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the path is invalid, already registered, or the store cannot be written.
    pub fn create_from_directory(&self, path: String) -> Result<Project, BackendError> {
        let mut projects = self.load()?;
        validate_unique_project_path(&projects, &path).map_err(BackendError::ProjectOperation)?;
        let project = create_project_from_path(&path).map_err(BackendError::ProjectOperation)?;
        projects.push(project.clone());
        self.save(&projects)?;
        Ok(project)
    }

    pub(crate) fn store(&self) -> &FileProjectStore {
        &self.store
    }
}

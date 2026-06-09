use crate::error::BackendError;
use crate::project::domain::{
    assign_workflow, cleanup_stored_projects, create_project_from_path,
    unassign_workflow_from_project, validate_unique_project_path,
};
use crate::project::ports::{Project, ProjectStore};

pub struct ProjectRegistry {
    store: Box<dyn ProjectStore>,
}

impl ProjectRegistry {
    #[must_use]
    pub fn new(store: Box<dyn ProjectStore>) -> Self {
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

    /// # Errors
    /// Returns an error if the project is missing or the store cannot be written.
    pub fn link_workflow(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<String, BackendError> {
        let mut project_list = self.load()?;
        let project_path = project_list
            .iter()
            .find(|project| project.id == project_id)
            .map(|project| project.path.clone())
            .ok_or_else(|| BackendError::ProjectNotFound(project_id.to_string()))?;
        assign_workflow(&mut project_list, project_id, workflow_id)
            .map_err(BackendError::ProjectOperation)?;
        self.save(&project_list)?;
        Ok(project_path)
    }

    /// # Errors
    /// Returns an error if the project is missing or the store cannot be written.
    pub fn unlink_workflow(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<String, BackendError> {
        let mut project_list = self.load()?;
        let project_path = project_list
            .iter()
            .find(|project| project.id == project_id)
            .map(|project| project.path.clone())
            .ok_or_else(|| BackendError::ProjectNotFound(project_id.to_string()))?;
        unassign_workflow_from_project(&mut project_list, project_id, workflow_id)
            .map_err(BackendError::ProjectOperation)?;
        self.save(&project_list)?;
        Ok(project_path)
    }
}

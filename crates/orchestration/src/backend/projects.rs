use crate::project::ports::Project;

use super::{AppBackend, BackendError, ProjectFileReference};

impl AppBackend {
    pub fn list_projects(&self) -> Result<Vec<Project>, BackendError> {
        self.projects.list()
    }

    pub fn list_project_file_references(
        &self,
        execution_cwd: String,
        query: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<ProjectFileReference>, BackendError> {
        crate::project::file_refs::list_project_file_references(
            &execution_cwd,
            query.as_deref(),
            limit,
        )
    }

    pub fn save_projects(&self, projects: &[Project]) -> Result<(), BackendError> {
        self.projects.save(projects)
    }

    pub fn create_project_from_directory(&self, path: String) -> Result<Project, BackendError> {
        self.projects.create_from_directory(path)
    }
}

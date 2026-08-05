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
        let current_projects = self.projects.load()?;
        let project_ids = projects
            .iter()
            .map(|project| project.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let removed_project_ids = current_projects
            .iter()
            .filter(|project| !project_ids.contains(project.id.as_str()))
            .map(|project| project.id.clone())
            .collect::<Vec<_>>();
        self.chats.detach_from_projects(&removed_project_ids)?;
        self.projects.save(projects)
    }

    pub fn create_project_from_directory(&self, path: String) -> Result<Project, BackendError> {
        let candidate = crate::project::domain::create_project_from_path(&path)
            .map_err(BackendError::ProjectOperation)?;
        self.workflows
            .ensure_project_storage_writable(std::path::Path::new(&candidate.path))?;
        self.projects.create_from_directory(path)
    }
}

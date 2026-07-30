use crate::adapters::storage::workspace_access::ensure_writable_directory;
use crate::project::ports::Project;
use crate::run::persistence::RunStoreRoot;
use std::path::{Component, Path};

use super::{AppBackend, BackendError};

#[derive(Debug, Clone)]
pub(super) struct RunWorkspace {
    pub execution_cwd: String,
    pub run_root: RunStoreRoot,
}

#[derive(Debug, Clone, Copy)]
enum ManagedWorkspaceKind {
    Workflow,
    Chat,
}

impl ManagedWorkspaceKind {
    const fn directory(self) -> &'static str {
        match self {
            Self::Workflow => "workflows",
            Self::Chat => "chats",
        }
    }
}

impl AppBackend {
    pub(super) fn workspace_for_workflow(
        &self,
        workflow_id: &str,
        project_id: Option<&str>,
    ) -> Result<RunWorkspace, BackendError> {
        let projects = self.projects.load()?;
        let project = select_workflow_project(&projects, workflow_id, project_id)?;
        match project {
            Some(project) => self.workspace_for_project(project),
            None => self.managed_workspace(ManagedWorkspaceKind::Workflow, workflow_id),
        }
    }

    pub(super) fn workspace_for_chat(
        &self,
        chat_id: &str,
        project_id: Option<&str>,
    ) -> Result<RunWorkspace, BackendError> {
        match project_id {
            Some(project_id) => {
                let projects = self.projects.load()?;
                let project = projects
                    .iter()
                    .find(|project| project.id == project_id)
                    .ok_or_else(|| BackendError::ProjectNotFound(project_id.to_string()))?;
                self.workspace_for_project(project)
            }
            None => self.managed_workspace(ManagedWorkspaceKind::Chat, chat_id),
        }
    }

    fn workspace_for_project(&self, project: &Project) -> Result<RunWorkspace, BackendError> {
        self.workflows
            .ensure_project_storage_writable(Path::new(&project.path))?;
        let configured = project.default_execution_cwd.trim();
        let execution_cwd = if configured.is_empty() {
            project.path.clone()
        } else {
            configured.to_string()
        };
        Ok(RunWorkspace {
            execution_cwd,
            run_root: RunStoreRoot {
                project_id: Some(project.id.clone()),
                root: Path::new(&project.path).join(".flow").join("runs"),
            },
        })
    }

    fn managed_workspace(
        &self,
        kind: ManagedWorkspaceKind,
        subject_id: &str,
    ) -> Result<RunWorkspace, BackendError> {
        validate_managed_subject_id(subject_id)?;
        let execution_cwd = self
            .managed_workspace_root
            .join(kind.directory())
            .join(subject_id);
        ensure_writable_directory(&execution_cwd).map_err(|error| {
            BackendError::InvalidExecutionCwd(format!(
                "cannot prepare managed OpenFlow workspace at {}: {error}",
                execution_cwd.display()
            ))
        })?;
        Ok(RunWorkspace {
            execution_cwd: execution_cwd.display().to_string(),
            run_root: RunStoreRoot {
                project_id: None,
                root: self.app_runs_root.clone(),
            },
        })
    }
}

fn select_workflow_project<'a>(
    projects: &'a [Project],
    workflow_id: &str,
    project_id: Option<&str>,
) -> Result<Option<&'a Project>, BackendError> {
    if let Some(project_id) = project_id {
        let project = projects
            .iter()
            .find(|project| project.id == project_id)
            .ok_or_else(|| BackendError::ProjectNotFound(project_id.to_string()))?;
        if !project
            .workflow_ids
            .iter()
            .any(|candidate| candidate == workflow_id)
        {
            return Err(BackendError::ProjectOperation(format!(
                "workflow {workflow_id} is not linked to project {}",
                project.name
            )));
        }
        return Ok(Some(project));
    }

    Ok(projects
        .iter()
        .find(|project| project.workflow_ids.iter().any(|id| id == workflow_id)))
}

fn validate_managed_subject_id(subject_id: &str) -> Result<(), BackendError> {
    let mut components = Path::new(subject_id).components();
    if subject_id.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(BackendError::InvalidExecutionCwd(
            "workflow/chat id cannot identify a managed workspace".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_managed_subject_id;

    #[test]
    fn managed_workspace_subject_must_be_one_path_component() {
        assert!(validate_managed_subject_id("workflow-1").is_ok());
        assert!(validate_managed_subject_id("../escape").is_err());
        assert!(validate_managed_subject_id("nested/workflow").is_err());
        assert!(validate_managed_subject_id("").is_err());
    }
}

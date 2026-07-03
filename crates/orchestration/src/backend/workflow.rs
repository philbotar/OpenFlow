use engine::{Node, Workflow};

use super::{AppBackend, BackendError, WorkflowListItem, WorkflowValidationSummary};

impl AppBackend {
    pub fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError> {
        self.workflows.list(&self.projects)
    }

    pub fn load_all_workflows(&self) -> Result<Vec<Workflow>, BackendError> {
        let workflows = self.workflows.load_all(&self.projects)?;
        let _ = self.schedule.refresh(&workflows, chrono::Utc::now());
        Ok(workflows)
    }

    pub fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError> {
        self.workflows.load_one(&self.projects, workflow_id)
    }

    pub fn create_workflow(&self, name: String) -> Result<Workflow, BackendError> {
        self.workflows.create(name)
    }

    pub fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError> {
        let saved = self
            .workflows
            .save_one(&self.projects, workflow)
            .map_err(|error| self.persistence_err("persistence.workflow_save", error))?;
        self.refresh_schedules()?;
        Ok(saved)
    }

    pub fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), BackendError> {
        self.workflows
            .save_all(&self.projects, workflows)
            .map_err(|error| self.persistence_err("persistence.workflow_save", error))?;
        self.refresh_schedules()?;
        Ok(())
    }

    pub fn rename_workflow(
        &self,
        workflow_id: &str,
        name: String,
    ) -> Result<WorkflowListItem, BackendError> {
        self.workflows.rename(&self.projects, workflow_id, name)
    }

    pub fn create_agent_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<&str>,
    ) -> Result<Node, BackendError> {
        self.agents.create_node(index, x, y, agent_id)
    }

    pub fn validate_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError> {
        self.settings.validate_workflow(workflow)
    }

    pub fn assign_workflow_to_project(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<crate::project::ports::Project>, BackendError> {
        self.workflows
            .assign_to_project(&self.projects, project_id, workflow_id)
    }

    pub fn copy_workflow_to_project(
        &self,
        target_project_id: &str,
        source_workflow_id: &str,
    ) -> Result<crate::api::CopyWorkflowToProjectResult, BackendError> {
        let workflow = self.workflows.copy_to_project(
            &self.projects,
            target_project_id,
            source_workflow_id,
        )?;
        let projects = self.projects.load()?;
        Ok(crate::api::CopyWorkflowToProjectResult { workflow, projects })
    }

    pub fn unassign_workflow_from_project(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<crate::project::ports::Project>, BackendError> {
        self.workflows
            .unassign_from_project(&self.projects, project_id, workflow_id)
    }

    pub fn delete_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<crate::project::ports::Project>, BackendError> {
        let projects = self
            .workflows
            .delete(&self.projects, workflow_id)
            .map_err(|error| self.persistence_err("persistence.workflow_delete", error))?;
        self.refresh_schedules()?;
        Ok(projects)
    }
}

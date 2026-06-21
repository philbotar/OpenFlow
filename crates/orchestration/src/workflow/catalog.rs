use crate::api::WorkflowListItem;
use crate::error::BackendError;
use crate::project::ports::Project;
use crate::project::registry::ProjectRegistry;
use crate::workflow::ports::{ProjectWorkflowStore, WorkflowStore};
use engine::{Node, Workflow, WorkflowId};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub struct WorkflowCatalog {
    store: Box<dyn WorkflowStore>,
    project_workflows: Box<dyn ProjectWorkflowStore>,
}

impl WorkflowCatalog {
    #[must_use]
    pub fn new(
        store: Box<dyn WorkflowStore>,
        project_workflows: Box<dyn ProjectWorkflowStore>,
    ) -> Self {
        Self {
            store,
            project_workflows,
        }
    }

    /// # Errors
    /// Returns an error if workflow stores cannot be read.
    pub fn load_all(&self, projects: &ProjectRegistry) -> Result<Vec<Workflow>, BackendError> {
        let mut by_id = BTreeMap::<String, Workflow>::new();
        for workflow in self.store.load()? {
            by_id.insert(workflow.id.to_string(), workflow);
        }
        for project in projects.load()? {
            for workflow in self.project_workflows.discover(Path::new(&project.path))? {
                by_id.insert(workflow.id.to_string(), workflow);
            }
        }
        Ok(by_id.into_values().collect())
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be read.
    pub fn list(&self, projects: &ProjectRegistry) -> Result<Vec<WorkflowListItem>, BackendError> {
        Ok(self
            .load_all(projects)?
            .into_iter()
            .map(|workflow| WorkflowListItem {
                id: workflow.id.to_string(),
                name: workflow.name,
            })
            .collect())
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be read or the workflow does not exist.
    pub fn load_one(
        &self,
        projects: &ProjectRegistry,
        workflow_id: &str,
    ) -> Result<Workflow, BackendError> {
        self.load_all(projects)?
            .into_iter()
            .find(|workflow| workflow.id == workflow_id)
            .ok_or_else(|| BackendError::WorkflowNotFound(workflow_id.to_string()))
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be written.
    pub fn create(&self, name: String) -> Result<Workflow, BackendError> {
        let mut workflows = self.store.load()?;
        let workflow = default_workflow(name.as_str());
        workflows.push(workflow.clone());
        self.store.save(&workflows)?;
        Ok(workflow)
    }

    /// # Errors
    /// Returns an error if workflow stores cannot be written.
    pub fn save_one(
        &self,
        projects: &ProjectRegistry,
        workflow: Workflow,
    ) -> Result<Workflow, BackendError> {
        let mut workflows = self.load_all(projects)?;
        if let Some(existing) = workflows.iter_mut().find(|item| item.id == workflow.id) {
            *existing = workflow.clone();
        } else {
            workflows.push(workflow.clone());
        }
        self.save_all(projects, &workflows)?;
        Ok(workflow)
    }

    /// # Errors
    /// Returns an error if workflow stores cannot be written.
    pub fn save_all(
        &self,
        projects: &ProjectRegistry,
        workflows: &[Workflow],
    ) -> Result<(), BackendError> {
        let project_list = projects.load()?;
        let assigned_ids: HashSet<String> = project_list
            .iter()
            .flat_map(|project| project.workflow_ids.iter().cloned())
            .collect();

        let app_workflows: Vec<Workflow> = workflows
            .iter()
            .filter(|workflow| !assigned_ids.contains(&*workflow.id))
            .cloned()
            .collect();
        self.store.save(&app_workflows)?;

        for project in &project_list {
            let project_workflows: Vec<Workflow> = workflows
                .iter()
                .filter(|workflow| project.workflow_ids.iter().any(|id| id == &*workflow.id))
                .cloned()
                .collect();
            self.project_workflows
                .save_all(Path::new(&project.path), &project_workflows)?;
        }

        Ok(())
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be written or the workflow does not exist.
    pub fn rename(
        &self,
        projects: &ProjectRegistry,
        workflow_id: &str,
        name: String,
    ) -> Result<WorkflowListItem, BackendError> {
        let mut workflows = self.load_all(projects)?;
        let workflow = workflows
            .iter_mut()
            .find(|item| item.id == workflow_id)
            .ok_or_else(|| BackendError::WorkflowNotFound(workflow_id.to_string()))?;
        workflow.name = name.clone();
        self.save_all(projects, &workflows)?;
        Ok(WorkflowListItem {
            id: workflow_id.to_string(),
            name,
        })
    }

    /// # Errors
    /// Returns an error if the source workflow, target project, or stores are missing.
    pub fn copy_to_project(
        &self,
        projects: &ProjectRegistry,
        target_project_id: &str,
        source_workflow_id: &str,
    ) -> Result<Workflow, BackendError> {
        let source = self.load_one(projects, source_workflow_id)?;
        let mut copy = source;
        copy.id = WorkflowId(uuid::Uuid::new_v4().to_string());
        copy.name = format!("{} copy", copy.name);
        let project_path = projects.link_workflow(target_project_id, &copy.id.to_string())?;
        self.project_workflows
            .save_one(Path::new(&project_path), &copy)?;
        Ok(copy)
    }

    /// # Errors
    /// Returns an error if the project is missing or stores cannot be written.
    pub fn assign_to_project(
        &self,
        projects: &ProjectRegistry,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<Project>, BackendError> {
        let workflow = self.load_one(projects, workflow_id)?;
        let project_path = projects.link_workflow(project_id, workflow_id)?;
        self.project_workflows
            .save_one(Path::new(&project_path), &workflow)?;
        projects.load()
    }

    /// # Errors
    /// Returns an error if the store cannot be read or written.
    pub fn unassign_from_project(
        &self,
        projects: &ProjectRegistry,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<Project>, BackendError> {
        let project_path = projects.unlink_workflow(project_id, workflow_id)?;
        self.project_workflows
            .delete(Path::new(&project_path), workflow_id)?;
        projects.load()
    }

    /// Permanently removes a workflow from app and project stores.
    ///
    /// # Errors
    /// Returns an error if the workflow is missing or stores cannot be written.
    pub fn delete(
        &self,
        projects: &ProjectRegistry,
        workflow_id: &str,
    ) -> Result<Vec<Project>, BackendError> {
        self.load_one(projects, workflow_id)?;

        let project_ids: Vec<String> = projects
            .load()?
            .into_iter()
            .filter(|project| project.workflow_ids.iter().any(|id| id == workflow_id))
            .map(|project| project.id)
            .collect();

        for project_id in project_ids {
            self.unassign_from_project(projects, &project_id, workflow_id)?;
        }

        let mut app_workflows = self.store.load()?;
        let before = app_workflows.len();
        app_workflows.retain(|workflow| workflow.id != workflow_id);
        if app_workflows.len() != before {
            self.store.save(&app_workflows)?;
        }

        projects.load()
    }
}

pub(crate) fn default_workflow(name: &str) -> Workflow {
    let mut workflow = Workflow::new(name);
    workflow.nodes.push(Node::agent("Idea", 80.0, 120.0));
    workflow
}

use crate::api::WorkflowListItem;
use crate::error::BackendError;
use crate::project::ports::Project;
use crate::project::registry::ProjectRegistry;
use crate::workflow::ports::{ProjectWorkflowStore, WorkflowStore, WorkflowStoreState};
use engine::{Node, Workflow, WorkflowId};
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashSet};
use std::io;
use std::path::Path;

const MATT_POCOCK_IDEA_TO_SHIP_WORKFLOW_ID: &str = "matt-pocock-idea-to-ship";
const MATT_POCOCK_IDEA_TO_SHIP_SEED_KEY: &str = "matt-pocock-idea-to-ship";
const MATT_POCOCK_IDEA_TO_SHIP_REPAIR_KEY: &str =
    "matt-pocock-idea-to-ship:repair-incomplete-graph";
const MATT_POCOCK_IDEA_TO_SHIP_WORKFLOW_JSON: &str =
    include_str!("../../../../examples/matt_pocock_idea_to_ship.workflow.json");

pub struct WorkflowCatalog {
    store: Box<dyn WorkflowStore>,
    project_workflows: Box<dyn ProjectWorkflowStore>,
    mutation_lock: Mutex<()>,
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
            mutation_lock: Mutex::new(()),
        }
    }

    /// # Errors
    /// Returns an error if the bundled example cannot be parsed, the app store cannot
    /// be read or initialized, or a project workflow store cannot be read.
    pub fn load_all(&self, projects: &ProjectRegistry) -> Result<Vec<Workflow>, BackendError> {
        let _guard = self.mutation_lock.lock();
        self.load_all_unlocked(projects)
    }

    fn load_all_unlocked(&self, projects: &ProjectRegistry) -> Result<Vec<Workflow>, BackendError> {
        let app_state = self.ensure_bundled_examples_seeded()?;

        let mut by_id = BTreeMap::<String, Workflow>::new();
        for workflow in app_state.workflows {
            by_id.insert(workflow.id.to_string(), workflow);
        }
        for project in projects.load()? {
            for workflow in self.project_workflows.discover(Path::new(&project.path))? {
                by_id.insert(workflow.id.to_string(), workflow);
            }
        }
        Ok(by_id.into_values().collect())
    }

    fn ensure_bundled_examples_seeded(&self) -> Result<WorkflowStoreState, BackendError> {
        let bundled_example = matt_pocock_idea_to_ship_workflow()?;
        debug_assert_eq!(
            bundled_example.id, MATT_POCOCK_IDEA_TO_SHIP_WORKFLOW_ID,
            "bundled example ID must match its catalog ID"
        );
        let mut app_state = self.store.load_state()?;
        let seed_was_already_applied = app_state
            .applied_seeds
            .contains(MATT_POCOCK_IDEA_TO_SHIP_SEED_KEY);
        let seed_pending = app_state
            .applied_seeds
            .insert(MATT_POCOCK_IDEA_TO_SHIP_SEED_KEY.to_string());
        let repair_pending = app_state
            .applied_seeds
            .insert(MATT_POCOCK_IDEA_TO_SHIP_REPAIR_KEY.to_string());

        if seed_pending
            && !app_state
                .workflows
                .iter()
                .any(|workflow| workflow.id == bundled_example.id)
        {
            app_state.workflows.push(bundled_example.clone());
        }
        if repair_pending && seed_was_already_applied {
            if let Some(incomplete) = app_state
                .workflows
                .iter_mut()
                .find(|workflow| is_incomplete_matt_pocock_seed(workflow))
            {
                *incomplete = bundled_example;
            }
        }
        if seed_pending || repair_pending {
            self.store.save_state(&app_state)?;
        }
        Ok(app_state)
    }

    /// # Errors
    /// Returns an error if workflow stores cannot be initialized or read.
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
    /// Returns an error if workflow stores cannot be initialized or read, or the workflow does
    /// not exist.
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
        let _guard = self.mutation_lock.lock();
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
        let _guard = self.mutation_lock.lock();
        let mut workflows = self.load_all_unlocked(projects)?;
        if let Some(existing) = workflows.iter_mut().find(|item| item.id == workflow.id) {
            *existing = workflow.clone();
        } else {
            workflows.push(workflow.clone());
        }
        self.save_all_unlocked(projects, &workflows)?;
        Ok(workflow)
    }

    /// # Errors
    /// Returns an error if workflow stores cannot be written.
    pub fn save_all(
        &self,
        projects: &ProjectRegistry,
        workflows: &[Workflow],
    ) -> Result<(), BackendError> {
        let _guard = self.mutation_lock.lock();
        self.save_all_unlocked(projects, workflows)
    }

    fn save_all_unlocked(
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
        let _guard = self.mutation_lock.lock();
        let mut workflows = self.load_all_unlocked(projects)?;
        let workflow = workflows
            .iter_mut()
            .find(|item| item.id == workflow_id)
            .ok_or_else(|| BackendError::WorkflowNotFound(workflow_id.to_string()))?;
        workflow.name = name.clone();
        self.save_all_unlocked(projects, &workflows)?;
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
        let _guard = self.mutation_lock.lock();
        let source = self.load_one_unlocked(projects, source_workflow_id)?;
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
        let _guard = self.mutation_lock.lock();
        let workflow = self.load_one_unlocked(projects, workflow_id)?;
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
        let _guard = self.mutation_lock.lock();
        self.unassign_from_project_unlocked(projects, project_id, workflow_id)
    }

    fn unassign_from_project_unlocked(
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
        let _guard = self.mutation_lock.lock();
        self.load_one_unlocked(projects, workflow_id)?;

        let project_ids: Vec<String> = projects
            .load()?
            .into_iter()
            .filter(|project| project.workflow_ids.iter().any(|id| id == workflow_id))
            .map(|project| project.id)
            .collect();

        for project_id in project_ids {
            self.unassign_from_project_unlocked(projects, &project_id, workflow_id)?;
        }

        let mut app_workflows = self.store.load()?;
        let before = app_workflows.len();
        app_workflows.retain(|workflow| workflow.id != workflow_id);
        if app_workflows.len() != before {
            self.store.save(&app_workflows)?;
        }

        projects.load()
    }

    fn load_one_unlocked(
        &self,
        projects: &ProjectRegistry,
        workflow_id: &str,
    ) -> Result<Workflow, BackendError> {
        self.load_all_unlocked(projects)?
            .into_iter()
            .find(|workflow| workflow.id == workflow_id)
            .ok_or_else(|| BackendError::WorkflowNotFound(workflow_id.to_string()))
    }
}

pub(crate) fn default_workflow(name: &str) -> Workflow {
    let mut workflow = Workflow::new(name);
    workflow.nodes.push(Node::agent("Idea", 80.0, 120.0));
    workflow
}

fn matt_pocock_idea_to_ship_workflow() -> Result<Workflow, BackendError> {
    serde_json::from_str(MATT_POCOCK_IDEA_TO_SHIP_WORKFLOW_JSON).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bundled Matt Pocock workflow JSON invalid: {error}"),
        )
        .into()
    })
}

fn is_incomplete_matt_pocock_seed(workflow: &Workflow) -> bool {
    workflow.id == MATT_POCOCK_IDEA_TO_SHIP_WORKFLOW_ID
        && workflow.nodes.len() == 2
        && workflow.edges.is_empty()
        && workflow.nodes.iter().any(|node| node.id == "select-ticket")
        && workflow.nodes.iter().any(|node| node.id == "commit-gate")
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{validate_workflow, ApprovalMode};

    #[test]
    fn matt_pocock_idea_to_ship_example_is_valid_and_human_steered() {
        let workflow = matt_pocock_idea_to_ship_workflow().expect("bundled example");

        validate_workflow(&workflow).expect("valid idea-to-ship workflow");
        assert_eq!(workflow.id, MATT_POCOCK_IDEA_TO_SHIP_WORKFLOW_ID);
        assert_eq!(workflow.nodes.len(), 4);
        assert_eq!(workflow.edges.len(), 3);
        assert!(workflow.settings.plan_mode.is_none());
        assert!(workflow
            .nodes
            .iter()
            .all(|node| node.agent.model.is_empty()));

        let planning = workflow
            .nodes
            .iter()
            .find(|node| node.id == "shape-work")
            .expect("planning node");
        assert!(!planning.agent.auto_start);
        assert!(planning.agent.request_user_input);

        let selection = workflow
            .nodes
            .iter()
            .find(|node| node.id == "select-ticket")
            .expect("ticket selection node");
        assert_eq!(
            selection.agent.tools.effective_approval_mode(),
            ApprovalMode::ReadOnly
        );
        assert!(selection.agent.request_user_input);

        let implementation = workflow
            .nodes
            .iter()
            .find(|node| node.id == "implement-ticket")
            .expect("implementation node");
        assert!(implementation
            .agent
            .system_prompt
            .contains("two independent subagents"));
        assert!(implementation.agent.request_user_input);

        let commit_gate = workflow
            .nodes
            .iter()
            .find(|node| node.id == "commit-gate")
            .expect("commit gate");
        assert!(commit_gate.agent.request_user_input);
        assert!(commit_gate.agent.system_prompt.contains("explicit yes"));
    }
}

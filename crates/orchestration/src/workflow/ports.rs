use engine::Workflow;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkflowStoreState {
    pub workflows: Vec<Workflow>,
    pub applied_seeds: BTreeSet<String>,
}

pub trait WorkflowStore: Send + Sync {
    fn load_state(&self) -> io::Result<WorkflowStoreState>;
    fn save_state(&self, state: &WorkflowStoreState) -> io::Result<()>;

    fn load(&self) -> io::Result<Vec<Workflow>> {
        Ok(self.load_state()?.workflows)
    }

    fn save(&self, workflows: &[Workflow]) -> io::Result<()> {
        let mut state = self.load_state()?;
        state.workflows = workflows.to_vec();
        self.save_state(&state)
    }
}

pub trait ProjectWorkflowStore: Send + Sync {
    fn discover(&self, project_root: &Path) -> io::Result<Vec<Workflow>>;
    fn save_one(&self, project_root: &Path, workflow: &Workflow) -> io::Result<()>;
    fn save_all(&self, project_root: &Path, workflows: &[Workflow]) -> io::Result<()>;
    fn delete(&self, project_root: &Path, workflow_id: &str) -> io::Result<()>;
}

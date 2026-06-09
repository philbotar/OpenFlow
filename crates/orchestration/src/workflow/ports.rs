use engine::Workflow;
use std::io;
use std::path::Path;

pub trait WorkflowStore: Send + Sync {
    fn load(&self) -> io::Result<Vec<Workflow>>;
    fn save(&self, workflows: &[Workflow]) -> io::Result<()>;
}

pub trait ProjectWorkflowStore: Send + Sync {
    fn discover(&self, project_root: &Path) -> io::Result<Vec<Workflow>>;
    fn save_one(&self, project_root: &Path, workflow: &Workflow) -> io::Result<()>;
    fn save_all(&self, project_root: &Path, workflows: &[Workflow]) -> io::Result<()>;
    fn delete(&self, project_root: &Path, workflow_id: &str) -> io::Result<()>;
}

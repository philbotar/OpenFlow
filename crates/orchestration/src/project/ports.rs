use serde::{Deserialize, Serialize};
use std::io;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProjectMetadata {
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub metadata: ProjectMetadata,
    #[serde(default)]
    pub workflow_ids: Vec<String>,
    #[serde(default)]
    pub default_execution_cwd: String,
}

impl Project {
    #[must_use]
    pub fn new(path: impl Into<String>, name: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            path: path.clone(),
            name: name.into(),
            metadata: ProjectMetadata::default(),
            workflow_ids: Vec::new(),
            default_execution_cwd: path,
        }
    }
}

pub trait ProjectStore: Send + Sync {
    fn load(&self) -> io::Result<Vec<Project>>;
    fn save(&self, projects: &[Project]) -> io::Result<()>;
}

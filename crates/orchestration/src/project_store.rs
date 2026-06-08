use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CURRENT_DATA_DIR_SLUG: &str = "openflow";
const LEGACY_DATA_DIR_SLUG: &str = "step-through-agentic-workflow";
const PROJECTS_FILE_NAME: &str = "projects.json";

fn legacy_store_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let dir_name = parent.file_name()?;

    if dir_name != CURRENT_DATA_DIR_SLUG || path.file_name()? != PROJECTS_FILE_NAME {
        return None;
    }

    Some(
        parent
            .with_file_name(LEGACY_DATA_DIR_SLUG)
            .join(PROJECTS_FILE_NAME),
    )
}

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

/// Returns the first project that contains `workflow_id`, if any.
#[must_use]
pub fn find_project_for_workflow<'a>(
    projects: &'a [Project],
    workflow_id: &str,
) -> Option<&'a Project> {
    projects
        .iter()
        .find(|project| project.workflow_ids.iter().any(|id| id == workflow_id))
}

/// Drops legacy global pseudo-project rows and deduplicates membership ids.
pub fn cleanup_stored_projects(projects: &mut Vec<Project>) {
    projects.retain(|project| project.id != "global" && !project.path.is_empty());
    for project in projects.iter_mut() {
        project.workflow_ids.sort();
        project.workflow_ids.dedup();
    }
}

/// Adds `workflow_id` to `project_id` without removing it from other projects.
///
/// # Errors
/// Returns an error when `project_id` does not match any project.
pub fn assign_workflow(
    projects: &mut [Project],
    project_id: &str,
    workflow_id: &str,
) -> Result<(), String> {
    let target = projects
        .iter_mut()
        .find(|project| project.id == project_id)
        .ok_or_else(|| format!("project {project_id} not found"))?;

    if !target.workflow_ids.iter().any(|id| id == workflow_id) {
        target.workflow_ids.push(workflow_id.to_string());
    }

    Ok(())
}

/// Removes `workflow_id` from a single project.
///
/// # Errors
/// Returns an error when `project_id` does not match any project.
pub fn unassign_workflow_from_project(
    projects: &mut [Project],
    project_id: &str,
    workflow_id: &str,
) -> Result<(), String> {
    let target = projects
        .iter_mut()
        .find(|project| project.id == project_id)
        .ok_or_else(|| format!("project {project_id} not found"))?;

    target.workflow_ids.retain(|id| id != workflow_id);
    Ok(())
}

/// Derives a display name from the last path segment, falling back to the full path.
#[must_use]
pub fn project_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// # Errors
/// Returns an error when the path is missing or not a directory.
pub fn create_project_from_path(path: impl AsRef<Path>) -> Result<Project, String> {
    let path = path.as_ref();
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("invalid project directory: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!(
            "project path is not a directory: {}",
            canonical.display()
        ));
    }
    let path_string = canonical.to_string_lossy().into_owned();
    let name = project_name_from_path(&canonical);
    Ok(Project::new(path_string, name))
}

/// # Errors
/// Returns an error when another project already uses the same canonical path.
pub fn validate_unique_project_path(projects: &[Project], path: &str) -> Result<(), String> {
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|error| format!("invalid project directory: {error}"))?;
    let canonical = canonical.to_string_lossy();
    if projects
        .iter()
        .any(|project| project.path == canonical.as_ref())
    {
        return Err(format!("project already exists for {}", canonical));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct FileProjectStore {
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct StoredProjects {
    projects: Vec<Project>,
}

impl FileProjectStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CURRENT_DATA_DIR_SLUG)
            .join(PROJECTS_FILE_NAME)
    }

    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(&self) -> io::Result<Vec<Project>> {
        if !self.path.exists() {
            if let Some(legacy_path) = legacy_store_path(&self.path) {
                if legacy_path.exists() {
                    return Self::new(legacy_path).load();
                }
            }
            return Ok(Vec::new());
        }

        let text = fs::read_to_string(&self.path)?;
        let stored: StoredProjects = serde_json::from_str(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("project store JSON invalid: {error}"),
            )
        })?;
        Ok(stored.projects)
    }

    /// # Errors
    /// Returns an error if the file cannot be serialized or written.
    pub fn save(&self, projects: &[Project]) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let stored = StoredProjects {
            projects: projects.to_vec(),
        };
        let text = serde_json::to_string_pretty(&stored).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("project store JSON serialization failed: {error}"),
            )
        })?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_store_loads_empty() {
        let dir = tempdir().unwrap();
        let store = FileProjectStore::new(dir.path().join("projects.json"));

        let projects = store.load().unwrap();

        assert!(projects.is_empty());
    }

    #[test]
    fn saves_and_loads_projects() {
        let dir = tempdir().unwrap();
        let store = FileProjectStore::new(dir.path().join("nested").join("projects.json"));
        let project = Project::new("/tmp/repo", "Repo");

        store.save(std::slice::from_ref(&project)).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded, vec![project]);
    }

    #[test]
    fn assign_workflow_allows_multiple_project_memberships() {
        let mut projects = vec![Project::new("/tmp/a", "A"), Project::new("/tmp/b", "B")];
        let workflow_id = "wf-1";
        let alpha_project_id = projects[0].id.clone();
        let beta_project_id = projects[1].id.clone();

        assign_workflow(&mut projects, &alpha_project_id, workflow_id).unwrap();
        assign_workflow(&mut projects, &beta_project_id, workflow_id).unwrap();
        assert_eq!(projects[0].workflow_ids, vec![workflow_id.to_string()]);
        assert_eq!(projects[1].workflow_ids, vec![workflow_id.to_string()]);

        unassign_workflow_from_project(&mut projects, &alpha_project_id, workflow_id).unwrap();
        assert!(projects[0].workflow_ids.is_empty());
        assert_eq!(projects[1].workflow_ids, vec![workflow_id.to_string()]);
    }

    #[test]
    fn cleanup_stored_projects_removes_legacy_global_rows() {
        let mut projects = vec![
            Project {
                id: "global".to_string(),
                path: String::new(),
                name: "Global".to_string(),
                metadata: ProjectMetadata::default(),
                workflow_ids: vec!["wf-1".to_string()],
                default_execution_cwd: String::new(),
            },
            Project::new("/tmp/a", "A"),
        ];

        cleanup_stored_projects(&mut projects);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "A");
    }

    #[test]
    fn create_project_from_path_uses_directory_name() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("my-repo");
        fs::create_dir_all(&nested).unwrap();

        let project = create_project_from_path(&nested).unwrap();

        assert_eq!(project.name, "my-repo");
        assert!(project.default_execution_cwd.ends_with("my-repo"));
    }
}

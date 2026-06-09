use super::ports::Project;
use std::path::Path;

/// Drops legacy global pseudo-project rows and deduplicates membership ids.
pub fn cleanup_stored_projects(projects: &mut Vec<Project>) -> bool {
    let before_len = projects.len();
    projects.retain(|project| project.id != "global" && !project.path.is_empty());
    let mut changed = before_len != projects.len();
    for project in projects.iter_mut() {
        let before_ids = project.workflow_ids.clone();
        project.workflow_ids.sort();
        project.workflow_ids.dedup();
        if project.workflow_ids != before_ids {
            changed = true;
        }
    }
    changed
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

fn project_name_from_path(path: &Path) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ports::ProjectMetadata;
    use std::fs;
    use tempfile::tempdir;

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

use domain::Workflow;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const FLOW_DIR_NAME: &str = ".flow";
const WORKFLOWS_DIR_NAME: &str = "workflows";
const WORKFLOW_FILE_SUFFIX: &str = ".workflow.json";

/// Returns `{project_root}/.flow/workflows`.
#[must_use]
pub fn flow_workflows_dir(project_root: &Path) -> PathBuf {
    project_root.join(FLOW_DIR_NAME).join(WORKFLOWS_DIR_NAME)
}

fn workflow_file_path(project_root: &Path, workflow_id: &str) -> PathBuf {
    flow_workflows_dir(project_root).join(format!("{workflow_id}{WORKFLOW_FILE_SUFFIX}"))
}

/// Discovers workflow files under `{project_root}/.flow/workflows/*.workflow.json`.
///
/// # Errors
/// Returns an error if the directory cannot be read.
pub fn discover_project_workflows(project_root: &Path) -> io::Result<Vec<Workflow>> {
    let dir = flow_workflows_dir(project_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut by_id = BTreeMap::<String, Workflow>::new();
    for entry in WalkDir::new(&dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(WORKFLOW_FILE_SUFFIX) {
            continue;
        }
        let text = fs::read_to_string(path)?;
        let workflow: Workflow = serde_json::from_str(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("workflow file {} invalid: {error}", path.display()),
            )
        })?;
        by_id.insert(workflow.id.to_string(), workflow);
    }

    Ok(by_id.into_values().collect())
}

/// # Errors
/// Returns an error if the workflow cannot be serialized or written.
pub fn save_project_workflow(project_root: &Path, workflow: &Workflow) -> io::Result<()> {
    let dir = flow_workflows_dir(project_root);
    fs::create_dir_all(&dir)?;

    let path = workflow_file_path(project_root, &workflow.id);
    let text = serde_json::to_string_pretty(workflow).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("workflow JSON serialization failed: {error}"),
        )
    })?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

/// # Errors
/// Returns an error if workflow files cannot be written.
pub fn save_project_workflows(project_root: &Path, workflows: &[Workflow]) -> io::Result<()> {
    for workflow in workflows {
        save_project_workflow(project_root, workflow)?;
    }
    Ok(())
}

/// # Errors
/// Returns an error if the workflow file cannot be removed.
pub fn delete_project_workflow(project_root: &Path, workflow_id: &str) -> io::Result<()> {
    let path = workflow_file_path(project_root, workflow_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discovers_and_saves_project_workflows() {
        let dir = tempdir().unwrap();
        let workflow = Workflow::new("Repo flow");

        save_project_workflow(dir.path(), &workflow).unwrap();
        let loaded = discover_project_workflows(dir.path()).unwrap();

        assert_eq!(loaded, vec![workflow]);
        assert!(flow_workflows_dir(dir.path()).exists());
    }

    #[test]
    fn delete_project_workflow_removes_file() {
        let dir = tempdir().unwrap();
        let workflow = Workflow::new("Temporary");

        save_project_workflow(dir.path(), &workflow).unwrap();
        delete_project_workflow(dir.path(), &workflow.id).unwrap();

        assert!(discover_project_workflows(dir.path()).unwrap().is_empty());
    }
}

use crate::ipc_types::CommandError;
use orchestration::backend::AppBackend;
use orchestration::{Project, ProjectFileReference};

#[tauri::command]
pub fn list_projects(backend: tauri::State<AppBackend>) -> Result<Vec<Project>, CommandError> {
    Ok(backend.list_projects()?)
}

#[tauri::command]
pub fn list_project_file_references(
    backend: tauri::State<AppBackend>,
    execution_cwd: String,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ProjectFileReference>, CommandError> {
    Ok(backend.list_project_file_references(execution_cwd, query, limit)?)
}

#[tauri::command]
pub fn save_projects(
    backend: tauri::State<AppBackend>,
    projects: Vec<Project>,
) -> Result<(), CommandError> {
    Ok(backend.save_projects(&projects)?)
}

#[tauri::command]
pub fn create_project_from_directory(
    backend: tauri::State<AppBackend>,
    path: String,
) -> Result<Project, CommandError> {
    Ok(backend.create_project_from_directory(path)?)
}

#[tauri::command]
pub fn assign_workflow_to_project(
    backend: tauri::State<AppBackend>,
    project_id: String,
    workflow_id: String,
) -> Result<Vec<Project>, CommandError> {
    Ok(backend.assign_workflow_to_project(&project_id, &workflow_id)?)
}

#[tauri::command]
pub fn copy_workflow_to_project(
    backend: tauri::State<AppBackend>,
    target_project_id: String,
    source_workflow_id: String,
) -> Result<orchestration::api::CopyWorkflowToProjectResult, CommandError> {
    Ok(backend.copy_workflow_to_project(&target_project_id, &source_workflow_id)?)
}

#[tauri::command]
pub fn unassign_workflow_from_project(
    backend: tauri::State<AppBackend>,
    project_id: String,
    workflow_id: String,
) -> Result<Vec<Project>, CommandError> {
    Ok(backend.unassign_workflow_from_project(&project_id, &workflow_id)?)
}

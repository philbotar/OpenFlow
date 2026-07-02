use crate::ipc_types::CommandError;
use orchestration::backend::{AppBackend, WorkflowAuthoringTurnResult};
use orchestration::{AppSettings, Workflow};

#[tauri::command]
pub fn start_workflow_authoring(
    backend: tauri::State<AppBackend>,
    base_workflow: Option<Workflow>,
) -> Result<String, CommandError> {
    Ok(backend.start_workflow_authoring(base_workflow))
}

#[tauri::command]
pub fn end_workflow_authoring(
    backend: tauri::State<AppBackend>,
    session_id: String,
) -> Result<bool, CommandError> {
    Ok(backend.end_workflow_authoring(&session_id))
}

#[tauri::command]
pub async fn workflow_authoring_turn(
    backend: tauri::State<'_, AppBackend>,
    session_id: String,
    message: String,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> Result<WorkflowAuthoringTurnResult, CommandError> {
    Ok(backend
        .workflow_authoring_turn(session_id, message, &settings, transient_api_key.as_deref())
        .await?)
}

use crate::ipc_types::CommandError;
use orchestration::api::{WorkflowAuthoringDraftEvent, WorkflowAuthoringStartResult, WorkflowAuthoringThinkingEvent};
use orchestration::backend::{AppBackend, WorkflowAuthoringTurnResult};
use orchestration::{AppSettings, Workflow};
use tauri::Emitter;

#[tauri::command]
pub fn start_workflow_authoring(
    backend: tauri::State<AppBackend>,
    base_workflow: Option<Workflow>,
    target_project_id: Option<String>,
) -> Result<WorkflowAuthoringStartResult, CommandError> {
    Ok(backend.start_workflow_authoring(base_workflow, target_project_id.as_deref())?)
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
    app: tauri::AppHandle,
    backend: tauri::State<'_, AppBackend>,
    session_id: String,
    message: String,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> Result<WorkflowAuthoringTurnResult, CommandError> {
    let thinking_app = app.clone();
    let draft_app = app;
    Ok(backend
        .workflow_authoring_turn(
            session_id,
            message,
            &settings,
            transient_api_key.as_deref(),
            move |event: WorkflowAuthoringThinkingEvent| {
                let _ = thinking_app.emit("workflow-authoring-thinking", event);
            },
            move |event: WorkflowAuthoringDraftEvent| {
                let _ = draft_app.emit("workflow-authoring-draft", event);
            },
        )
        .await?)
}

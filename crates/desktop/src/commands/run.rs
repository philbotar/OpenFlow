use crate::ipc_types::CommandError;
use crate::run_event_bridge::spawn_run_event_bridge;
use crate::run_sleep_guard;
use orchestration::backend::{AppBackend, FileEditPreview};
use orchestration::run::state::WorkflowRunState;
use orchestration::{AppSettings, Workflow};
use tauri::Emitter;

#[tauri::command]
pub async fn start_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    workflow: Workflow,
    settings: AppSettings,
    execution_cwd: Option<String>,
    transient_api_key: Option<String>,
    entrypoint: Option<String>,
) -> Result<WorkflowRunState, CommandError> {
    let workflow_name = workflow.name.clone();
    let (initial_state, event_rx) = backend
        .start_run(
            workflow,
            entrypoint,
            execution_cwd,
            &settings,
            transient_api_key.as_deref(),
        )
        .await?;
    spawn_run_event_bridge(app, workflow_name, event_rx, initial_state.run_id.clone());
    Ok(initial_state)
}

#[tauri::command]
pub async fn continue_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    workflow: Workflow,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> Result<WorkflowRunState, CommandError> {
    let workflow_name = workflow.name.clone();
    let (initial_state, event_rx) = backend
        .continue_run(workflow, None, &settings, transient_api_key.as_deref())
        .await?;
    spawn_run_event_bridge(app, workflow_name, event_rx, initial_state.run_id.clone());
    Ok(initial_state)
}

#[tauri::command]
pub async fn is_run_continuable(
    backend: tauri::State<'_, AppBackend>,
) -> Result<bool, CommandError> {
    Ok(backend.is_run_continuable().await)
}

#[tauri::command]
pub fn list_runs(
    backend: tauri::State<'_, AppBackend>,
    workflow_id: Option<String>,
) -> Result<Vec<orchestration::run::persistence::RunSummary>, CommandError> {
    Ok(backend.list_runs(workflow_id.as_deref())?)
}

#[tauri::command]
pub fn replay_run(
    backend: tauri::State<'_, AppBackend>,
    run_id: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.replay_run(&run_id)?)
}

#[tauri::command]
pub async fn resume_durable_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    run_id: String,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> Result<WorkflowRunState, CommandError> {
    let (initial_state, event_rx, workflow_name) = backend
        .resume_durable_run(&run_id, &settings, transient_api_key.as_deref())
        .await?;
    spawn_run_event_bridge(app, workflow_name, event_rx, initial_state.run_id.clone());
    Ok(initial_state)
}

#[tauri::command]
pub async fn preview_file_edit(
    backend: tauri::State<'_, AppBackend>,
    approval_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<FileEditPreview, CommandError> {
    Ok(backend
        .preview_file_edit(&approval_id, tool_name, arguments)
        .await?)
}

#[tauri::command]
pub async fn git_diff_file(
    backend: tauri::State<'_, AppBackend>,
    path: String,
) -> Result<String, CommandError> {
    Ok(backend.git_diff_file(path).await?)
}

#[tauri::command]
pub async fn revert_edit_batch(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    batch_id: String,
) -> Result<WorkflowRunState, CommandError> {
    let run_state = backend.revert_edit_batch(batch_id).await?;
    let _ = app.emit("run-state", run_state.clone());
    Ok(run_state)
}

#[tauri::command]
pub async fn stop_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
) -> Result<WorkflowRunState, CommandError> {
    let run_state = backend.stop_run().await?;
    run_sleep_guard::stop_for_app(&app);
    let _ = app.emit("run-state", run_state.clone());
    Ok(run_state)
}

#[tauri::command]
pub async fn interrupt_node(
    backend: tauri::State<'_, AppBackend>,
    node_id: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.interrupt_node(&node_id).await?)
}

#[tauri::command]
pub async fn retry_node(
    backend: tauri::State<'_, AppBackend>,
    node_id: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.retry_node(&node_id).await?)
}

#[tauri::command]
pub async fn update_node_runtime_config(
    backend: tauri::State<'_, AppBackend>,
    node_id: String,
    update: orchestration::api::NodeRuntimeConfigUpdate,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.update_node_runtime_config(&node_id, update).await?)
}

#[tauri::command]
pub async fn submit_user_input(
    backend: tauri::State<'_, AppBackend>,
    node_id: String,
    text: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.submit_user_input(&node_id, text).await?)
}

#[tauri::command]
pub async fn submit_tool_approval(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    approval_id: String,
    allow: bool,
    reason: Option<String>,
) -> Result<WorkflowRunState, CommandError> {
    let run_state = backend
        .submit_tool_approval(&approval_id, allow, reason)
        .await?;
    let _ = app.emit("run-state", run_state.clone());
    Ok(run_state)
}

#[tauri::command]
pub async fn get_run_state(
    backend: tauri::State<'_, AppBackend>,
) -> Result<Option<WorkflowRunState>, CommandError> {
    Ok(backend.get_run_state().await)
}

#[tauri::command]
pub async fn clear_run_trace(
    backend: tauri::State<'_, AppBackend>,
) -> Result<Option<WorkflowRunState>, CommandError> {
    Ok(backend.clear_run_trace().await?)
}

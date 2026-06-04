use agent_workflow_app::agent_store::AgentDefinition;
use agent_workflow_app::backend::{
    AgentDefinitionSummary, AppBackend, BackendError, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use agent_workflow_app::settings_store::AppSettings;
use agent_workflow_app::state::WorkflowRunState;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use thiserror::Error;
use workflow_core::{Node, Workflow};

const RUN_STATE_EVENT: &str = "run-state";

type SharedBackend = Arc<AppBackend>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    workflows: Vec<Workflow>,
    agents: Vec<AgentDefinition>,
    settings: AppSettings,
    run_state: Option<WorkflowRunState>,
}

#[derive(Debug, Error)]
enum CommandError {
    #[error(transparent)]
    Backend(#[from] BackendError),
    #[error("failed to emit run state event: {0}")]
    Emit(String),
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[tauri::command]
async fn bootstrap_app(
    backend: State<'_, SharedBackend>,
) -> Result<BootstrapPayload, CommandError> {
    Ok(BootstrapPayload {
        workflows: backend.load_all_workflows()?,
        agents: backend.load_agents()?,
        settings: backend.load_settings()?,
        run_state: backend.get_run_state().await,
    })
}

#[tauri::command]
fn list_workflows(
    backend: State<'_, SharedBackend>,
) -> Result<Vec<WorkflowListItem>, CommandError> {
    backend.list_workflows().map_err(CommandError::from)
}

#[tauri::command]
fn load_all_workflows(backend: State<'_, SharedBackend>) -> Result<Vec<Workflow>, CommandError> {
    backend.load_all_workflows().map_err(CommandError::from)
}

#[tauri::command]
fn load_workflow(
    workflow_id: String,
    backend: State<'_, SharedBackend>,
) -> Result<Workflow, CommandError> {
    backend
        .load_workflow(&workflow_id)
        .map_err(CommandError::from)
}

#[tauri::command]
fn create_workflow(
    name: String,
    backend: State<'_, SharedBackend>,
) -> Result<Workflow, CommandError> {
    backend.create_workflow(name).map_err(CommandError::from)
}

#[tauri::command]
fn save_workflow(
    workflow: Workflow,
    backend: State<'_, SharedBackend>,
) -> Result<Workflow, CommandError> {
    backend.save_workflow(workflow).map_err(CommandError::from)
}

#[tauri::command]
fn save_workflows(
    workflows: Vec<Workflow>,
    backend: State<'_, SharedBackend>,
) -> Result<(), CommandError> {
    backend
        .save_workflows(&workflows)
        .map_err(CommandError::from)
}

#[tauri::command]
fn rename_workflow(
    workflow_id: String,
    name: String,
    backend: State<'_, SharedBackend>,
) -> Result<WorkflowListItem, CommandError> {
    backend
        .rename_workflow(&workflow_id, name)
        .map_err(CommandError::from)
}

#[tauri::command]
fn list_agents(
    backend: State<'_, SharedBackend>,
) -> Result<Vec<AgentDefinitionSummary>, CommandError> {
    backend.list_agents().map_err(CommandError::from)
}

#[tauri::command]
fn load_agents(backend: State<'_, SharedBackend>) -> Result<Vec<AgentDefinition>, CommandError> {
    backend.load_agents().map_err(CommandError::from)
}

#[tauri::command]
fn create_agent_definition(
    name: String,
    backend: State<'_, SharedBackend>,
) -> Result<AgentDefinition, CommandError> {
    backend
        .create_agent_definition(name)
        .map_err(CommandError::from)
}

#[tauri::command]
fn save_agents(
    agents: Vec<AgentDefinition>,
    backend: State<'_, SharedBackend>,
) -> Result<(), CommandError> {
    backend.save_agents(&agents).map_err(CommandError::from)
}

#[tauri::command]
fn load_settings(backend: State<'_, SharedBackend>) -> Result<AppSettings, CommandError> {
    backend.load_settings().map_err(CommandError::from)
}

#[tauri::command]
fn save_settings(
    settings: AppSettings,
    backend: State<'_, SharedBackend>,
) -> Result<(), CommandError> {
    backend.save_settings(&settings).map_err(CommandError::from)
}

#[tauri::command]
fn resolve_provider_readiness(
    settings: AppSettings,
    backend: State<'_, SharedBackend>,
) -> ProviderReadiness {
    backend.resolve_provider_readiness(&settings, None)
}

#[tauri::command]
fn validate_workflow(
    workflow: Workflow,
    backend: State<'_, SharedBackend>,
) -> Result<WorkflowValidationSummary, CommandError> {
    backend
        .validate_workflow(&workflow)
        .map_err(CommandError::from)
}

#[tauri::command]
fn create_agent_node(index: usize, x: f32, y: f32) -> Node {
    Node::agent(format!("Agent {}", index + 1), x, y)
}

#[tauri::command]
async fn start_run(
    app: AppHandle,
    workflow: Workflow,
    settings: AppSettings,
    backend: State<'_, SharedBackend>,
) -> Result<WorkflowRunState, CommandError> {
    let mut event_rx = backend.start_run(workflow, None, &settings, None).await?;
    let backend_handle = backend.inner().clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match backend_handle.apply_execution_event(event).await {
                Ok(run_state) => {
                    let _ = app_handle.emit(RUN_STATE_EVENT, &run_state);
                }
                Err(_) => break,
            }
        }
    });

    backend
        .get_run_state()
        .await
        .ok_or(BackendError::NoActiveRun)
        .map_err(CommandError::from)
}

#[tauri::command]
async fn submit_user_input(
    app: AppHandle,
    node_id: String,
    text: String,
    backend: State<'_, SharedBackend>,
) -> Result<WorkflowRunState, CommandError> {
    let run_state = backend.submit_user_input(&node_id, text).await?;
    app.emit(RUN_STATE_EVENT, &run_state)
        .map_err(|error| CommandError::Emit(error.to_string()))?;
    Ok(run_state)
}

#[tauri::command]
async fn complete_manual_node(
    app: AppHandle,
    backend: State<'_, SharedBackend>,
) -> Result<WorkflowRunState, CommandError> {
    let run_state = backend.complete_manual_node().await?;
    app.emit(RUN_STATE_EVENT, &run_state)
        .map_err(|error| CommandError::Emit(error.to_string()))?;
    Ok(run_state)
}

#[tauri::command]
async fn get_run_state(
    backend: State<'_, SharedBackend>,
) -> Result<Option<WorkflowRunState>, CommandError> {
    Ok(backend.get_run_state().await)
}

#[tauri::command]
async fn clear_run_trace(
    app: AppHandle,
    backend: State<'_, SharedBackend>,
) -> Result<Option<WorkflowRunState>, CommandError> {
    let run_state = backend.clear_run_trace().await?;
    if let Some(state) = &run_state {
        app.emit(RUN_STATE_EVENT, state)
            .map_err(|error| CommandError::Emit(error.to_string()))?;
    }
    Ok(run_state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(AppBackend::with_default_paths()))
        .invoke_handler(tauri::generate_handler![
            complete_manual_node,
            bootstrap_app,
            list_workflows,
            list_agents,
            load_agents,
            create_agent_definition,
            save_agents,
            load_all_workflows,
            load_workflow,
            create_workflow,
            save_workflow,
            save_workflows,
            rename_workflow,
            load_settings,
            save_settings,
            resolve_provider_readiness,
            validate_workflow,
            create_agent_node,
            start_run,
            submit_user_input,
            get_run_state,
            clear_run_trace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#![allow(clippy::cargo, clippy::nursery, clippy::pedantic)]

use agent_workflow_app::agent_store::AgentDefinition;
use agent_workflow_app::backend::{
    AppBackend, BackendError, ProviderReadiness, WorkflowListItem, WorkflowValidationSummary,
};
use agent_workflow_app::settings_store::AppSettings;
use agent_workflow_app::state::WorkflowRunState;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use workflow_core::Workflow;

/// Bootstrap payload returned on app startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    workflows: Vec<Workflow>,
    agents: Vec<AgentDefinition>,
    settings: AppSettings,
    run_state: Option<WorkflowRunState>,
}

/// Error type returned to Tauri frontend.
#[derive(Debug, thiserror::Error)]
enum CommandError {
    #[error(transparent)]
    Backend(#[from] BackendError),
}

impl serde::Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Tauri command: Initialize the application.
#[tauri::command]
async fn bootstrap_app(
    backend: tauri::State<'_, AppBackend>,
) -> Result<BootstrapPayload, CommandError> {
    let workflows = backend.load_all_workflows()?;
    let agents = backend.load_agents()?;
    let settings = backend.load_settings()?;
    let run_state = backend.get_run_state().await;
    Ok(BootstrapPayload {
        workflows,
        agents,
        settings,
        run_state,
    })
}

/// Tauri command: List workflow metadata.
#[tauri::command]
fn list_workflows(
    backend: tauri::State<AppBackend>,
) -> Result<Vec<WorkflowListItem>, CommandError> {
    Ok(backend.list_workflows()?)
}

/// Tauri command: Load all workflows.
#[tauri::command]
fn load_all_workflows(backend: tauri::State<AppBackend>) -> Result<Vec<Workflow>, CommandError> {
    Ok(backend.load_all_workflows()?)
}

/// Tauri command: Load a single workflow.
#[tauri::command]
fn load_workflow(
    backend: tauri::State<AppBackend>,
    workflow_id: String,
) -> Result<Workflow, CommandError> {
    Ok(backend.load_workflow(&workflow_id)?)
}

/// Tauri command: Create a new workflow.
#[tauri::command]
fn create_workflow(
    backend: tauri::State<AppBackend>,
    name: String,
) -> Result<Workflow, CommandError> {
    Ok(backend.create_workflow(name)?)
}

/// Tauri command: Save a workflow.
#[tauri::command]
fn save_workflow(
    backend: tauri::State<AppBackend>,
    workflow: Workflow,
) -> Result<Workflow, CommandError> {
    Ok(backend.save_workflow(workflow)?)
}

/// Tauri command: Save multiple workflows.
#[tauri::command]
fn save_workflows(
    backend: tauri::State<AppBackend>,
    workflows: Vec<Workflow>,
) -> Result<(), CommandError> {
    Ok(backend.save_workflows(&workflows)?)
}

/// Tauri command: Rename a workflow.
#[tauri::command]
fn rename_workflow(
    backend: tauri::State<AppBackend>,
    workflow_id: String,
    name: String,
) -> Result<WorkflowListItem, CommandError> {
    Ok(backend.rename_workflow(&workflow_id, name)?)
}

/// Tauri command: List agent summaries.
#[tauri::command]
fn list_agents(
    backend: tauri::State<'_, AppBackend>,
) -> Result<Vec<agent_workflow_app::backend::AgentDefinitionSummary>, CommandError> {
    Ok(backend.list_agents()?)
}

/// Tauri command: Load all agents.
#[tauri::command]
fn load_agents(backend: tauri::State<AppBackend>) -> Result<Vec<AgentDefinition>, CommandError> {
    Ok(backend.load_agents()?)
}

/// Tauri command: Create a new agent definition.
#[tauri::command]
fn create_agent_definition(
    backend: tauri::State<AppBackend>,
    name: String,
) -> Result<AgentDefinition, CommandError> {
    Ok(backend.create_agent_definition(name)?)
}

/// Tauri command: Save agents.
#[tauri::command]
fn save_agents(
    backend: tauri::State<AppBackend>,
    agents: Vec<AgentDefinition>,
) -> Result<(), CommandError> {
    Ok(backend.save_agents(&agents)?)
}

/// Tauri command: Load settings.
#[tauri::command]
fn load_settings(backend: tauri::State<AppBackend>) -> Result<AppSettings, CommandError> {
    Ok(backend.load_settings()?)
}

/// Tauri command: Save settings.
#[tauri::command]
fn save_settings(
    backend: tauri::State<AppBackend>,
    settings: AppSettings,
) -> Result<(), CommandError> {
    Ok(backend.save_settings(&settings)?)
}

/// Tauri command: Check provider readiness.
#[tauri::command]
fn resolve_provider_readiness(
    backend: tauri::State<AppBackend>,
    settings: AppSettings,
) -> ProviderReadiness {
    backend.resolve_provider_readiness(&settings, None)
}

/// Tauri command: Validate a workflow.
#[tauri::command]
fn validate_workflow(
    backend: tauri::State<AppBackend>,
    workflow: Workflow,
) -> Result<WorkflowValidationSummary, CommandError> {
    Ok(backend.validate_workflow(&workflow)?)
}

/// Tauri command: Create an agent node from a saved agent definition.
#[tauri::command]
fn create_agent_node(
    backend: tauri::State<AppBackend>,
    index: usize,
    x: f32,
    y: f32,
    agent_id: Option<String>,
) -> Result<workflow_core::Node, CommandError> {
    Ok(backend.create_agent_node(index, x, y, agent_id.as_deref())?)
}

/// Tauri command: Start a workflow run.
#[tauri::command]
async fn start_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    workflow: Workflow,
    settings: AppSettings,
) -> Result<WorkflowRunState, CommandError> {
    let (initial_state, mut event_rx) = backend.start_run(workflow, None, &settings, None).await?;
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let backend = app_handle.state::<AppBackend>();
            match backend.apply_execution_event(event).await {
                Ok(run_state) => {
                    let _ = app_handle.emit("run-state", run_state.clone());
                    if !run_state.active {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    Ok(initial_state)
}

/// Tauri command: Submit user input to a node.
#[tauri::command]
async fn submit_user_input(
    backend: tauri::State<'_, AppBackend>,
    node_id: String,
    text: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.submit_user_input(&node_id, text).await?)
}

/// Tauri command: Submit a tool approval decision.
#[tauri::command]
async fn submit_tool_approval(
    backend: tauri::State<'_, AppBackend>,
    approval_id: String,
    allow: bool,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.submit_tool_approval(&approval_id, allow).await?)
}

/// Tauri command: Complete a manual node.
#[tauri::command]
async fn complete_manual_node(
    backend: tauri::State<'_, AppBackend>,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.complete_manual_node().await?)
}

#[tauri::command]
async fn get_run_state(
    backend: tauri::State<'_, AppBackend>,
) -> Result<Option<WorkflowRunState>, CommandError> {
    Ok(backend.get_run_state().await)
}

/// Tauri command: Clear run trace.
#[tauri::command]
async fn clear_run_trace(
    backend: tauri::State<'_, AppBackend>,
) -> Result<Option<WorkflowRunState>, CommandError> {
    Ok(backend.clear_run_trace().await?)
}

pub fn run() {
    let backend = AppBackend::with_default_paths();

    let builder = tauri::Builder::default();

    builder
        .manage(backend)
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            bootstrap_app,
            list_workflows,
            load_all_workflows,
            load_workflow,
            create_workflow,
            save_workflow,
            save_workflows,
            rename_workflow,
            list_agents,
            load_agents,
            create_agent_definition,
            save_agents,
            load_settings,
            save_settings,
            resolve_provider_readiness,
            validate_workflow,
            create_agent_node,
            start_run,
            submit_user_input,
            submit_tool_approval,
            complete_manual_node,
            get_run_state,
            clear_run_trace,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

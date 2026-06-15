#![allow(
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    reason = "Tauri desktop shell; strict pedantic/nursery lint not enforced on thin IPC glue"
)]

mod run_sleep_guard;

use orchestration::backend::{
    AppBackend, BackendError, FileEditPreview, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
use orchestration::run::execution::ExecutionEvent;
use orchestration::run::state::WorkflowRunState;
use orchestration::terminal::TerminalStart;
use orchestration::{AgentDefinition, AppSettings, SkillSummary};
use orchestration::{Project, ProjectFileReference, ProjectFileReferenceContent, Workflow};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tokio::sync::mpsc::UnboundedReceiver;

const TERMINAL_EVENT: &str = "terminal-event";

/// Bootstrap payload returned on app startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    workflows: Vec<Workflow>,
    agents: Vec<AgentDefinition>,
    projects: Vec<Project>,
    skills: Vec<SkillSummary>,
    settings: AppSettings,
    run_state: Option<WorkflowRunState>,
    run_continuable: bool,
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
    let projects = backend.list_projects()?;
    let skills = backend.list_skills()?;
    let settings = backend.load_settings()?;
    let run_state = backend.get_run_state().await;
    let run_continuable = backend.is_run_continuable().await;
    Ok(BootstrapPayload {
        workflows,
        agents,
        projects,
        skills,
        settings,
        run_state,
        run_continuable,
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
) -> Result<Vec<orchestration::backend::AgentDefinitionSummary>, CommandError> {
    Ok(backend.list_agents()?)
}

/// Tauri command: List discovered skills.
#[tauri::command]
fn list_skills(backend: tauri::State<AppBackend>) -> Result<Vec<SkillSummary>, CommandError> {
    Ok(backend.list_skills()?)
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

/// Tauri command: Load a provider API key from the OS credential store.
#[tauri::command]
fn load_provider_api_key(
    backend: tauri::State<AppBackend>,
    provider_id: String,
) -> Result<Option<String>, CommandError> {
    Ok(backend.load_provider_api_key(&provider_id)?)
}

/// Tauri command: Save a provider API key to the OS credential store.
#[tauri::command]
fn save_provider_api_key(
    backend: tauri::State<AppBackend>,
    provider_id: String,
    api_key: String,
) -> Result<(), CommandError> {
    Ok(backend.save_provider_api_key(&provider_id, &api_key)?)
}

/// Tauri command: Delete a provider API key from the OS credential store.
#[tauri::command]
fn delete_provider_api_key(
    backend: tauri::State<AppBackend>,
    provider_id: String,
) -> Result<(), CommandError> {
    Ok(backend.delete_provider_api_key(&provider_id)?)
}

/// Tauri command: Check provider readiness.
#[tauri::command]
fn resolve_provider_readiness(
    backend: tauri::State<AppBackend>,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> ProviderReadiness {
    backend.resolve_provider_readiness(&settings, transient_api_key.as_deref())
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
) -> Result<orchestration::Node, CommandError> {
    Ok(backend.create_agent_node(index, x, y, agent_id.as_deref())?)
}

const RUN_STATE_COALESCE_WINDOW: std::time::Duration = std::time::Duration::from_millis(30);

fn spawn_run_event_bridge(app: tauri::AppHandle, mut event_rx: UnboundedReceiver<ExecutionEvent>) {
    run_sleep_guard::start_for_app(&app);
    tauri::async_runtime::spawn(async move {
        let mut failed = false;
        while !failed {
            let Some(event) = event_rx.recv().await else {
                break;
            };
            let backend = app.state::<AppBackend>();
            let mut run_state = match backend.apply_execution_event(event).await {
                Ok(state) => state,
                Err(_) => break,
            };
            let deadline = tokio::time::Instant::now() + RUN_STATE_COALESCE_WINDOW;
            while run_state.active {
                tokio::select! {
                    () = tokio::time::sleep_until(deadline) => break,
                    maybe_event = event_rx.recv() => match maybe_event {
                        Some(event) => match backend.apply_execution_event(event).await {
                            Ok(state) => run_state = state,
                            Err(_) => {
                                failed = true;
                                break;
                            }
                        },
                        None => break,
                    },
                }
            }
            let active = run_state.active;
            let _ = app.emit("run-state", run_state);
            if !active {
                let backend = app.state::<AppBackend>();
                if !backend.is_run_active().await {
                    run_sleep_guard::stop_for_app(&app);
                }
                break;
            }
        }
        let backend = app.state::<AppBackend>();
        if !backend.is_run_active().await {
            run_sleep_guard::stop_for_app(&app);
        }
    });
}

/// Tauri command: Start a workflow run.
#[tauri::command]
async fn start_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    workflow: Workflow,
    settings: AppSettings,
    execution_cwd: Option<String>,
    transient_api_key: Option<String>,
    entrypoint: Option<String>,
) -> Result<WorkflowRunState, CommandError> {
    let (initial_state, event_rx) = backend
        .start_run(
            workflow,
            entrypoint,
            execution_cwd,
            &settings,
            transient_api_key.as_deref(),
        )
        .await?;
    spawn_run_event_bridge(app, event_rx);
    Ok(initial_state)
}

/// Tauri command: Continue a stopped workflow run from the in-session checkpoint.
#[tauri::command]
async fn continue_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    workflow: Workflow,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> Result<WorkflowRunState, CommandError> {
    let (initial_state, event_rx) = backend
        .continue_run(workflow, None, &settings, transient_api_key.as_deref())
        .await?;
    spawn_run_event_bridge(app, event_rx);
    Ok(initial_state)
}

/// Tauri command: Whether a stopped run can be resumed in this session.
#[tauri::command]
async fn is_run_continuable(backend: tauri::State<'_, AppBackend>) -> Result<bool, CommandError> {
    Ok(backend.is_run_continuable().await)
}

/// Tauri command: Preview write-tier file edits before approval.
#[tauri::command]
async fn preview_file_edit(
    backend: tauri::State<'_, AppBackend>,
    approval_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<FileEditPreview, CommandError> {
    Ok(backend
        .preview_file_edit(&approval_id, tool_name, arguments)
        .await?)
}

/// Tauri command: Return `git diff` for a file under the active run cwd.
#[tauri::command]
async fn git_diff_file(
    backend: tauri::State<'_, AppBackend>,
    path: String,
) -> Result<String, CommandError> {
    Ok(backend.git_diff_file(path).await?)
}

/// Tauri command: Restore files from a recorded edit batch.
#[tauri::command]
async fn revert_edit_batch(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    batch_id: String,
) -> Result<WorkflowRunState, CommandError> {
    let run_state = backend.revert_edit_batch(batch_id).await?;
    let _ = app.emit("run-state", run_state.clone());
    Ok(run_state)
}

/// Tauri command: Stop the active workflow run.
#[tauri::command]
async fn stop_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
) -> Result<WorkflowRunState, CommandError> {
    let run_state = backend.stop_run().await?;
    run_sleep_guard::stop_for_app(&app);
    let _ = app.emit("run-state", run_state.clone());
    Ok(run_state)
}

/// Tauri command: Interrupt an in-flight AI call for a single node.
#[tauri::command]
async fn interrupt_node(
    backend: tauri::State<'_, AppBackend>,
    node_id: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.interrupt_node(&node_id).await?)
}

/// Tauri command: Retry a failed or interrupted node.
#[tauri::command]
async fn retry_node(
    backend: tauri::State<'_, AppBackend>,
    node_id: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.retry_node(&node_id).await?)
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

#[tauri::command]
async fn start_terminal(
    app: tauri::AppHandle,
    backend: tauri::State<'_, AppBackend>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<TerminalStart, CommandError> {
    let (session, mut events) = backend.start_terminal(cwd.as_deref(), cols, rows)?;
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = events.recv().await {
            let _ = app_handle.emit(TERMINAL_EVENT, event);
        }
    });
    Ok(session)
}

#[tauri::command]
fn write_terminal(
    backend: tauri::State<AppBackend>,
    session_id: String,
    data: String,
) -> Result<(), CommandError> {
    Ok(backend.write_terminal(&session_id, &data)?)
}

#[tauri::command]
fn resize_terminal(
    backend: tauri::State<AppBackend>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), CommandError> {
    Ok(backend.resize_terminal(&session_id, cols, rows)?)
}

#[tauri::command]
fn stop_terminal(
    backend: tauri::State<AppBackend>,
    session_id: String,
) -> Result<(), CommandError> {
    Ok(backend.stop_terminal(&session_id)?)
}

/// Tauri command: List projects.
#[tauri::command]
fn list_projects(backend: tauri::State<AppBackend>) -> Result<Vec<Project>, CommandError> {
    Ok(backend.list_projects()?)
}

/// Tauri command: List files under an execution folder for chat @ references.
#[tauri::command]
fn list_project_file_references(
    backend: tauri::State<AppBackend>,
    execution_cwd: String,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ProjectFileReference>, CommandError> {
    Ok(backend.list_project_file_references(execution_cwd, query, limit)?)
}

/// Tauri command: Read selected project files for chat @ references.
#[tauri::command]
fn read_project_file_references(
    backend: tauri::State<AppBackend>,
    execution_cwd: String,
    paths: Vec<String>,
) -> Result<Vec<ProjectFileReferenceContent>, CommandError> {
    Ok(backend.read_project_file_references(execution_cwd, paths)?)
}

/// Tauri command: Save projects.
#[tauri::command]
fn save_projects(
    backend: tauri::State<AppBackend>,
    projects: Vec<Project>,
) -> Result<(), CommandError> {
    Ok(backend.save_projects(&projects)?)
}

/// Tauri command: Create a project from a directory path.
#[tauri::command]
fn create_project_from_directory(
    backend: tauri::State<AppBackend>,
    path: String,
) -> Result<Project, CommandError> {
    Ok(backend.create_project_from_directory(path)?)
}

/// Tauri command: Assign a workflow to a project.
#[tauri::command]
fn assign_workflow_to_project(
    backend: tauri::State<AppBackend>,
    project_id: String,
    workflow_id: String,
) -> Result<Vec<Project>, CommandError> {
    Ok(backend.assign_workflow_to_project(&project_id, &workflow_id)?)
}

/// Tauri command: Remove a workflow from any project.
#[tauri::command]
fn unassign_workflow_from_project(
    backend: tauri::State<AppBackend>,
    project_id: String,
    workflow_id: String,
) -> Result<Vec<Project>, CommandError> {
    Ok(backend.unassign_workflow_from_project(&project_id, &workflow_id)?)
}

/// Tauri command: List unresolved incident summaries.
#[tauri::command]
fn list_incidents(
    backend: tauri::State<'_, AppBackend>,
    limit: Option<usize>,
) -> Result<Vec<orchestration::api::IncidentSummary>, CommandError> {
    backend
        .list_incident_summaries(limit.unwrap_or(200))
        .map_err(|error| {
            CommandError::from(
                backend.backend_err(BackendError::ProjectOperation(error.to_string())),
            )
        })
}

/// Tauri command: Dismiss an incident by id.
#[tauri::command]
fn dismiss_incident(backend: tauri::State<'_, AppBackend>, id: String) -> Result<(), CommandError> {
    backend.dismiss_incident(&id).map_err(|error| {
        CommandError::from(backend.backend_err(BackendError::ProjectOperation(error.to_string())))
    })
}

/// Tauri command: Remove all resolved incidents from the store.
#[tauri::command]
fn clear_resolved_incidents(backend: tauri::State<'_, AppBackend>) -> Result<u32, CommandError> {
    backend
        .clear_resolved_incidents()
        .map(|count| count as u32)
        .map_err(|error| {
            CommandError::from(
                backend.backend_err(BackendError::ProjectOperation(error.to_string())),
            )
        })
}

pub fn run() {
    let builder = tauri::Builder::default();

    builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            bootstrap_app,
            list_projects,
            list_project_file_references,
            read_project_file_references,
            save_projects,
            create_project_from_directory,
            assign_workflow_to_project,
            unassign_workflow_from_project,
            list_workflows,
            load_all_workflows,
            load_workflow,
            create_workflow,
            save_workflow,
            save_workflows,
            rename_workflow,
            list_agents,
            list_skills,
            load_agents,
            create_agent_definition,
            save_agents,
            load_settings,
            save_settings,
            load_provider_api_key,
            save_provider_api_key,
            delete_provider_api_key,
            resolve_provider_readiness,
            validate_workflow,
            create_agent_node,
            start_run,
            continue_run,
            is_run_continuable,
            preview_file_edit,
            git_diff_file,
            revert_edit_batch,
            stop_run,
            interrupt_node,
            retry_node,
            submit_user_input,
            submit_tool_approval,
            complete_manual_node,
            get_run_state,
            clear_run_trace,
            list_incidents,
            dismiss_incident,
            clear_resolved_incidents,
            start_terminal,
            write_terminal,
            resize_terminal,
            stop_terminal,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let app_handle = window.app_handle().clone();
                tauri::async_runtime::block_on(async move {
                    let backend = app_handle.state::<AppBackend>();
                    if backend.is_run_active().await {
                        let _ = backend.stop_run().await;
                    }
                    backend.stop_all_terminals();
                    run_sleep_guard::stop_for_app(&app_handle);
                });
            }
        })
        .setup(|app| {
            let runtime_handle = tauri::async_runtime::handle().inner().clone();
            app.manage(AppBackend::with_runtime_handle(runtime_handle));
            app.manage(run_sleep_guard::RunSleepGuard::new());
            #[cfg(debug_assertions)]
            app.get_webview_window("main").unwrap().open_devtools();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

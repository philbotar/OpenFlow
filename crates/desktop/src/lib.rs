#![allow(
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    reason = "Tauri desktop shell; strict pedantic/nursery lint not enforced on thin IPC glue"
)]

mod run_notifications;
mod run_sleep_guard;

use orchestration::api::{McpDiscoveryRow, SettingsLoadPayload};
use orchestration::backend::{
    AppBackend, BackendError, FileEditPreview, ProviderReadiness, ScheduleStatus,
    WorkflowAuthoringTurnResult, WorkflowListItem, WorkflowValidationSummary,
};
use orchestration::run::execution::ExecutionEvent;
use orchestration::run::state::WorkflowRunState;
use orchestration::terminal::TerminalStart;
use orchestration::{AgentDefinition, AppSettings, McpServerConfig, SkillSummary};
use orchestration::{Project, ProjectFileReference, ProjectFileReferenceContent, Workflow};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tokio::sync::mpsc::UnboundedReceiver;

const TERMINAL_EVENT: &str = "terminal-event";
const SCHEDULE_EVENT: &str = "schedule-event";
const SCHEDULE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

fn emit_schedule_statuses(app: &tauri::AppHandle) {
    let backend = app.state::<AppBackend>();
    backend.tick_schedules();
    let _ = app.emit(SCHEDULE_EVENT, backend.list_schedule_statuses());
}

fn spawn_schedule_loop(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(SCHEDULE_POLL_INTERVAL);
        loop {
            interval.tick().await;
            let backend = app.state::<AppBackend>();
            match backend.start_due_scheduled_run().await {
                Ok(Some((initial_state, event_rx, workflow_name))) => {
                    let run_id = initial_state.run_id.clone();
                    let _ = app.emit("run-state", initial_state);
                    emit_schedule_statuses(&app);
                    spawn_run_event_bridge(app.clone(), workflow_name, event_rx, run_id);
                }
                Ok(None) => {
                    emit_schedule_statuses(&app);
                }
                Err(error) => {
                    log::warn!("scheduled workflow failed to start: {error}");
                    emit_schedule_statuses(&app);
                }
            }
        }
    });
}

/// Bootstrap payload returned on app startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    workflows: Vec<Workflow>,
    agents: Vec<AgentDefinition>,
    projects: Vec<Project>,
    skills: Vec<SkillSummary>,
    settings: AppSettings,
    discovered_mcp: Vec<McpDiscoveryRow>,
    run_state: Option<WorkflowRunState>,
    run_continuable: bool,
    schedule_statuses: Vec<ScheduleStatus>,
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
    let settings_payload = backend.load_settings(None)?;
    let run_state = backend.get_run_state().await;
    let run_continuable = backend.is_run_continuable().await;
    let schedule_statuses = backend.refresh_schedules()?;
    Ok(BootstrapPayload {
        workflows,
        agents,
        projects,
        skills,
        settings: settings_payload.settings,
        discovered_mcp: settings_payload.discovered_mcp,
        run_state,
        run_continuable,
        schedule_statuses,
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
fn load_all_workflows(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
) -> Result<Vec<Workflow>, CommandError> {
    let workflows = backend.load_all_workflows()?;
    emit_schedule_statuses(&app);
    Ok(workflows)
}

/// Tauri command: List workflow schedule statuses.
#[tauri::command]
fn list_schedule_statuses(
    backend: tauri::State<AppBackend>,
) -> Result<Vec<ScheduleStatus>, CommandError> {
    Ok(backend.list_schedule_statuses())
}

/// Tauri command: Refresh workflow schedule statuses.
#[tauri::command]
fn refresh_schedules(
    backend: tauri::State<AppBackend>,
) -> Result<Vec<ScheduleStatus>, CommandError> {
    Ok(backend.refresh_schedules()?)
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
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
    workflow: Workflow,
) -> Result<Workflow, CommandError> {
    let saved = backend.save_workflow(workflow)?;
    emit_schedule_statuses(&app);
    Ok(saved)
}

/// Tauri command: Save multiple workflows.
#[tauri::command]
fn save_workflows(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
    workflows: Vec<Workflow>,
) -> Result<(), CommandError> {
    backend.save_workflows(&workflows)?;
    emit_schedule_statuses(&app);
    Ok(())
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
fn load_settings(
    backend: tauri::State<AppBackend>,
    project_path: Option<String>,
) -> Result<SettingsLoadPayload, CommandError> {
    Ok(backend.load_settings(project_path.as_deref())?)
}

/// Tauri command: Save settings.
#[tauri::command]
fn save_settings(
    backend: tauri::State<AppBackend>,
    settings: AppSettings,
) -> Result<(), CommandError> {
    Ok(backend.save_settings(&settings)?)
}

/// Tauri command: Probe one MCP server and list tool names.
#[tauri::command]
async fn probe_mcp_server(
    backend: tauri::State<'_, AppBackend>,
    config: McpServerConfig,
) -> Result<Vec<String>, CommandError> {
    Ok(backend.probe_mcp_server(config).await?)
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

/// Tauri command: List Bedrock foundation models for the configured region/profile.
#[tauri::command]
async fn refresh_bedrock_models(
    backend: tauri::State<'_, AppBackend>,
    settings: AppSettings,
) -> Result<Vec<String>, CommandError> {
    Ok(backend.refresh_bedrock_models(&settings).await?)
}

/// Tauri command: Validate a workflow.
#[tauri::command]
fn validate_workflow(
    backend: tauri::State<AppBackend>,
    workflow: Workflow,
) -> Result<WorkflowValidationSummary, CommandError> {
    Ok(backend.validate_workflow(&workflow)?)
}

/// Tauri command: Start a workflow authoring chat session.
#[tauri::command]
fn start_workflow_authoring(
    backend: tauri::State<AppBackend>,
    base_workflow: Option<Workflow>,
) -> Result<String, CommandError> {
    Ok(backend.start_workflow_authoring(base_workflow))
}

/// Tauri command: Send a message in a workflow authoring session.
#[tauri::command]
async fn workflow_authoring_turn(
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

async fn bridge_still_owns_run(backend: &AppBackend, bridge_run_id: &Option<String>) -> bool {
    match (bridge_run_id, backend.current_run_id().await) {
        (Some(expected), Some(current)) => expected == &current,
        (None, _) => true,
        (Some(_), None) => false,
    }
}

fn spawn_run_event_bridge(
    app: tauri::AppHandle,
    workflow_name: String,
    mut event_rx: UnboundedReceiver<ExecutionEvent>,
    bridge_run_id: Option<String>,
) {
    run_sleep_guard::start_for_app(&app);
    tauri::async_runtime::spawn(async move {
        let mut failed = false;
        while !failed {
            let Some(event) = event_rx.recv().await else {
                break;
            };
            let notification =
                run_notifications::notification_for_event(&event, workflow_name.as_str());
            let backend = app.state::<AppBackend>();
            if !bridge_still_owns_run(&backend, &bridge_run_id).await {
                break;
            }
            let mut run_state = match backend.apply_execution_event(event).await {
                Ok(state) => state,
                Err(_) => break,
            };
            if let Some(notification) = notification.as_ref() {
                run_notifications::show_run_notification(&app, notification);
            }
            let deadline = tokio::time::Instant::now() + RUN_STATE_COALESCE_WINDOW;
            while run_state.active {
                tokio::select! {
                    () = tokio::time::sleep_until(deadline) => break,
                    maybe_event = event_rx.recv() => match maybe_event {
                        Some(event) => {
                            let notification = run_notifications::notification_for_event(
                                &event,
                                workflow_name.as_str(),
                            );
                            let backend = app.state::<AppBackend>();
                            if !bridge_still_owns_run(&backend, &bridge_run_id).await {
                                failed = true;
                                break;
                            }
                            match backend.apply_execution_event(event).await {
                                Ok(state) => {
                                    run_state = state;
                                    if let Some(notification) = notification.as_ref() {
                                        run_notifications::show_run_notification(
                                            &app, notification,
                                        );
                                    }
                                }
                                Err(_) => {
                                    failed = true;
                                    break;
                                }
                            }
                        },
                        None => break,
                    },
                }
            }
            let backend = app.state::<AppBackend>();
            if !bridge_still_owns_run(&backend, &bridge_run_id).await {
                break;
            }
            run_state = backend.get_run_state().await.unwrap_or(run_state);
            let active = run_state.active;
            let _ = app.emit("run-state", run_state);
            if !active {
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

/// Tauri command: Continue a stopped workflow run from the in-session checkpoint.
#[tauri::command]
async fn continue_run(
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

/// Tauri command: Whether a stopped run can be resumed in this session.
#[tauri::command]
async fn is_run_continuable(backend: tauri::State<'_, AppBackend>) -> Result<bool, CommandError> {
    Ok(backend.is_run_continuable().await)
}

#[tauri::command]
fn list_runs(
    backend: tauri::State<'_, AppBackend>,
    workflow_id: Option<String>,
) -> Result<Vec<orchestration::run::persistence::RunSummary>, CommandError> {
    Ok(backend.list_runs(workflow_id.as_deref())?)
}

#[tauri::command]
fn replay_run(
    backend: tauri::State<'_, AppBackend>,
    run_id: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.replay_run(&run_id)?)
}

#[tauri::command]
async fn resume_durable_run(
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

/// Tauri command: Return `git diff` for the whole repo at `cwd`.
#[tauri::command]
async fn git_diff_repo(cwd: String) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        orchestration::git::diff_repo(std::path::Path::new(&cwd))
    })
    .await
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))?
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))
}

/// Tauri command: Return whether `cwd` is inside a git work tree.
#[tauri::command]
async fn git_is_repo(cwd: String) -> Result<bool, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        orchestration::git::is_repo(std::path::Path::new(&cwd))
    })
    .await
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))
}

/// Tauri command: Return the current branch name for the repo at `cwd`.
#[tauri::command]
async fn git_current_branch(cwd: String) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        orchestration::git::current_branch(std::path::Path::new(&cwd))
    })
    .await
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))?
    .map_err(|error| CommandError::Backend(BackendError::GitFailed(error.to_string())))
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

/// Tauri command: Copy a workflow into a project as an independent duplicate.
#[tauri::command]
fn copy_workflow_to_project(
    backend: tauri::State<AppBackend>,
    target_project_id: String,
    source_workflow_id: String,
) -> Result<orchestration::api::CopyWorkflowToProjectResult, CommandError> {
    Ok(backend.copy_workflow_to_project(&target_project_id, &source_workflow_id)?)
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

/// Tauri command: Permanently delete a workflow.
#[tauri::command]
fn delete_workflow(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
    workflow_id: String,
) -> Result<Vec<Project>, CommandError> {
    let projects = backend.delete_workflow(&workflow_id)?;
    emit_schedule_statuses(&app);
    Ok(projects)
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
    let builder = {
        let builder = tauri::Builder::default();
        #[cfg(feature = "e2e-testing")]
        let builder = builder.plugin(tauri_plugin_playwright::init());
        builder
    };

    builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            bootstrap_app,
            list_projects,
            list_project_file_references,
            read_project_file_references,
            save_projects,
            create_project_from_directory,
            assign_workflow_to_project,
            copy_workflow_to_project,
            unassign_workflow_from_project,
            delete_workflow,
            list_workflows,
            load_all_workflows,
            list_schedule_statuses,
            refresh_schedules,
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
            probe_mcp_server,
            load_provider_api_key,
            save_provider_api_key,
            delete_provider_api_key,
            resolve_provider_readiness,
            refresh_bedrock_models,
            validate_workflow,
            start_workflow_authoring,
            workflow_authoring_turn,
            create_agent_node,
            start_run,
            continue_run,
            is_run_continuable,
            list_runs,
            replay_run,
            resume_durable_run,
            preview_file_edit,
            git_diff_file,
            git_diff_repo,
            git_is_repo,
            git_current_branch,
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
            spawn_schedule_loop(app.handle().clone());
            #[cfg(debug_assertions)]
            app.get_webview_window("main").unwrap().open_devtools();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

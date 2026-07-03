use crate::ipc_types::CommandError;
use crate::schedule_events::emit_schedule_statuses;
use orchestration::api::ScheduleDraft;
use orchestration::backend::{
    AppBackend, ScheduleStatus, WorkflowListItem, WorkflowValidationSummary,
};
use orchestration::{Project, Workflow, WorkflowSchedule};

#[tauri::command]
pub fn list_workflows(
    backend: tauri::State<AppBackend>,
) -> Result<Vec<WorkflowListItem>, CommandError> {
    Ok(backend.list_workflows()?)
}

#[tauri::command]
pub fn load_all_workflows(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
) -> Result<Vec<Workflow>, CommandError> {
    let workflows = backend.load_all_workflows()?;
    emit_schedule_statuses(&app);
    Ok(workflows)
}

#[tauri::command]
pub fn list_schedule_statuses(
    backend: tauri::State<AppBackend>,
) -> Result<Vec<ScheduleStatus>, CommandError> {
    Ok(backend.list_schedule_statuses())
}

#[tauri::command]
pub fn refresh_schedules(
    backend: tauri::State<AppBackend>,
) -> Result<Vec<ScheduleStatus>, CommandError> {
    Ok(backend.refresh_schedules()?)
}

#[tauri::command]
pub fn build_schedule_from_draft(
    backend: tauri::State<AppBackend>,
    draft: ScheduleDraft,
) -> WorkflowSchedule {
    backend.build_schedule_from_draft(draft)
}

#[tauri::command]
pub fn schedule_draft_from_schedule(
    backend: tauri::State<AppBackend>,
    schedule: WorkflowSchedule,
) -> ScheduleDraft {
    backend.schedule_draft_from_schedule(&schedule)
}

#[tauri::command]
pub fn describe_workflow_schedule(
    backend: tauri::State<AppBackend>,
    schedule: WorkflowSchedule,
) -> String {
    backend.describe_schedule(&schedule)
}

#[tauri::command]
pub fn load_workflow(
    backend: tauri::State<AppBackend>,
    workflow_id: String,
) -> Result<Workflow, CommandError> {
    Ok(backend.load_workflow(&workflow_id)?)
}

#[tauri::command]
pub fn create_workflow(
    backend: tauri::State<AppBackend>,
    name: String,
) -> Result<Workflow, CommandError> {
    Ok(backend.create_workflow(name)?)
}

#[tauri::command]
pub fn save_workflow(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
    workflow: Workflow,
) -> Result<Workflow, CommandError> {
    let saved = backend.save_workflow(workflow)?;
    emit_schedule_statuses(&app);
    Ok(saved)
}

#[tauri::command]
pub fn save_workflows(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
    workflows: Vec<Workflow>,
) -> Result<(), CommandError> {
    backend.save_workflows(&workflows)?;
    emit_schedule_statuses(&app);
    Ok(())
}

#[tauri::command]
pub fn rename_workflow(
    backend: tauri::State<AppBackend>,
    workflow_id: String,
    name: String,
) -> Result<WorkflowListItem, CommandError> {
    Ok(backend.rename_workflow(&workflow_id, name)?)
}

#[tauri::command]
pub fn validate_workflow(
    backend: tauri::State<AppBackend>,
    workflow: Workflow,
) -> Result<WorkflowValidationSummary, CommandError> {
    Ok(backend.validate_workflow(&workflow)?)
}

#[tauri::command]
pub fn create_agent_node(
    backend: tauri::State<AppBackend>,
    index: usize,
    x: f32,
    y: f32,
    agent_id: Option<String>,
) -> Result<orchestration::Node, CommandError> {
    Ok(backend.create_agent_node(index, x, y, agent_id.as_deref())?)
}

#[tauri::command]
pub fn delete_workflow(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
    workflow_id: String,
) -> Result<Vec<Project>, CommandError> {
    let projects = backend.delete_workflow(&workflow_id)?;
    emit_schedule_statuses(&app);
    Ok(projects)
}

use crate::ipc_types::{BootstrapPayload, CommandError};
use orchestration::backend::AppBackend;

/// Tauri command: Initialize the application.
#[tauri::command]
pub async fn bootstrap_app(
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

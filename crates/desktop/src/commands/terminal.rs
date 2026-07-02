use crate::ipc_types::CommandError;
use crate::terminal_events::spawn_terminal_event_bridge;
use orchestration::backend::AppBackend;
use orchestration::terminal::TerminalStart;

#[tauri::command]
pub async fn start_terminal(
    app: tauri::AppHandle,
    backend: tauri::State<'_, AppBackend>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<TerminalStart, CommandError> {
    let (session, events) = backend.start_terminal(cwd.as_deref(), cols, rows)?;
    spawn_terminal_event_bridge(app, events);
    Ok(session)
}

#[tauri::command]
pub fn write_terminal(
    backend: tauri::State<AppBackend>,
    session_id: String,
    data: String,
) -> Result<(), CommandError> {
    Ok(backend.write_terminal(&session_id, &data)?)
}

#[tauri::command]
pub fn resize_terminal(
    backend: tauri::State<AppBackend>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), CommandError> {
    Ok(backend.resize_terminal(&session_id, cols, rows)?)
}

#[tauri::command]
pub fn stop_terminal(
    backend: tauri::State<AppBackend>,
    session_id: String,
) -> Result<(), CommandError> {
    Ok(backend.stop_terminal(&session_id)?)
}

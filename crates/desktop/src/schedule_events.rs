use orchestration::backend::AppBackend;
use tauri::{Emitter, Manager};

use crate::run_event_bridge::spawn_run_event_bridge;

const SCHEDULE_EVENT: &str = "schedule-event";
const SCHEDULE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) fn emit_schedule_statuses(app: &tauri::AppHandle) {
    let backend = app.state::<AppBackend>();
    backend.tick_schedules();
    let _ = app.emit(SCHEDULE_EVENT, backend.list_schedule_statuses());
}

pub(crate) fn spawn_schedule_loop(app: tauri::AppHandle) {
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

use orchestration::backend::AppBackend;
use orchestration::run::execution::ExecutionEvent;
use tauri::{Emitter, Manager};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{run_notifications, run_sleep_guard};

const RUN_STATE_COALESCE_WINDOW: std::time::Duration = std::time::Duration::from_millis(30);

async fn bridge_still_owns_run(backend: &AppBackend, bridge_run_id: &Option<String>) -> bool {
    match (bridge_run_id, backend.current_run_id().await) {
        (Some(expected), Some(current)) => expected == &current,
        (None, _) => true,
        (Some(_), None) => false,
    }
}

pub(crate) fn spawn_run_event_bridge(
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

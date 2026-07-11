use orchestration::backend::AppBackend;
use tauri::Manager;

use crate::{run_sleep_guard, schedule_events};

pub(crate) fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    crate::search_sidecar::publish_bundled_search_path();
    let runtime_handle = tauri::async_runtime::handle().inner().clone();
    app.manage(AppBackend::with_runtime_handle(runtime_handle));
    app.manage(run_sleep_guard::RunSleepGuard::new());
    schedule_events::spawn_schedule_loop(app.handle().clone());
    #[cfg(debug_assertions)]
    app.get_webview_window("main").unwrap().open_devtools();
    Ok(())
}

pub(crate) fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
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
}

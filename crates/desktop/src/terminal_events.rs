use orchestration::terminal::TerminalEvent;
use tauri::Emitter;
use tokio::sync::mpsc::UnboundedReceiver;

const TERMINAL_EVENT: &str = "terminal-event";

pub(crate) fn spawn_terminal_event_bridge(
    app: tauri::AppHandle,
    mut events: UnboundedReceiver<TerminalEvent>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = events.recv().await {
            let _ = app.emit(TERMINAL_EVENT, event);
        }
    });
}

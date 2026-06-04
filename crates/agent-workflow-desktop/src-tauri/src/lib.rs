// TODO: Restore these imports once agent_store and backend modules are complete
// use agent_workflow_app::agent_store::AgentDefinition;
// use agent_workflow_app::backend::{
//     Backend, BackendCommand, BackendEvent, ProviderConfigError, RunWorkflowCommand,
//     SaveWorkflowCommand,
// };
// use agent_workflow_app::state::WorkflowRunState;
use tauri::{generate_context, Manager};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub fn run() {
    let builder = tauri::Builder::default();

    builder
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
            Ok(())
        })
        .run(generate_context!())
        .expect("error while running tauri application");
}

#![allow(
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    reason = "Tauri desktop shell; strict pedantic/nursery lint not enforced on thin IPC glue"
)]

mod app_lifecycle;
mod commands;
mod ipc_types;
mod run_event_bridge;
mod run_notifications;
mod run_sleep_guard;
mod schedule_events;
mod search_sidecar;
mod terminal_events;

use crate::app_lifecycle::{handle_window_event, setup_app};

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
            commands::bootstrap::bootstrap_app,
            commands::project::list_projects,
            commands::project::list_project_file_references,
            commands::project::save_projects,
            commands::project::create_project_from_directory,
            commands::project::assign_workflow_to_project,
            commands::project::copy_workflow_to_project,
            commands::project::unassign_workflow_from_project,
            commands::workflow::delete_workflow,
            commands::workflow::list_workflows,
            commands::workflow::load_all_workflows,
            commands::workflow::list_schedule_statuses,
            commands::workflow::refresh_schedules,
            commands::workflow::build_schedule_from_draft,
            commands::workflow::schedule_draft_from_schedule,
            commands::workflow::describe_workflow_schedule,
            commands::workflow::load_workflow,
            commands::workflow::create_workflow,
            commands::workflow::save_workflow,
            commands::workflow::save_workflows,
            commands::workflow::rename_workflow,
            commands::agent::list_agents,
            commands::agent::list_skills,
            commands::agent::load_agents,
            commands::agent::create_agent_definition,
            commands::agent::save_agents,
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::settings::debug_log_path,
            commands::settings::append_debug_log,
            commands::settings::probe_mcp_server,
            commands::settings::load_provider_api_key,
            commands::settings::save_provider_api_key,
            commands::settings::delete_provider_api_key,
            commands::settings::load_search_api_key,
            commands::settings::save_search_api_key,
            commands::settings::delete_search_api_key,
            commands::settings::resolve_provider_readiness,
            commands::settings::refresh_bedrock_models,
            commands::settings::verify_bedrock_credentials,
            commands::workflow::validate_workflow,
            commands::authoring::start_workflow_authoring,
            commands::authoring::end_workflow_authoring,
            commands::authoring::workflow_authoring_turn,
            commands::workflow::create_agent_node,
            commands::run::start_run,
            commands::run::continue_run,
            commands::run::is_run_continuable,
            commands::run::list_runs,
            commands::run::replay_run,
            commands::run::resume_durable_run,
            commands::run::preview_file_edit,
            commands::run::git_diff_file,
            commands::git::git_diff_repo,
            commands::git::git_is_repo,
            commands::git::git_current_branch,
            commands::run::revert_edit_batch,
            commands::run::stop_run,
            commands::run::interrupt_node,
            commands::run::retry_node,
            commands::run::update_node_runtime_config,
            commands::run::submit_user_input,
            commands::run::submit_tool_approval,
            commands::run::get_run_state,
            commands::run::clear_run_trace,
            commands::terminal::start_terminal,
            commands::terminal::write_terminal,
            commands::terminal::resize_terminal,
            commands::terminal::stop_terminal,
        ])
        .on_window_event(handle_window_event)
        .setup(setup_app)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

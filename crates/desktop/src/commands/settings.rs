use crate::ipc_types::CommandError;
use orchestration::api::{DebugLogEntry, DebugLogWrite, SettingsLoadPayload};
use orchestration::backend::{AppBackend, ProviderReadiness};
use orchestration::{AppSettings, McpServerConfig};

#[tauri::command]
pub fn load_settings(
    backend: tauri::State<AppBackend>,
    project_path: Option<String>,
) -> Result<SettingsLoadPayload, CommandError> {
    Ok(backend.load_settings(project_path.as_deref())?)
}

#[tauri::command]
pub fn save_settings(
    backend: tauri::State<AppBackend>,
    settings: AppSettings,
) -> Result<(), CommandError> {
    Ok(backend.save_settings(&settings)?)
}

#[tauri::command]
pub fn debug_log_path(backend: tauri::State<AppBackend>) -> String {
    backend.debug_log_path()
}

#[tauri::command]
pub fn append_debug_log(
    backend: tauri::State<AppBackend>,
    settings: AppSettings,
    entry: DebugLogEntry,
) -> Result<DebugLogWrite, CommandError> {
    Ok(backend.append_debug_log(&settings, &entry)?)
}

#[tauri::command]
pub async fn probe_mcp_server(
    backend: tauri::State<'_, AppBackend>,
    config: McpServerConfig,
) -> Result<Vec<String>, CommandError> {
    Ok(backend.probe_mcp_server(config).await?)
}

#[tauri::command]
pub fn load_provider_api_key(
    backend: tauri::State<AppBackend>,
    provider_id: String,
) -> Result<Option<String>, CommandError> {
    Ok(backend.load_provider_api_key(&provider_id)?)
}

#[tauri::command]
pub fn save_provider_api_key(
    backend: tauri::State<AppBackend>,
    provider_id: String,
    api_key: String,
) -> Result<(), CommandError> {
    Ok(backend.save_provider_api_key(&provider_id, &api_key)?)
}

#[tauri::command]
pub fn delete_provider_api_key(
    backend: tauri::State<AppBackend>,
    provider_id: String,
) -> Result<(), CommandError> {
    Ok(backend.delete_provider_api_key(&provider_id)?)
}

#[tauri::command]
pub fn resolve_provider_readiness(
    backend: tauri::State<AppBackend>,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> ProviderReadiness {
    backend.resolve_provider_readiness(&settings, transient_api_key.as_deref())
}

#[tauri::command]
pub async fn refresh_bedrock_models(
    backend: tauri::State<'_, AppBackend>,
    settings: AppSettings,
) -> Result<Vec<String>, CommandError> {
    Ok(backend.refresh_bedrock_models(&settings).await?)
}

#[tauri::command]
pub async fn verify_bedrock_credentials(
    backend: tauri::State<'_, AppBackend>,
    settings: AppSettings,
) -> Result<String, CommandError> {
    Ok(backend.verify_bedrock_credentials(&settings).await?)
}

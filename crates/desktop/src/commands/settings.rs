use crate::ipc_types::CommandError;
use orchestration::api::{DebugLogEntry, DebugLogWrite, SettingsLoadPayload};
use orchestration::backend::{AppBackend, ProviderReadiness};
use orchestration::{AppSettings, McpServerConfig};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn start_codex_login(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
) -> Result<orchestration::CodexLoginStatus, CommandError> {
    Ok(backend.start_codex_login(move |url| {
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|error| error.to_string())
    })?)
}

#[tauri::command]
pub fn codex_login_status(backend: tauri::State<AppBackend>) -> orchestration::CodexLoginStatus {
    backend.codex_login_status()
}

#[tauri::command]
pub fn cancel_codex_login(backend: tauri::State<AppBackend>) -> orchestration::CodexLoginStatus {
    backend.cancel_codex_login()
}

#[tauri::command]
pub fn disconnect_codex(
    backend: tauri::State<AppBackend>,
) -> Result<orchestration::CodexLoginStatus, CommandError> {
    Ok(backend.disconnect_codex()?)
}

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
pub fn import_mcp_config(
    backend: tauri::State<AppBackend>,
    content: String,
) -> Result<orchestration::api::McpConfigImport, CommandError> {
    Ok(backend.import_mcp_config(&content)?)
}

#[tauri::command]
pub fn apply_mcp_config(
    backend: tauri::State<AppBackend>,
    content: String,
) -> Result<orchestration::api::McpConfigImport, CommandError> {
    Ok(backend.apply_mcp_config(&content)?)
}

#[tauri::command]
pub fn export_mcp_config(backend: tauri::State<AppBackend>) -> Result<String, CommandError> {
    Ok(backend.export_mcp_config()?)
}

#[tauri::command]
pub async fn probe_mcp_server(
    backend: tauri::State<'_, AppBackend>,
    config: McpServerConfig,
    source_path: Option<String>,
) -> Result<orchestration::api::McpProbeResult, CommandError> {
    Ok(backend
        .probe_mcp_server(config, source_path.as_deref())
        .await?)
}

#[tauri::command]
pub async fn list_mcp_capabilities(
    backend: tauri::State<'_, AppBackend>,
    server_id: String,
) -> Result<orchestration::McpCapabilityCatalog, CommandError> {
    Ok(backend.list_mcp_capabilities(&server_id).await?)
}

#[tauri::command]
pub async fn preview_mcp_resource(
    backend: tauri::State<'_, AppBackend>,
    server_id: String,
    uri: String,
    max_bytes: u32,
) -> Result<orchestration::McpContextSnapshot, CommandError> {
    Ok(backend
        .preview_mcp_resource(&server_id, &uri, max_bytes)
        .await?)
}

#[tauri::command]
pub async fn preview_mcp_prompt(
    backend: tauri::State<'_, AppBackend>,
    server_id: String,
    name: String,
    arguments: std::collections::BTreeMap<String, String>,
    max_bytes: u32,
) -> Result<orchestration::McpContextSnapshot, CommandError> {
    Ok(backend
        .preview_mcp_prompt(&server_id, &name, arguments, max_bytes)
        .await?)
}

#[tauri::command]
pub async fn start_mcp_oauth(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    server_id: String,
    scopes: Vec<String>,
) -> Result<orchestration::McpOAuthStatus, CommandError> {
    let start = backend.start_mcp_oauth(&server_id, scopes).await?;
    if app
        .opener()
        .open_url(&start.authorization_url, None::<&str>)
        .is_err()
    {
        let _ = backend.disconnect_mcp_oauth(&server_id).await;
        return Err(
            orchestration::backend::BackendError::Io(std::io::Error::other(
                "MCP OAuth browser could not be opened",
            ))
            .into(),
        );
    }
    Ok(start.status)
}

#[tauri::command]
pub async fn mcp_oauth_status(
    backend: tauri::State<'_, AppBackend>,
    server_id: String,
) -> Result<orchestration::McpOAuthStatus, CommandError> {
    Ok(backend.mcp_oauth_status(&server_id).await?)
}

#[tauri::command]
pub async fn refresh_mcp_oauth(
    backend: tauri::State<'_, AppBackend>,
    server_id: String,
) -> Result<orchestration::McpOAuthStatus, CommandError> {
    Ok(backend.refresh_mcp_oauth(&server_id).await?)
}

#[tauri::command]
pub async fn disconnect_mcp_oauth(
    backend: tauri::State<'_, AppBackend>,
    server_id: String,
) -> Result<orchestration::McpOAuthStatus, CommandError> {
    Ok(backend.disconnect_mcp_oauth(&server_id).await?)
}

#[tauri::command]
pub fn save_mcp_secret(
    backend: tauri::State<AppBackend>,
    server_id: String,
    slot: String,
    value: String,
) -> Result<String, CommandError> {
    Ok(backend.save_mcp_secret(&server_id, &slot, &value)?)
}

#[tauri::command]
pub fn delete_mcp_secret(
    backend: tauri::State<AppBackend>,
    secret_ref: String,
) -> Result<(), CommandError> {
    Ok(backend.delete_mcp_secret(&secret_ref)?)
}

#[tauri::command]
pub async fn search_mcp_registry(
    backend: tauri::State<'_, AppBackend>,
    search: Option<String>,
    cursor: Option<String>,
) -> Result<orchestration::mcp::catalog::McpCatalogPage, CommandError> {
    Ok(backend.search_mcp_registry(search, cursor).await?)
}

#[tauri::command]
pub async fn list_mcp_registry_versions(
    backend: tauri::State<'_, AppBackend>,
    server_name: String,
) -> Result<orchestration::mcp::catalog::McpCatalogPage, CommandError> {
    Ok(backend.list_mcp_registry_versions(&server_name).await?)
}

#[tauri::command]
pub async fn preview_mcp_registry_install(
    backend: tauri::State<'_, AppBackend>,
    server_name: String,
    version: String,
    package_index: usize,
) -> Result<orchestration::api::McpInstallPreview, CommandError> {
    Ok(backend
        .preview_mcp_registry_install(&server_name, &version, package_index)
        .await?)
}

#[tauri::command]
pub async fn preview_mcp_registry_remote(
    backend: tauri::State<'_, AppBackend>,
    server_name: String,
    version: String,
    remote_index: usize,
) -> Result<orchestration::api::McpInstallPreview, CommandError> {
    Ok(backend
        .preview_mcp_registry_remote(&server_name, &version, remote_index)
        .await?)
}

#[tauri::command]
pub async fn install_mcp_package(
    backend: tauri::State<'_, AppBackend>,
    operation_id: String,
    server: McpServerConfig,
) -> Result<orchestration::api::McpInstallResult, CommandError> {
    Ok(backend.install_mcp_package(&operation_id, server).await?)
}

#[tauri::command]
pub fn cancel_mcp_install(
    backend: tauri::State<AppBackend>,
    operation_id: String,
) -> Result<bool, CommandError> {
    Ok(backend.cancel_mcp_install(&operation_id)?)
}

#[tauri::command]
pub fn rollback_mcp_install(
    backend: tauri::State<AppBackend>,
    server_id: String,
) -> Result<McpServerConfig, CommandError> {
    Ok(backend.rollback_mcp_install(&server_id)?)
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
pub fn load_search_api_key(
    backend: tauri::State<AppBackend>,
    provider: String,
) -> Result<Option<String>, CommandError> {
    Ok(backend.load_search_api_key(&provider)?)
}

#[tauri::command]
pub fn save_search_api_key(
    backend: tauri::State<AppBackend>,
    provider: String,
    api_key: String,
) -> Result<(), CommandError> {
    Ok(backend.save_search_api_key(&provider, &api_key)?)
}

#[tauri::command]
pub fn delete_search_api_key(
    backend: tauri::State<AppBackend>,
    provider: String,
) -> Result<(), CommandError> {
    Ok(backend.delete_search_api_key(&provider)?)
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
pub async fn refresh_provider_models(
    backend: tauri::State<'_, AppBackend>,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> Result<Vec<String>, CommandError> {
    Ok(backend
        .refresh_provider_models(&settings, transient_api_key.as_deref())
        .await?)
}

#[tauri::command]
pub async fn verify_bedrock_credentials(
    backend: tauri::State<'_, AppBackend>,
    settings: AppSettings,
) -> Result<String, CommandError> {
    Ok(backend.verify_bedrock_credentials(&settings).await?)
}

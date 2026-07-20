use std::io;

use crate::settings::model::AppSettings;

use super::{AppBackend, BackendError, ProviderReadiness};

impl AppBackend {
    pub fn start_codex_login<F>(
        &self,
        open_browser: F,
    ) -> Result<crate::CodexLoginStatus, BackendError>
    where
        F: Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    {
        self.settings.start_codex_login(open_browser)
    }

    #[must_use]
    pub fn codex_login_status(&self) -> crate::CodexLoginStatus {
        self.settings.codex_login_status()
    }

    #[must_use]
    pub fn cancel_codex_login(&self) -> crate::CodexLoginStatus {
        self.settings.cancel_codex_login()
    }

    pub fn disconnect_codex(&self) -> Result<crate::CodexLoginStatus, BackendError> {
        self.settings.disconnect_codex()
    }

    pub fn list_skills(&self) -> Result<Vec<crate::settings::ports::SkillSummary>, BackendError> {
        self.settings.list_skills()
    }

    pub fn load_settings(
        &self,
        project_path: Option<&str>,
    ) -> Result<crate::api::SettingsLoadPayload, BackendError> {
        let settings = self.settings.load()?;
        let root = project_path
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let discovered_mcp = crate::adapters::mcp::scan_external_mcp_for_api(&settings.mcp, &root);
        Ok(crate::api::SettingsLoadPayload {
            settings: settings.redacted(),
            discovered_mcp,
        })
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), BackendError> {
        self.settings
            .save(settings)
            .map_err(|error| self.persistence_err("persistence.settings_save", error))?;
        Ok(())
    }

    #[must_use]
    pub fn debug_log_path(&self) -> String {
        crate::diagnostics::debug_log_path().display().to_string()
    }

    pub fn append_debug_log(
        &self,
        settings: &AppSettings,
        entry: &crate::api::DebugLogEntry,
    ) -> Result<crate::api::DebugLogWrite, BackendError> {
        crate::diagnostics::append_debug_log(settings, entry)
    }

    pub async fn probe_mcp_server(
        &self,
        config: crate::settings::model::McpServerConfig,
    ) -> Result<Vec<String>, BackendError> {
        let client = crate::adapters::mcp::McpStdioClient::spawn(&config)
            .await
            .map_err(|error| io::Error::other(error.to_string()))?;
        let names = client
            .list_tool_names()
            .await
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(names)
    }

    pub fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError> {
        self.settings.load_provider_api_key(provider_id)
    }

    pub fn save_provider_api_key(
        &self,
        provider_id: &str,
        api_key: &str,
    ) -> Result<(), BackendError> {
        self.settings.save_provider_api_key(provider_id, api_key)
    }

    pub fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError> {
        self.settings.delete_provider_api_key(provider_id)
    }

    pub fn load_search_api_key(&self, provider: &str) -> Result<Option<String>, BackendError> {
        self.settings.load_search_api_key(provider)
    }

    pub fn save_search_api_key(&self, provider: &str, api_key: &str) -> Result<(), BackendError> {
        self.settings.save_search_api_key(provider, api_key)
    }

    pub fn delete_search_api_key(&self, provider: &str) -> Result<(), BackendError> {
        self.settings.delete_search_api_key(provider)
    }

    #[must_use]
    pub fn resolve_provider_readiness(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> ProviderReadiness {
        self.settings
            .resolve_provider_readiness(settings, transient_api_key)
    }

    pub async fn refresh_bedrock_models(
        &self,
        settings: &AppSettings,
    ) -> Result<Vec<String>, BackendError> {
        self.settings.refresh_bedrock_models(settings).await
    }

    pub async fn verify_bedrock_credentials(
        &self,
        settings: &AppSettings,
    ) -> Result<String, BackendError> {
        self.settings.verify_bedrock_credentials(settings).await
    }
}

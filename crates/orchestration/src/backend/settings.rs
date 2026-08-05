use std::collections::BTreeSet;
use std::io;
use std::time::Instant;

use crate::api::{McpProbeReport, McpProbeResult, McpProbeStage, McpProbeState};
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

    pub fn save_mcp_secret(
        &self,
        server_id: &str,
        slot: &str,
        value: &str,
    ) -> Result<String, BackendError> {
        self.settings.save_mcp_secret(server_id, slot, value)
    }

    pub fn delete_mcp_secret(&self, secret_ref: &str) -> Result<(), BackendError> {
        self.settings.delete_mcp_secret(secret_ref)
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

    pub fn import_mcp_config(
        &self,
        content: &str,
    ) -> Result<crate::api::McpConfigImport, BackendError> {
        let mut parsed = crate::adapters::mcp::import_mcp_servers_json(content)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
        let existing = self.settings.load()?;
        let existing_ids = existing
            .mcp
            .servers
            .iter()
            .map(|server| server.id.as_str())
            .collect::<BTreeSet<_>>();
        for server in &parsed.servers {
            if existing_ids.contains(server.id.as_str()) {
                parsed
                    .diagnostics
                    .push(crate::adapters::mcp::McpParseDiagnostic {
                        server_id: server.id.clone(),
                        message: "Apply replaces the existing server with this ID".to_string(),
                    });
            }
        }
        parsed.diagnostics.sort_by(|left, right| {
            (&left.server_id, &left.message).cmp(&(&right.server_id, &right.message))
        });
        Ok(mcp_config_import(parsed))
    }

    pub fn apply_mcp_config(
        &self,
        content: &str,
    ) -> Result<crate::api::McpConfigImport, BackendError> {
        let parsed = crate::adapters::mcp::import_mcp_servers_json(content)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
        let imported_ids = parsed
            .servers
            .iter()
            .map(|server| server.id.clone())
            .collect::<BTreeSet<_>>();
        let mut settings = self.settings.load()?;
        for candidate in parsed.servers {
            if let Some(index) = settings
                .mcp
                .servers
                .iter()
                .position(|server| server.id == candidate.id)
            {
                settings.mcp.servers[index] = candidate;
            } else {
                settings.mcp.servers.push(candidate);
            }
        }
        self.save_settings(&settings)?;
        let persisted = self.settings.load()?;
        Ok(crate::api::McpConfigImport {
            servers: persisted
                .mcp
                .servers
                .into_iter()
                .filter(|server| imported_ids.contains(&server.id))
                .map(|server| server.redacted())
                .collect(),
            diagnostics: parsed
                .diagnostics
                .into_iter()
                .map(|diagnostic| crate::api::McpImportDiagnostic {
                    server_id: diagnostic.server_id,
                    message: diagnostic.message,
                })
                .collect(),
        })
    }

    pub fn export_mcp_config(&self) -> Result<String, BackendError> {
        let settings = self.settings.load()?;
        crate::adapters::mcp::export_canonical_mcp_json(&settings.mcp.servers)
            .map_err(|error| io::Error::other(error).into())
    }
}

fn mcp_config_import(parsed: crate::adapters::mcp::McpParseResult) -> crate::api::McpConfigImport {
    crate::api::McpConfigImport {
        servers: parsed
            .servers
            .into_iter()
            .map(|server| server.redacted())
            .collect(),
        diagnostics: parsed
            .diagnostics
            .into_iter()
            .map(|diagnostic| crate::api::McpImportDiagnostic {
                server_id: diagnostic.server_id,
                message: diagnostic.message,
            })
            .collect(),
    }
}

impl AppBackend {
    pub async fn probe_mcp_server(
        &self,
        config: crate::settings::model::McpServerConfig,
        source_path: Option<&str>,
    ) -> Result<McpProbeResult, BackendError> {
        let started = Instant::now();
        let transport = config.connection.transport_kind();
        let hydrated = if let Some(source_path) = source_path.filter(|path| !path.trim().is_empty())
        {
            crate::adapters::mcp::hydrate_mcp_server_from_path(
                std::path::Path::new(source_path),
                config.clone(),
            )
            .map_err(BackendError::from)
        } else {
            self.settings.hydrate_mcp_server(config.clone())
        };
        let config = match hydrated {
            Ok(config) => config,
            Err(error) => {
                return Ok(failed_mcp_probe(
                    config,
                    started,
                    transport,
                    McpProbeStage::Preflight,
                    false,
                    error.to_string(),
                ));
            }
        };

        match crate::mcp::preflight::preflight(
            &config.connection,
            crate::mcp::environment::effective_path().await.as_deref(),
        ) {
            crate::mcp::preflight::McpPreflight::Ready { .. }
            | crate::mcp::preflight::McpPreflight::RemoteReady { .. } => {}
            crate::mcp::preflight::McpPreflight::Missing { command, .. } => {
                return Ok(failed_mcp_probe(
                    config,
                    started,
                    transport,
                    McpProbeStage::Preflight,
                    false,
                    format!("MCP command `{command}` was not found"),
                ));
            }
            crate::mcp::preflight::McpPreflight::UnsupportedTransport { transport } => {
                return Ok(failed_mcp_probe(
                    config,
                    started,
                    transport,
                    McpProbeStage::Preflight,
                    false,
                    format!("MCP transport `{transport:?}` is not available yet"),
                ));
            }
            crate::mcp::preflight::McpPreflight::InvalidRemote { reason } => {
                return Ok(failed_mcp_probe(
                    config,
                    started,
                    transport,
                    McpProbeStage::Preflight,
                    false,
                    reason,
                ));
            }
        }

        let client = match crate::adapters::mcp::McpStdioClient::spawn(&config).await {
            Ok(client) => client,
            Err(error) => {
                return Ok(failed_mcp_probe(
                    config,
                    started,
                    transport,
                    McpProbeStage::Connect,
                    mcp_probe_auth_required(&error),
                    error.to_string(),
                ));
            }
        };
        let metadata = client.server_metadata().await;
        let names = match client.list_tool_names().await {
            Ok(names) => names,
            Err(error) => {
                let _ = client.close().await;
                return Ok(failed_mcp_probe(
                    config,
                    started,
                    transport,
                    McpProbeStage::ListTools,
                    false,
                    error.to_string(),
                ));
            }
        };
        if let Err(error) = client.close().await {
            return Ok(failed_mcp_probe(
                config,
                started,
                transport,
                McpProbeStage::Close,
                false,
                error.to_string(),
            ));
        }

        let mut approved = config;
        approved.enabled = false;
        crate::mcp::trust::approve_current(&mut approved, chrono::Utc::now())
            .map_err(|error| io::Error::other(error.to_string()))?;
        let (protocol_version, server_name, server_version, capabilities) = metadata.map_or_else(
            || (None, None, None, vec!["tools".to_string()]),
            |metadata| {
                (
                    Some(metadata.protocol_version),
                    Some(metadata.server_name),
                    Some(metadata.server_version),
                    metadata.capabilities,
                )
            },
        );
        Ok(McpProbeResult {
            server: approved.redacted(),
            report: McpProbeReport {
                state: McpProbeState::Ready,
                stage: McpProbeStage::Close,
                auth_required: false,
                duration_ms: elapsed_ms(started),
                transport,
                protocol_version,
                server_name,
                server_version,
                capabilities,
                tool_names: names,
                error: None,
            },
        })
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

    pub async fn refresh_provider_models(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<Vec<String>, BackendError> {
        self.settings
            .refresh_provider_models(settings, transient_api_key)
            .await
    }

    pub async fn verify_bedrock_credentials(
        &self,
        settings: &AppSettings,
    ) -> Result<String, BackendError> {
        self.settings.verify_bedrock_credentials(settings).await
    }
}

fn failed_mcp_probe(
    mut server: crate::settings::model::McpServerConfig,
    started: Instant,
    transport: crate::mcp::model::McpTransportKind,
    stage: McpProbeStage,
    auth_required: bool,
    error: String,
) -> McpProbeResult {
    server.enabled = false;
    server.trust = Default::default();
    McpProbeResult {
        server: server.redacted(),
        report: McpProbeReport {
            state: McpProbeState::Failed,
            stage,
            auth_required,
            duration_ms: elapsed_ms(started),
            transport,
            protocol_version: None,
            server_name: None,
            server_version: None,
            capabilities: Vec::new(),
            tool_names: Vec::new(),
            error: Some(error),
        },
    }
}

fn mcp_probe_auth_required(error: &crate::adapters::mcp::McpError) -> bool {
    matches!(error, crate::adapters::mcp::McpError::OAuthRequired { .. })
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::mcp_probe_auth_required;

    #[test]
    fn oauth_probe_flag_uses_typed_error() {
        assert!(mcp_probe_auth_required(
            &crate::adapters::mcp::McpError::OAuthRequired {
                server_id: "massive".to_string(),
            }
        ));
        assert!(!mcp_probe_auth_required(
            &crate::adapters::mcp::McpError::RemoteTransport {
                server_id: "massive".to_string(),
                operation: "Streamable HTTP initialization",
            }
        ));
    }
}

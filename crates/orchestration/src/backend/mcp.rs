use super::{AppBackend, BackendError};
use crate::adapters::mcp::{
    McpClient, McpError, McpRegistryClient, PackageInstallStatus, PackageInstaller,
    MCP_REGISTRY_PREVIEW_LABEL,
};
use crate::api::{McpInstallPreview, McpInstallResult, McpInstallResultState};
use crate::mcp::catalog::{McpCatalog, McpCatalogPackage, McpCatalogPage, McpCatalogQuery};
use crate::mcp::installer::{
    installed_connection, package_install_plan, record_install_success, rollback_install,
};
use crate::mcp::model::{
    ExactPackageVersion, McpAuth, McpConnection, McpInstall, McpServerRecord, McpServerSource,
    PersistedValue,
};
use crate::mcp::oauth::{McpOAuthRequest, McpOAuthStart, McpOAuthStatus};
use crate::mcp::ports::mcp_secret_ref;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::hash_map::Entry;
use std::collections::BTreeMap;
use std::io;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

impl AppBackend {
    pub async fn list_mcp_capabilities(
        &self,
        server_id: &str,
    ) -> Result<crate::mcp::capabilities::McpCapabilityCatalog, BackendError> {
        let server = self.trusted_mcp_capability_server(server_id)?;
        let client = McpClient::spawn(&server).await.map_err(mcp_io_error)?;
        let result = client.capability_catalog().await;
        finish_mcp_capability_request(client, result).await
    }

    pub async fn preview_mcp_resource(
        &self,
        server_id: &str,
        uri: &str,
        max_bytes: u32,
    ) -> Result<engine::McpContextSnapshot, BackendError> {
        validate_context_preview(server_id, uri, max_bytes)?;
        let server = self.trusted_mcp_capability_server(server_id)?;
        let client = McpClient::spawn(&server).await.map_err(mcp_io_error)?;
        let result = client.read_resource_snapshot(uri, max_bytes).await;
        finish_mcp_capability_request(client, result).await
    }

    pub async fn preview_mcp_prompt(
        &self,
        server_id: &str,
        name: &str,
        arguments: BTreeMap<String, String>,
        max_bytes: u32,
    ) -> Result<engine::McpContextSnapshot, BackendError> {
        validate_context_preview(server_id, name, max_bytes)?;
        if arguments.len() > 64
            || arguments.iter().any(|(key, value)| {
                key.trim().is_empty()
                    || key.len() > 256
                    || key.chars().any(char::is_control)
                    || value.len() > 16_384
            })
        {
            return Err(mcp_invalid_input("MCP prompt arguments are invalid"));
        }
        let server = self.trusted_mcp_capability_server(server_id)?;
        let client = McpClient::spawn(&server).await.map_err(mcp_io_error)?;
        let result = client
            .get_prompt_snapshot(name, &arguments, max_bytes)
            .await;
        finish_mcp_capability_request(client, result).await
    }

    fn trusted_mcp_capability_server(
        &self,
        server_id: &str,
    ) -> Result<crate::settings::model::McpServerConfig, BackendError> {
        let settings = self.settings.load()?;
        let server = settings
            .mcp
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .ok_or_else(|| mcp_invalid_input("MCP capability server was not found"))?;
        if !crate::mcp::trust::is_trusted(server) {
            return Err(mcp_invalid_input(
                "MCP capability server must pass Approve & Test first",
            ));
        }
        self.settings.hydrate_mcp_server(server.clone())
    }

    pub async fn start_mcp_oauth(
        &self,
        server_id: &str,
        requested_scopes: Vec<String>,
    ) -> Result<McpOAuthStart, BackendError> {
        validate_oauth_scopes(&requested_scopes)?;
        let settings = self.settings.load()?;
        let server = settings
            .mcp
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .ok_or_else(|| mcp_invalid_input("MCP OAuth server was not found"))?;
        let OAuthConnection {
            resource_url,
            allow_localhost,
            client_id,
            configured_scopes,
            expected_issuer,
        } = oauth_connection(server)?;
        let scopes = if requested_scopes.is_empty() {
            configured_scopes
        } else {
            requested_scopes
        };
        let start = self
            .mcp_oauth
            .start(McpOAuthRequest {
                server_id: server_id.to_string(),
                resource_url,
                allow_localhost,
                client_id,
                requested_scopes: scopes,
                expected_issuer,
            })
            .await
            .map_err(mcp_io_error)?;

        let mut settings = self.settings.load()?;
        let server = settings
            .mcp
            .servers
            .iter_mut()
            .find(|server| server.id == server_id)
            .ok_or_else(|| {
                mcp_invalid_input("MCP OAuth server was removed during authorization")
            })?;
        let auth = remote_auth_mut(&mut server.connection)?;
        *auth = McpAuth::OAuth {
            client_id: start.public_config.client_id.clone(),
            scopes: start.public_config.scopes.clone(),
            issuer: Some(start.public_config.issuer.clone()),
            credential_ref: Some(start.public_config.credential_ref.clone()),
        };
        server.trust = Default::default();
        server.enabled = false;
        if let Err(error) = self.settings.save(&settings) {
            let _ = self
                .mcp_oauth
                .disconnect(server_id, Some(&start.public_config.credential_ref))
                .await;
            return Err(error);
        }
        Ok(start)
    }

    pub async fn mcp_oauth_status(&self, server_id: &str) -> Result<McpOAuthStatus, BackendError> {
        let settings = self.settings.load()?;
        let server = settings
            .mcp
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .ok_or_else(|| mcp_invalid_input("MCP OAuth server was not found"))?;
        let credential_ref = oauth_credential_ref(server)?;
        self.mcp_oauth
            .status(server_id, credential_ref.as_deref())
            .await
            .map_err(mcp_io_error)
    }

    pub async fn refresh_mcp_oauth(&self, server_id: &str) -> Result<McpOAuthStatus, BackendError> {
        let settings = self.settings.load()?;
        let server = settings
            .mcp
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .ok_or_else(|| mcp_invalid_input("MCP OAuth server was not found"))?;
        let credential_ref = oauth_credential_ref(server)?
            .ok_or_else(|| mcp_invalid_input("MCP OAuth is not connected"))?;
        self.mcp_oauth
            .refresh(server_id, &credential_ref)
            .await
            .map_err(mcp_io_error)
    }

    pub async fn disconnect_mcp_oauth(
        &self,
        server_id: &str,
    ) -> Result<McpOAuthStatus, BackendError> {
        let settings = self.settings.load()?;
        let server = settings
            .mcp
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .ok_or_else(|| mcp_invalid_input("MCP OAuth server was not found"))?;
        let credential_ref = oauth_credential_ref(server)?;
        let status = self
            .mcp_oauth
            .disconnect(server_id, credential_ref.as_deref())
            .await
            .map_err(mcp_io_error)?;
        let mut settings = self.settings.load()?;
        let server = settings
            .mcp
            .servers
            .iter_mut()
            .find(|server| server.id == server_id)
            .ok_or_else(|| mcp_invalid_input("MCP OAuth server was removed during disconnect"))?;
        if let McpAuth::OAuth { credential_ref, .. } = remote_auth_mut(&mut server.connection)? {
            *credential_ref = None;
        }
        server.trust = Default::default();
        server.enabled = false;
        self.settings.save(&settings)?;
        Ok(status)
    }

    pub async fn search_mcp_registry(
        &self,
        search: Option<String>,
        cursor: Option<String>,
    ) -> Result<McpCatalogPage, BackendError> {
        let catalog = self.mcp_catalog()?;
        catalog
            .search(&McpCatalogQuery {
                search: search.filter(|value| !value.trim().is_empty()),
                cursor,
                limit: Some(25),
            })
            .await
            .map_err(mcp_io_error)
    }

    pub async fn list_mcp_registry_versions(
        &self,
        server_name: &str,
    ) -> Result<McpCatalogPage, BackendError> {
        self.mcp_catalog()?
            .versions(server_name)
            .await
            .map_err(mcp_io_error)
    }

    pub async fn preview_mcp_registry_install(
        &self,
        server_name: &str,
        version: &str,
        package_index: usize,
    ) -> Result<McpInstallPreview, BackendError> {
        let catalog = self.mcp_catalog()?;
        let server = catalog
            .exact_version(server_name, version)
            .await
            .map_err(mcp_io_error)?;
        let package = server.packages.get(package_index).ok_or_else(|| {
            BackendError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MCP Registry package selection is invalid",
            ))
        })?;
        let mut warnings = vec![
            "Registry metadata describes provenance; it does not establish package safety."
                .to_string(),
        ];
        let server_id = registry_server_id(&server.name);
        let install = catalog_install(package)?;
        if package.transport_type != "stdio" {
            return Err(BackendError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                "selected Registry package does not use stdio",
            )));
        }
        let args = catalog_arguments(package, &mut warnings);
        let environment = catalog_environment(&server_id, package, &mut warnings)?;
        let command = package
            .runtime_hint
            .clone()
            .unwrap_or_else(|| package.registry_type.clone());
        let record = McpServerRecord::new(
            &server_id,
            server.title.clone().unwrap_or_else(|| server.name.clone()),
            McpServerSource::Registry {
                catalog_base_url: catalog.base_url().to_string(),
                server_name: server.name,
                version: server.version,
            },
            install,
            McpConnection::Stdio {
                command,
                args,
                environment,
            },
        );
        let plan = package_install_plan(&server_id, &record.install, &self.mcp_install_root)
            .map_err(mcp_io_error)?;
        Ok(McpInstallPreview {
            server: record.redacted(),
            display_command: plan.display_command,
            catalog_label: MCP_REGISTRY_PREVIEW_LABEL.to_string(),
            warnings,
            requires_install: true,
        })
    }

    pub async fn preview_mcp_registry_remote(
        &self,
        server_name: &str,
        version: &str,
        remote_index: usize,
    ) -> Result<McpInstallPreview, BackendError> {
        let catalog = self.mcp_catalog()?;
        let server = catalog
            .exact_version(server_name, version)
            .await
            .map_err(mcp_io_error)?;
        let remote = server.remotes.get(remote_index).ok_or_else(|| {
            BackendError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MCP Registry remote selection is invalid",
            ))
        })?;
        let url = remote.url.clone().ok_or_else(|| {
            BackendError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MCP Registry remote does not declare a URL",
            ))
        })?;
        crate::adapters::mcp::http_security::validate_endpoint_url(&url, false)
            .map_err(mcp_io_error)?;
        let server_id = registry_server_id(&server.name);
        let headers = catalog_headers(&server_id, &remote.inputs)?;
        let connection = match remote.transport_type.as_str() {
            "streamable-http" | "streamableHttp" | "http" => McpConnection::StreamableHttp {
                url: url.clone(),
                allow_localhost: false,
                headers,
                auth: crate::mcp::model::McpAuth::None,
            },
            "sse" => McpConnection::LegacySse {
                url: url.clone(),
                allow_localhost: false,
                headers,
                auth: crate::mcp::model::McpAuth::None,
            },
            _ => {
                return Err(BackendError::Io(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "MCP Registry remote transport is not supported",
                )));
            }
        };
        let record = McpServerRecord::new(
            server_id,
            server.title.unwrap_or_else(|| server.name.clone()),
            McpServerSource::Registry {
                catalog_base_url: catalog.base_url().to_string(),
                server_name: server.name,
                version: server.version,
            },
            McpInstall::External,
            connection,
        );
        Ok(McpInstallPreview {
            server: record.redacted(),
            display_command: format!("Connect to {url}"),
            catalog_label: MCP_REGISTRY_PREVIEW_LABEL.to_string(),
            warnings: vec![
                "Registry metadata describes provenance; it does not establish remote server safety."
                    .to_string(),
                "Remote config stays disabled until required inputs are stored and Approve & Test succeeds."
                    .to_string(),
            ],
            requires_install: false,
        })
    }

    pub async fn install_mcp_package(
        &self,
        operation_id: &str,
        mut server: McpServerRecord,
    ) -> Result<McpInstallResult, BackendError> {
        validate_operation_id(operation_id)?;
        if !matches!(server.source, McpServerSource::Registry { .. }) {
            return Err(BackendError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "package installs require Registry provenance",
            )));
        }
        let plan = package_install_plan(&server.id, &server.install, &self.mcp_install_root)
            .map_err(mcp_io_error)?;
        let (package_args, environment) = match &server.connection {
            McpConnection::Stdio {
                args, environment, ..
            } => (args.clone(), environment.clone()),
            _ => {
                return Err(BackendError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "package installs require a stdio connection",
                )));
            }
        };
        let token = CancellationToken::new();
        {
            let mut operations = self.mcp_install_operations.lock().map_err(lock_error)?;
            match operations.entry(operation_id.to_string()) {
                Entry::Vacant(entry) => {
                    entry.insert(token.clone());
                }
                Entry::Occupied(_) => {
                    return Err(BackendError::Io(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "MCP install operation already exists",
                    )));
                }
            }
        }
        let outcome = PackageInstaller.install(&plan, &token).await;
        self.mcp_install_operations
            .lock()
            .map_err(lock_error)?
            .remove(operation_id);
        let outcome = outcome.map_err(mcp_io_error)?;
        let mut persisted_server = None;
        if outcome.status == PackageInstallStatus::Succeeded {
            let server_id = server.id.clone();
            let mut settings = self.settings.load()?;
            if let Some(existing) = settings
                .mcp
                .servers
                .iter()
                .find(|existing| existing.id == server.id)
            {
                server.install_history.clone_from(&existing.install_history);
            }
            server.connection =
                installed_connection(&server.install, &plan, package_args, environment)
                    .map_err(mcp_io_error)?;
            record_install_success(&mut server, plan.target_dir, Utc::now());
            if let Some(index) = settings
                .mcp
                .servers
                .iter()
                .position(|existing| existing.id == server.id)
            {
                settings.mcp.servers[index] = server;
            } else {
                settings.mcp.servers.push(server);
            }
            self.save_settings(&settings)?;
            persisted_server = self
                .settings
                .load()?
                .mcp
                .servers
                .into_iter()
                .find(|candidate| candidate.id == server_id)
                .map(|record| record.redacted());
        }
        Ok(McpInstallResult {
            operation_id: operation_id.to_string(),
            state: install_result_state(outcome.status),
            exit_code: outcome.exit_code,
            stdout_tail: outcome.stdout_tail,
            stderr_tail: outcome.stderr_tail,
            output_truncated: outcome.output_truncated,
            duration_ms: outcome.duration_ms,
            server: persisted_server,
        })
    }

    pub fn cancel_mcp_install(&self, operation_id: &str) -> Result<bool, BackendError> {
        let operations = self.mcp_install_operations.lock().map_err(lock_error)?;
        let Some(token) = operations.get(operation_id) else {
            return Ok(false);
        };
        token.cancel();
        Ok(true)
    }

    pub fn rollback_mcp_install(&self, server_id: &str) -> Result<McpServerRecord, BackendError> {
        let mut settings = self.settings.load()?;
        let server = settings
            .mcp
            .servers
            .iter_mut()
            .find(|server| server.id == server_id)
            .ok_or_else(|| {
                BackendError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    "installed MCP server was not found",
                ))
            })?;
        if !rollback_install(server) {
            return Err(BackendError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "installed MCP server has no prior revision",
            )));
        }
        let redacted = server.redacted();
        self.save_settings(&settings)?;
        Ok(redacted)
    }

    fn mcp_catalog(&self) -> Result<McpRegistryClient, BackendError> {
        let settings = self.settings.load()?;
        McpRegistryClient::new(&settings.mcp.registry_base_url, Duration::from_secs(15))
            .map_err(mcp_io_error)
    }
}

async fn finish_mcp_capability_request<T>(
    client: McpClient,
    result: Result<T, McpError>,
) -> Result<T, BackendError> {
    let close = client.close().await;
    match (result, close) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(mcp_io_error(error)),
        (Ok(_), Err(error)) => Err(mcp_io_error(error)),
    }
}

fn validate_context_preview(
    server_id: &str,
    source: &str,
    max_bytes: u32,
) -> Result<(), BackendError> {
    if server_id.trim().is_empty()
        || server_id.len() > 128
        || source.trim().is_empty()
        || source.len() > 4096
        || source.chars().any(char::is_control)
        || max_bytes == 0
        || max_bytes > engine::MCP_CONTEXT_MAX_BYTES
    {
        return Err(mcp_invalid_input("MCP context preview request is invalid"));
    }
    Ok(())
}

fn catalog_install(package: &McpCatalogPackage) -> Result<McpInstall, BackendError> {
    let version = package.version.clone().ok_or_else(|| {
        BackendError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Registry package does not declare an exact version",
        ))
    })?;
    let version = ExactPackageVersion::new(version).map_err(mcp_io_error)?;
    match package.registry_type.as_str() {
        "npm" => Ok(McpInstall::Npm {
            package: package.identifier.clone(),
            version,
        }),
        "pypi" => Ok(McpInstall::Pypi {
            package: package.identifier.clone(),
            version,
            executable: None,
        }),
        _ => Err(BackendError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "Registry package type is not supported",
        ))),
    }
}

fn catalog_arguments(package: &McpCatalogPackage, warnings: &mut Vec<String>) -> Vec<String> {
    package
        .runtime_arguments
        .iter()
        .chain(&package.package_arguments)
        .filter_map(|argument| {
            argument
                .value
                .clone()
                .or_else(|| argument.default.clone())
                .or_else(|| {
                    if argument.required {
                        warnings.push(format!(
                            "Required package argument `{}` needs a value before install.",
                            argument.name.as_deref().unwrap_or("unnamed")
                        ));
                    }
                    None
                })
        })
        .collect()
}

fn catalog_environment(
    server_id: &str,
    package: &McpCatalogPackage,
    warnings: &mut Vec<String>,
) -> Result<BTreeMap<String, PersistedValue>, BackendError> {
    let mut environment = BTreeMap::new();
    for input in &package.inputs {
        let Some(name) = input.name.as_deref().filter(|name| !name.trim().is_empty()) else {
            if input.required {
                warnings.push("Registry declares a required unnamed input.".to_string());
            }
            continue;
        };
        let value = if !input.secret {
            input
                .default
                .clone()
                .map(|value| PersistedValue::Literal { value })
        } else {
            None
        };
        let value = match value {
            Some(value) => value,
            None => PersistedValue::Secret {
                secret_ref: mcp_secret_ref(server_id, &format!("env.{name}"))
                    .map_err(mcp_io_error)?
                    .to_string(),
                resolved_value: None,
            },
        };
        environment.insert(name.to_string(), value);
    }
    Ok(environment)
}

fn catalog_headers(
    server_id: &str,
    inputs: &[crate::mcp::catalog::McpCatalogInput],
) -> Result<BTreeMap<String, PersistedValue>, BackendError> {
    let mut headers = BTreeMap::new();
    for input in inputs {
        let Some(name) = input.name.as_deref().filter(|name| !name.trim().is_empty()) else {
            continue;
        };
        let value = if !input.secret {
            input
                .default
                .clone()
                .map(|value| PersistedValue::Literal { value })
        } else {
            None
        }
        .unwrap_or(PersistedValue::Secret {
            secret_ref: mcp_secret_ref(server_id, &format!("header.{name}"))
                .map_err(mcp_io_error)?
                .to_string(),
            resolved_value: None,
        });
        headers.insert(name.to_string(), value);
    }
    Ok(headers)
}

fn registry_server_id(name: &str) -> String {
    let mut id = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while id.contains("--") {
        id = id.replace("--", "-");
    }
    id = id.trim_matches('-').to_string();
    if id.is_empty() {
        "registry-mcp".to_string()
    } else if id.len() > 96 {
        let digest = Sha256::digest(name.as_bytes());
        format!("{}-{digest:x}", &id[..63])
    } else {
        id
    }
}

fn validate_operation_id(operation_id: &str) -> Result<(), BackendError> {
    let valid = !operation_id.is_empty()
        && operation_id.len() <= 128
        && operation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(BackendError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid MCP install operation ID",
        )))
    }
}

fn install_result_state(status: PackageInstallStatus) -> McpInstallResultState {
    match status {
        PackageInstallStatus::Succeeded => McpInstallResultState::Succeeded,
        PackageInstallStatus::Failed => McpInstallResultState::Failed,
        PackageInstallStatus::Cancelled => McpInstallResultState::Cancelled,
        PackageInstallStatus::TimedOut => McpInstallResultState::TimedOut,
    }
}

fn mcp_io_error(error: impl std::fmt::Display) -> BackendError {
    BackendError::Io(io::Error::other(error.to_string()))
}

fn mcp_invalid_input(message: &'static str) -> BackendError {
    BackendError::Io(io::Error::new(io::ErrorKind::InvalidInput, message))
}

fn validate_oauth_scopes(scopes: &[String]) -> Result<(), BackendError> {
    let valid = scopes.len() <= 64
        && scopes.iter().all(|scope| {
            !scope.is_empty()
                && scope.len() <= 256
                && !scope.chars().any(char::is_control)
                && !scope.chars().any(char::is_whitespace)
        });
    if valid {
        Ok(())
    } else {
        Err(mcp_invalid_input("MCP OAuth scopes are invalid"))
    }
}

struct OAuthConnection {
    resource_url: String,
    allow_localhost: bool,
    client_id: String,
    configured_scopes: Vec<String>,
    expected_issuer: Option<String>,
}

fn oauth_connection(server: &McpServerRecord) -> Result<OAuthConnection, BackendError> {
    let (url, allow_localhost, auth) = match &server.connection {
        McpConnection::StreamableHttp {
            url,
            allow_localhost,
            auth,
            ..
        }
        | McpConnection::LegacySse {
            url,
            allow_localhost,
            auth,
            ..
        } => (url, *allow_localhost, auth),
        McpConnection::Stdio { .. } => return Err(mcp_invalid_input("MCP OAuth requires HTTP")),
    };
    let McpAuth::OAuth {
        client_id,
        scopes,
        issuer,
        ..
    } = auth
    else {
        return Err(mcp_invalid_input(
            "MCP connection is not configured for OAuth",
        ));
    };
    Ok(OAuthConnection {
        resource_url: url.clone(),
        allow_localhost,
        client_id: client_id.clone(),
        configured_scopes: scopes.clone(),
        expected_issuer: issuer.clone(),
    })
}

fn oauth_credential_ref(server: &McpServerRecord) -> Result<Option<String>, BackendError> {
    let (_, _, auth) = match &server.connection {
        McpConnection::StreamableHttp {
            url,
            allow_localhost,
            auth,
            ..
        }
        | McpConnection::LegacySse {
            url,
            allow_localhost,
            auth,
            ..
        } => (url, allow_localhost, auth),
        McpConnection::Stdio { .. } => return Err(mcp_invalid_input("MCP OAuth requires HTTP")),
    };
    match auth {
        McpAuth::OAuth { credential_ref, .. } => Ok(credential_ref.clone()),
        _ => Err(mcp_invalid_input(
            "MCP connection is not configured for OAuth",
        )),
    }
}

fn remote_auth_mut(connection: &mut McpConnection) -> Result<&mut McpAuth, BackendError> {
    match connection {
        McpConnection::StreamableHttp { auth, .. } | McpConnection::LegacySse { auth, .. } => {
            Ok(auth)
        }
        McpConnection::Stdio { .. } => Err(mcp_invalid_input("MCP OAuth requires HTTP")),
    }
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> BackendError {
    BackendError::Io(io::Error::other("MCP install state is unavailable"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::catalog::{McpCatalogArgument, McpCatalogInput};

    fn npm_package(version: Option<&str>) -> McpCatalogPackage {
        McpCatalogPackage {
            registry_type: "npm".to_string(),
            identifier: "massive-mcp".to_string(),
            version: version.map(str::to_string),
            runtime_hint: Some("npx".to_string()),
            transport_type: "stdio".to_string(),
            runtime_arguments: vec![McpCatalogArgument {
                argument_type: "positional".to_string(),
                name: Some("root".to_string()),
                value: None,
                default: None,
                description: None,
                required: true,
                secret: false,
            }],
            package_arguments: Vec::new(),
            inputs: vec![McpCatalogInput {
                name: Some("API_KEY".to_string()),
                description: None,
                default: Some("must-not-use".to_string()),
                required: true,
                secret: true,
            }],
        }
    }

    #[test]
    fn catalog_package_requires_exact_version_and_never_materializes_secret_defaults() {
        let package = npm_package(Some("2.1.0"));
        assert!(matches!(
            catalog_install(&package).unwrap(),
            McpInstall::Npm { .. }
        ));
        assert!(catalog_install(&npm_package(None)).is_err());
        let mut warnings = Vec::new();
        let environment = catalog_environment("massive", &package, &mut warnings).unwrap();
        assert!(matches!(
            environment["API_KEY"],
            PersistedValue::Secret { ref resolved_value, .. } if resolved_value.is_none()
        ));
        assert!(!format!("{environment:?}").contains("must-not-use"));
        assert!(catalog_arguments(&package, &mut warnings).is_empty());
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("Required package argument")));
    }

    #[test]
    fn registry_ids_and_install_operation_ids_are_bounded() {
        let id = registry_server_id(&format!("io.example/{}", "massive".repeat(40)));
        assert!(id.len() <= 128);
        assert!(id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
        assert!(validate_operation_id("019fbdb8-a0d6-7981-a5a9-72df0c50dfed").is_ok());
        assert!(validate_operation_id("../escape").is_err());
    }
}

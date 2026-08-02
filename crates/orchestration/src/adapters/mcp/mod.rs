//! MCP client adapter — connect stdio/HTTP servers, negotiate capabilities, call tools.

mod client_handler;
mod discover;
mod export;
mod http;
pub(crate) mod http_security;
mod legacy_sse;
pub mod oauth_http;
pub mod oauth_store;
mod package_installer;
mod registry;
mod secrets;

pub use client_handler::{
    McpRequestOrigin, McpRunClientRequest, McpRunClientRequestPayload, McpRunClientResponse,
};
pub use discover::{
    effective_mcp_servers, hydrate_mcp_server_from_path, import_mcp_servers_json,
    load_mcp_server_from_path, parse_mcp_servers_json, parse_mcp_servers_json_with_diagnostics,
    scan_external_mcp_for_api, McpImportError, McpParseDiagnostic, McpParseResult,
};
pub use export::{
    export_canonical_mcp_json, OPENFLOW_MCP_EXPORT_FORMAT, OPENFLOW_MCP_EXPORT_SCHEMA_VERSION,
};
pub use package_installer::{
    PackageInstallOutcome, PackageInstallStatus, PackageInstaller, PackageInstallerError,
};
pub use registry::{
    McpRegistryClient, McpRegistryError, RegistryArgument, RegistryIcon, RegistryInput,
    RegistryListParams, RegistryOfficialMetadata, RegistryPackage, RegistryPageMetadata,
    RegistryRemote, RegistryRepository, RegistryResponseMetadata, RegistryServerDetail,
    RegistryServerList, RegistryServerResponse, RegistryTransport, MCP_REGISTRY_DEFAULT_BASE_URL,
    MCP_REGISTRY_PREVIEW_LABEL,
};
pub(crate) use secrets::LegacyKeyringSecretStore;
pub use secrets::{FileSecretStore, MCP_SECRET_FILE_NAME};

use crate::mcp::capabilities::{
    McpPromptArgumentDescriptor, McpPromptDescriptor, McpResourceDescriptor,
};
use crate::mcp::model::{
    McpConnection, McpPolicy, McpToolAccess, McpToolConcurrency, McpTransportKind, PersistedValue,
};
use crate::settings::model::{McpServerConfig, McpSettings};
use engine::{ToolConcurrency, ToolDefinition, ToolTier};
use rmcp::{
    model::{
        CallToolRequest, CallToolRequestParams, CancelledNotificationParam, ClientRequest,
        GetPromptRequest, GetPromptRequestParams, ListPromptsRequest, ListResourcesRequest,
        ListToolsRequest, PaginatedRequestParams, PromptMessage, ReadResourceRequest,
        ReadResourceRequestParams, ResourceContents, ServerResult, Tool as McpTool,
    },
    service::{PeerRequestOptions, RunningService},
    transport::TokioChildProcess,
    RoleClient, ServiceError, ServiceExt,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;
use tokio::sync::{mpsc::UnboundedReceiver, mpsc::UnboundedSender, Mutex, RwLock};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

const MCP_PREFIX: &str = "mcp_";
const MCP_LEGACY_PREFIX: &str = "mcp/";
const MCP_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const MCP_LIST_TOOLS_TIMEOUT: Duration = Duration::from_secs(15);
const MCP_CAPABILITY_TIMEOUT: Duration = Duration::from_secs(15);
const MCP_CALL_TOOL_TIMEOUT: Duration = Duration::from_secs(120);
const MCP_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const MCP_MAX_CONCURRENT_SERVERS: usize = 4;
const MCP_MAX_CAPABILITY_ITEMS: usize = 1_000;

#[derive(Debug, Clone, Copy)]
struct McpRuntimePolicy {
    startup_timeout: Duration,
    list_tools_timeout: Duration,
    capability_timeout: Duration,
    call_tool_timeout: Duration,
    shutdown_timeout: Duration,
    max_concurrent_servers: usize,
}

impl Default for McpRuntimePolicy {
    fn default() -> Self {
        Self {
            startup_timeout: MCP_STARTUP_TIMEOUT,
            list_tools_timeout: MCP_LIST_TOOLS_TIMEOUT,
            capability_timeout: MCP_CAPABILITY_TIMEOUT,
            call_tool_timeout: MCP_CALL_TOOL_TIMEOUT,
            shutdown_timeout: MCP_SHUTDOWN_TIMEOUT,
            max_concurrent_servers: MCP_MAX_CONCURRENT_SERVERS,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum McpError {
    #[error("invalid MCP tool segment `{segment}`: must not be empty")]
    InvalidSegment { segment: String },
    #[error("invalid namespaced MCP tool name `{name}`")]
    InvalidNamespacedName { name: String },
    #[error("MCP server `{server_id}` is not connected")]
    ServerNotConnected { server_id: String },
    #[error("MCP command `{command}` was not found; install it or use an absolute path")]
    CommandNotFound { command: String },
    #[error("MCP server `{server_id}` is not trusted for its current config")]
    Untrusted { server_id: String },
    #[error("MCP server `{server_id}` uses unsupported transport `{transport:?}`")]
    UnsupportedTransport {
        server_id: String,
        transport: crate::mcp::model::McpTransportKind,
    },
    #[error("MCP server `{server_id}` requires unresolved secret `{secret_ref}`")]
    SecretUnavailable {
        server_id: String,
        secret_ref: String,
    },
    #[error("MCP server `{server_id}` remote endpoint was blocked: {reason}")]
    HttpSecurity { server_id: String, reason: String },
    #[error("MCP server `{server_id}` has an invalid HTTP header")]
    InvalidHttpHeader { server_id: String },
    #[error("MCP server `{server_id}` requires OAuth authorization")]
    OAuthRequired { server_id: String },
    #[error("MCP server `{server_id}` remote transport failed during {operation}")]
    RemoteTransport {
        server_id: String,
        operation: &'static str,
    },
    #[error("MCP server `{server_id}` did not start within {after_secs} seconds")]
    StartupTimeout { server_id: String, after_secs: u64 },
    #[error("MCP server `{server_id}` did not list tools within {after_secs} seconds")]
    ListToolsTimeout { server_id: String, after_secs: u64 },
    #[error("MCP server `{server_id}` did not {operation} within {after_secs} seconds")]
    CapabilityTimeout {
        server_id: String,
        operation: &'static str,
        after_secs: u64,
    },
    #[error("MCP server `{server_id}` returned more than {limit} {capability} items")]
    CapabilityLimit {
        server_id: String,
        capability: &'static str,
        limit: usize,
    },
    #[error("MCP server `{server_id}` could not {operation} `{source_id}`")]
    CapabilityRequest {
        server_id: String,
        operation: &'static str,
        source_id: String,
    },
    #[error(
        "MCP tool `{server_id}/{tool_name}` timed out after {after:?}; it may have completed on the server"
    )]
    CallToolTimeout {
        server_id: String,
        tool_name: String,
        after: Duration,
    },
    #[error("MCP tool `{server_id}/{tool_name}` was cancelled")]
    CallToolCancelled {
        server_id: String,
        tool_name: String,
    },
    #[error("MCP server `{server_id}` returned an unexpected response to {operation}")]
    UnexpectedResponse {
        server_id: String,
        operation: &'static str,
    },
    #[error("MCP transport error: {0}")]
    Transport(String),
    #[error("failed to project MCP tool result: {0}")]
    ResultProjection(String),
    #[error("MCP server `{server_id}` did not stop within {after_secs} seconds")]
    ShutdownTimeout { server_id: String, after_secs: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolOutcome {
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerMetadata {
    pub protocol_version: String,
    pub server_name: String,
    pub server_version: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSetupStage {
    Trust,
    Preflight,
    Connect,
    ListTools,
    Context,
}

impl std::fmt::Display for McpSetupStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trust => f.write_str("verify trust"),
            Self::Preflight => f.write_str("pass preflight"),
            Self::Connect => f.write_str("connect"),
            Self::ListTools => f.write_str("list tools"),
            Self::Context => f.write_str("prepare context"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSetupIssue {
    pub server_id: String,
    pub stage: McpSetupStage,
    pub error: McpError,
}

impl std::fmt::Display for McpSetupIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MCP server `{}` could not {} and was skipped: {}",
            self.server_id, self.stage, self.error
        )
    }
}

pub fn namespaced_tool_name(server_id: &str, tool_name: &str) -> Result<String, McpError> {
    validate_segment(server_id)?;
    validate_segment(tool_name)?;
    let server_id = encode_tool_segment(server_id);
    let tool_name = encode_tool_segment(tool_name);
    Ok(format!(
        "{MCP_PREFIX}{}_{server_id}_{tool_name}",
        server_id.len()
    ))
}

pub fn parse_namespaced_tool_name(name: &str) -> Result<(String, String), McpError> {
    if let Some(rest) = name.strip_prefix(MCP_LEGACY_PREFIX) {
        let (server_id, tool_name) = rest
            .split_once('/')
            .ok_or_else(|| invalid_namespaced_name(name))?;
        validate_segment(server_id)?;
        validate_segment(tool_name)?;
        return Ok((server_id.to_string(), tool_name.to_string()));
    }

    let rest = name
        .strip_prefix(MCP_PREFIX)
        .ok_or_else(|| invalid_namespaced_name(name))?;
    let (server_length, encoded) = rest
        .split_once('_')
        .ok_or_else(|| invalid_namespaced_name(name))?;
    let server_length = server_length
        .parse::<usize>()
        .map_err(|_| invalid_namespaced_name(name))?;
    if server_length == 0
        || encoded.len() <= server_length
        || encoded.as_bytes().get(server_length) != Some(&b'_')
    {
        return Err(invalid_namespaced_name(name));
    }
    let server_id = decode_tool_segment(&encoded[..server_length])
        .ok_or_else(|| invalid_namespaced_name(name))?;
    let tool_name = decode_tool_segment(&encoded[server_length + 1..])
        .ok_or_else(|| invalid_namespaced_name(name))?;
    validate_segment(&server_id)?;
    validate_segment(&tool_name)?;
    Ok((server_id, tool_name))
}

fn validate_segment(segment: &str) -> Result<(), McpError> {
    if segment.is_empty() {
        return Err(McpError::InvalidSegment {
            segment: segment.to_string(),
        });
    }
    Ok(())
}

fn invalid_namespaced_name(name: &str) -> McpError {
    McpError::InvalidNamespacedName {
        name: name.to_string(),
    }
}

fn encode_tool_segment(segment: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' => encoded.push(char::from(byte)),
            b'_' => encoded.push_str("__"),
            _ => {
                encoded.push_str("_x");
                encoded.push(char::from(HEX[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    encoded
}

fn decode_tool_segment(encoded: &str) -> Option<String> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(encoded.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'_' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        match bytes.get(index + 1) {
            Some(b'_') => {
                decoded.push(b'_');
                index += 2;
            }
            Some(b'x') => {
                let high = decode_hex(*bytes.get(index + 2)?)?;
                let low = decode_hex(*bytes.get(index + 3)?)?;
                decoded.push((high << 4) | low);
                index += 4;
            }
            _ => return None,
        }
    }
    String::from_utf8(decoded).ok()
}

const fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn mcp_tool_to_definition(
    server_id: &str,
    tool: &McpTool,
    policy: &McpPolicy,
) -> Result<ToolDefinition, McpError> {
    Ok(ToolDefinition {
        name: namespaced_tool_name(server_id, tool.name.as_ref())?,
        description: tool
            .description
            .as_ref()
            .map(|description| description.to_string())
            .unwrap_or_else(|| tool.name.to_string()),
        input_schema: serde_json::Value::Object(tool.input_schema.as_ref().clone()),
        tier: match policy.default_tool_access {
            McpToolAccess::Read => ToolTier::Read,
            McpToolAccess::Write => ToolTier::Write,
        },
        concurrency: match policy.default_tool_concurrency {
            McpToolConcurrency::Shared => ToolConcurrency::Shared,
            McpToolConcurrency::Exclusive => ToolConcurrency::Exclusive,
        },
    })
}

fn project_tool_result(result: &rmcp::model::CallToolResult) -> Result<McpToolOutcome, McpError> {
    let has_rich_content = result.structured_content.is_some()
        || result.content.iter().any(|block| block.as_text().is_none());
    let content = if has_rich_content {
        serde_json::to_string_pretty(result)
            .map_err(|error| McpError::ResultProjection(error.to_string()))?
    } else {
        result
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|text| text.text.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(McpToolOutcome {
        content,
        is_error: result.is_error.unwrap_or(false),
    })
}

fn truncate_utf8(mut content: String, max_bytes: u32) -> (String, u64, bool) {
    let original_size = content.len() as u64;
    let max_bytes = max_bytes as usize;
    if content.len() <= max_bytes {
        return (content, original_size, false);
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !content.is_char_boundary(boundary) {
        boundary -= 1;
    }
    content.truncate(boundary);
    (content, original_size, true)
}

fn project_resource_snapshot(
    server_id: &str,
    uri: &str,
    contents: &[ResourceContents],
    max_bytes: u32,
) -> Result<engine::McpContextSnapshot, McpError> {
    let mime_type = contents.first().and_then(|content| match content {
        ResourceContents::TextResourceContents { mime_type, .. }
        | ResourceContents::BlobResourceContents { mime_type, .. } => mime_type.clone(),
    });
    let content = match contents {
        [ResourceContents::TextResourceContents { text, .. }] => text.clone(),
        _ => serde_json::to_string_pretty(contents)
            .map_err(|error| McpError::ResultProjection(error.to_string()))?,
    };
    let (content, original_size_bytes, truncated) = truncate_utf8(content, max_bytes);
    Ok(engine::McpContextSnapshot {
        kind: engine::McpContextKind::Resource,
        server_id: server_id.to_string(),
        source: uri.to_string(),
        title: None,
        description: None,
        mime_type,
        included_size_bytes: content.len() as u64,
        content,
        original_size_bytes,
        truncated,
        error: None,
    })
}

fn project_prompt_snapshot(
    server_id: &str,
    name: &str,
    description: Option<String>,
    messages: &[PromptMessage],
    max_bytes: u32,
) -> Result<engine::McpContextSnapshot, McpError> {
    let content = serde_json::to_string_pretty(messages)
        .map_err(|error| McpError::ResultProjection(error.to_string()))?;
    let (content, original_size_bytes, truncated) = truncate_utf8(content, max_bytes);
    Ok(engine::McpContextSnapshot {
        kind: engine::McpContextKind::Prompt,
        server_id: server_id.to_string(),
        source: name.to_string(),
        title: None,
        description,
        mime_type: Some("application/json".to_string()),
        included_size_bytes: content.len() as u64,
        content,
        original_size_bytes,
        truncated,
        error: None,
    })
}

pub struct McpClient {
    service: RwLock<RunningService<RoleClient, client_handler::OpenFlowMcpClientHandler>>,
    server_id: String,
    transport: McpTransportKind,
    policy: McpRuntimePolicy,
    tool_policy: McpPolicy,
    handler: client_handler::OpenFlowMcpClientHandler,
    callback_call_gate: Mutex<()>,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("server_id", &self.server_id)
            .finish_non_exhaustive()
    }
}

impl McpClient {
    pub async fn spawn(config: &McpServerConfig) -> Result<Self, McpError> {
        Self::spawn_with_policy(config, McpRuntimePolicy::default()).await
    }

    #[cfg(test)]
    async fn spawn_with_timeout(
        config: &McpServerConfig,
        startup_timeout: Duration,
    ) -> Result<Self, McpError> {
        Self::spawn_with_policy(
            config,
            McpRuntimePolicy {
                startup_timeout,
                ..McpRuntimePolicy::default()
            },
        )
        .await
    }

    async fn spawn_with_policy(
        config: &McpServerConfig,
        policy: McpRuntimePolicy,
    ) -> Result<Self, McpError> {
        let handler = client_handler::OpenFlowMcpClientHandler::disconnected(
            config.id.clone(),
            config.policy.clone(),
        );
        Self::spawn_with_policy_and_handler(config, policy, handler).await
    }

    async fn spawn_with_policy_and_handler(
        config: &McpServerConfig,
        policy: McpRuntimePolicy,
        handler: client_handler::OpenFlowMcpClientHandler,
    ) -> Result<Self, McpError> {
        let transport_kind = config.connection.transport_kind();
        let service_handler = handler.clone();
        let startup = async {
            match &config.connection {
                McpConnection::Stdio {
                    command: executable,
                    args,
                    environment,
                } => {
                    let mut command = Command::new(executable);
                    command.args(args);
                    for (key, value) in environment {
                        match value {
                            PersistedValue::Literal { value } => {
                                command.env(key, value);
                            }
                            PersistedValue::Secret {
                                secret_ref,
                                resolved_value,
                            } => {
                                let Some(value) = resolved_value else {
                                    return Err(McpError::SecretUnavailable {
                                        server_id: config.id.clone(),
                                        secret_ref: secret_ref.clone(),
                                    });
                                };
                                command.env(key, value);
                            }
                        }
                    }
                    command.stdin(std::process::Stdio::piped());
                    command.stdout(std::process::Stdio::piped());
                    command.stderr(std::process::Stdio::piped());
                    command.kill_on_drop(true);
                    let transport = TokioChildProcess::new(command).map_err(|error| {
                        if error.kind() == std::io::ErrorKind::NotFound {
                            McpError::CommandNotFound {
                                command: executable.clone(),
                            }
                        } else {
                            McpError::Transport(error.to_string())
                        }
                    })?;
                    service_handler
                        .clone()
                        .serve(transport)
                        .await
                        .map_err(|error| McpError::Transport(error.to_string()))
                }
                McpConnection::StreamableHttp { .. } => {
                    let transport = http::streamable_http_transport(config).await?;
                    match service_handler.clone().serve(transport).await {
                        Ok(service) => Ok(service),
                        Err(error) if streamable_error_requires_oauth(&error) => {
                            Err(McpError::OAuthRequired {
                                server_id: config.id.clone(),
                            })
                        }
                        Err(error) if streamable_error_allows_legacy_fallback(&error) => {
                            let legacy =
                                legacy_sse::legacy_sse_transport_from_streamable(config).await?;
                            service_handler.clone().serve(legacy).await.map_err(|_| {
                                McpError::RemoteTransport {
                                    server_id: config.id.clone(),
                                    operation: "legacy SSE initialization",
                                }
                            })
                        }
                        Err(_) => Err(McpError::RemoteTransport {
                            server_id: config.id.clone(),
                            operation: "Streamable HTTP initialization",
                        }),
                    }
                }
                McpConnection::LegacySse { .. } => {
                    let transport = legacy_sse::legacy_sse_transport(config).await?;
                    service_handler.clone().serve(transport).await.map_err(|_| {
                        McpError::RemoteTransport {
                            server_id: config.id.clone(),
                            operation: "legacy SSE initialization",
                        }
                    })
                }
            }
        };
        let service = match tokio::time::timeout(policy.startup_timeout, startup).await {
            Ok(Ok(service)) => service,
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                return Err(McpError::StartupTimeout {
                    server_id: config.id.clone(),
                    after_secs: policy.startup_timeout.as_secs(),
                });
            }
        };

        Ok(Self {
            service: RwLock::new(service),
            server_id: config.id.clone(),
            transport: transport_kind,
            policy,
            tool_policy: config.policy.clone(),
            handler,
            callback_call_gate: Mutex::new(()),
        })
    }

    pub async fn list_tool_definitions(&self) -> Result<Vec<ToolDefinition>, McpError> {
        let deadline = tokio::time::Instant::now() + self.policy.list_tools_timeout;
        let mut tools = Vec::new();
        let mut cursor = None;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(self.list_tools_timeout_error());
            }
            let request = ClientRequest::ListToolsRequest(ListToolsRequest::with_param(
                PaginatedRequestParams::default().with_cursor(cursor),
            ));
            let result = match self.send_request(request, remaining).await {
                Ok(ServerResult::ListToolsResult(result)) => result,
                Ok(_) => {
                    return Err(McpError::UnexpectedResponse {
                        server_id: self.server_id.clone(),
                        operation: "list tools",
                    });
                }
                Err(ServiceError::Timeout { .. }) => return Err(self.list_tools_timeout_error()),
                Err(error) => return Err(self.service_error(error, "list tools")),
            };
            tools.extend(result.tools);
            cursor = result.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        tools
            .into_iter()
            .filter(|tool| {
                self.tool_policy
                    .enabled_tools
                    .as_ref()
                    .is_none_or(|enabled| enabled.contains(tool.name.as_ref()))
            })
            .map(|tool| mcp_tool_to_definition(&self.server_id, &tool, &self.tool_policy))
            .collect()
    }

    fn list_tools_timeout_error(&self) -> McpError {
        McpError::ListToolsTimeout {
            server_id: self.server_id.clone(),
            after_secs: self.policy.list_tools_timeout.as_secs(),
        }
    }

    fn service_error(&self, error: ServiceError, operation: &'static str) -> McpError {
        if matches!(
            self.transport,
            McpTransportKind::StreamableHttp | McpTransportKind::LegacySse
        ) {
            McpError::RemoteTransport {
                server_id: self.server_id.clone(),
                operation,
            }
        } else {
            McpError::Transport(error.to_string())
        }
    }

    async fn send_request(
        &self,
        request: ClientRequest,
        timeout: Duration,
    ) -> Result<ServerResult, ServiceError> {
        let peer = self.service.read().await.peer().clone();
        let mut options = PeerRequestOptions::default();
        options.timeout = Some(timeout);
        peer.send_cancellable_request(request, options)
            .await?
            .await_response()
            .await
    }

    pub async fn list_tool_names(&self) -> Result<Vec<String>, McpError> {
        Ok(self
            .list_tool_definitions()
            .await?
            .into_iter()
            .map(|definition| definition.name)
            .collect())
    }

    async fn advertised_capabilities(&self) -> (bool, bool, bool) {
        let service = self.service.read().await;
        let Some(info) = service.peer().peer_info() else {
            return (false, false, false);
        };
        let resources = info.capabilities.resources.as_ref();
        (
            resources.is_some(),
            resources.and_then(|capability| capability.subscribe) == Some(true),
            info.capabilities.prompts.is_some(),
        )
    }

    fn capability_timeout_error(&self, operation: &'static str) -> McpError {
        McpError::CapabilityTimeout {
            server_id: self.server_id.clone(),
            operation,
            after_secs: self.policy.capability_timeout.as_secs(),
        }
    }

    async fn capability_request(
        &self,
        request: ClientRequest,
        remaining: Duration,
        operation: &'static str,
        source: Option<&str>,
    ) -> Result<ServerResult, McpError> {
        match self.send_request(request, remaining).await {
            Ok(result) => Ok(result),
            Err(ServiceError::Timeout { .. }) => Err(self.capability_timeout_error(operation)),
            Err(_) => Err(McpError::CapabilityRequest {
                server_id: self.server_id.clone(),
                operation,
                source_id: source.unwrap_or("catalog").to_string(),
            }),
        }
    }

    pub async fn list_resource_descriptors(&self) -> Result<Vec<McpResourceDescriptor>, McpError> {
        let (supported, subscribable, _) = self.advertised_capabilities().await;
        if !supported {
            return Ok(Vec::new());
        }
        let deadline = tokio::time::Instant::now() + self.policy.capability_timeout;
        let mut resources = Vec::new();
        let mut cursor = None;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(self.capability_timeout_error("list resources"));
            }
            let request = ClientRequest::ListResourcesRequest(ListResourcesRequest::with_param(
                PaginatedRequestParams::default().with_cursor(cursor),
            ));
            let result = self
                .capability_request(request, remaining, "list resources", None)
                .await?;
            let ServerResult::ListResourcesResult(result) = result else {
                return Err(McpError::UnexpectedResponse {
                    server_id: self.server_id.clone(),
                    operation: "list resources",
                });
            };
            for resource in result.resources {
                if resources.len() == MCP_MAX_CAPABILITY_ITEMS {
                    return Err(McpError::CapabilityLimit {
                        server_id: self.server_id.clone(),
                        capability: "resource",
                        limit: MCP_MAX_CAPABILITY_ITEMS,
                    });
                }
                resources.push(McpResourceDescriptor {
                    server_id: self.server_id.clone(),
                    uri: resource.uri.clone(),
                    name: resource.name.clone(),
                    title: resource.title.clone(),
                    description: resource.description.clone(),
                    mime_type: resource.mime_type.clone(),
                    size_bytes: resource.size,
                    subscribable,
                });
            }
            cursor = result.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        Ok(resources)
    }

    pub async fn list_prompt_descriptors(&self) -> Result<Vec<McpPromptDescriptor>, McpError> {
        let (_, _, supported) = self.advertised_capabilities().await;
        if !supported {
            return Ok(Vec::new());
        }
        let deadline = tokio::time::Instant::now() + self.policy.capability_timeout;
        let mut prompts = Vec::new();
        let mut cursor = None;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(self.capability_timeout_error("list prompts"));
            }
            let request = ClientRequest::ListPromptsRequest(ListPromptsRequest::with_param(
                PaginatedRequestParams::default().with_cursor(cursor),
            ));
            let result = self
                .capability_request(request, remaining, "list prompts", None)
                .await?;
            let ServerResult::ListPromptsResult(result) = result else {
                return Err(McpError::UnexpectedResponse {
                    server_id: self.server_id.clone(),
                    operation: "list prompts",
                });
            };
            for prompt in result.prompts {
                if prompts.len() == MCP_MAX_CAPABILITY_ITEMS {
                    return Err(McpError::CapabilityLimit {
                        server_id: self.server_id.clone(),
                        capability: "prompt",
                        limit: MCP_MAX_CAPABILITY_ITEMS,
                    });
                }
                prompts.push(McpPromptDescriptor {
                    server_id: self.server_id.clone(),
                    name: prompt.name,
                    title: prompt.title,
                    description: prompt.description,
                    arguments: prompt
                        .arguments
                        .unwrap_or_default()
                        .into_iter()
                        .map(|argument| McpPromptArgumentDescriptor {
                            name: argument.name,
                            title: argument.title,
                            description: argument.description,
                            required: argument.required.unwrap_or(false),
                        })
                        .collect(),
                });
            }
            cursor = result.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        Ok(prompts)
    }

    pub async fn capability_catalog(
        &self,
    ) -> Result<crate::mcp::capabilities::McpCapabilityCatalog, McpError> {
        let (resources, prompts) = tokio::join!(
            self.list_resource_descriptors(),
            self.list_prompt_descriptors()
        );
        Ok(crate::mcp::capabilities::McpCapabilityCatalog {
            server_id: self.server_id.clone(),
            resources: resources?,
            prompts: prompts?,
        })
    }

    pub async fn read_resource_snapshot(
        &self,
        uri: &str,
        max_bytes: u32,
    ) -> Result<engine::McpContextSnapshot, McpError> {
        let (supported, _, _) = self.advertised_capabilities().await;
        if !supported {
            return Err(McpError::CapabilityRequest {
                server_id: self.server_id.clone(),
                operation: "read resource",
                source_id: uri.to_string(),
            });
        }
        let request = ClientRequest::ReadResourceRequest(ReadResourceRequest::new(
            ReadResourceRequestParams::new(uri),
        ));
        let result = self
            .capability_request(
                request,
                self.policy.capability_timeout,
                "read resource",
                Some(uri),
            )
            .await?;
        let ServerResult::ReadResourceResult(result) = result else {
            return Err(McpError::UnexpectedResponse {
                server_id: self.server_id.clone(),
                operation: "read resource",
            });
        };
        project_resource_snapshot(&self.server_id, uri, &result.contents, max_bytes)
    }

    pub async fn get_prompt_snapshot(
        &self,
        name: &str,
        arguments: &std::collections::BTreeMap<String, String>,
        max_bytes: u32,
    ) -> Result<engine::McpContextSnapshot, McpError> {
        let (_, _, supported) = self.advertised_capabilities().await;
        if !supported {
            return Err(McpError::CapabilityRequest {
                server_id: self.server_id.clone(),
                operation: "get prompt",
                source_id: name.to_string(),
            });
        }
        let mut params = GetPromptRequestParams::new(name);
        if !arguments.is_empty() {
            params = params.with_arguments(
                arguments
                    .iter()
                    .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                    .collect(),
            );
        }
        let request = ClientRequest::GetPromptRequest(GetPromptRequest::new(params));
        let result = self
            .capability_request(
                request,
                self.policy.capability_timeout,
                "get prompt",
                Some(name),
            )
            .await?;
        let ServerResult::GetPromptResult(result) = result else {
            return Err(McpError::UnexpectedResponse {
                server_id: self.server_id.clone(),
                operation: "get prompt",
            });
        };
        project_prompt_snapshot(
            &self.server_id,
            name,
            result.description,
            &result.messages,
            max_bytes,
        )
    }

    pub async fn server_metadata(&self) -> Option<McpServerMetadata> {
        let service = self.service.read().await;
        let info = service.peer().peer_info()?.clone();
        let capabilities = &info.capabilities;
        let mut names = Vec::new();
        if capabilities.tools.is_some() {
            names.push("tools".to_string());
        }
        if capabilities.resources.is_some() {
            names.push("resources".to_string());
        }
        if capabilities.prompts.is_some() {
            names.push("prompts".to_string());
        }
        if capabilities.logging.is_some() {
            names.push("logging".to_string());
        }
        if capabilities.completions.is_some() {
            names.push("completions".to_string());
        }
        if capabilities.tasks.is_some() {
            names.push("tasks".to_string());
        }
        if capabilities.extensions.is_some() {
            names.push("extensions".to_string());
        }
        if capabilities.experimental.is_some() {
            names.push("experimental".to_string());
        }
        Some(McpServerMetadata {
            protocol_version: info.protocol_version.to_string(),
            server_name: info.server_info.name,
            server_version: info.server_info.version,
            capabilities: names,
        })
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
    ) -> Result<McpToolOutcome, McpError> {
        self.call_tool_with_cancel(tool_name, args, &CancellationToken::new())
            .await
    }

    pub async fn call_tool_with_cancel(
        &self,
        tool_name: &str,
        args: Value,
        cancel: &CancellationToken,
    ) -> Result<McpToolOutcome, McpError> {
        self.call_tool_with_origin(tool_name, args, cancel, None)
            .await
    }

    async fn call_tool_with_origin(
        &self,
        tool_name: &str,
        args: Value,
        cancel: &CancellationToken,
        origin: Option<McpRequestOrigin>,
    ) -> Result<McpToolOutcome, McpError> {
        if cancel.is_cancelled() {
            return Err(McpError::CallToolCancelled {
                server_id: self.server_id.clone(),
                tool_name: tool_name.to_string(),
            });
        }
        let _callback_gate = if self.handler.callbacks_enabled() {
            Some(self.callback_call_gate.lock().await)
        } else {
            None
        };
        let _origin_guard = origin.map(|origin| self.handler.set_active_origin(origin));
        let mut params = CallToolRequestParams::new(tool_name.to_string());
        if let Some(arguments) = args.as_object().cloned() {
            params = params.with_arguments(arguments);
        }
        let request = ClientRequest::CallToolRequest(CallToolRequest::new(params));
        let peer = self.service.read().await.peer().clone();
        let mut options = PeerRequestOptions::default();
        options.timeout = Some(self.policy.call_tool_timeout);
        let handle = peer
            .send_cancellable_request(request, options)
            .await
            .map_err(|error| self.service_error(error, "call tool"))?;
        let request_id = handle.id.clone();
        let result = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                let _ = peer.notify_cancelled(CancelledNotificationParam {
                    request_id,
                    reason: Some("OpenFlow run cancelled".to_string()),
                }).await;
                return Err(McpError::CallToolCancelled {
                    server_id: self.server_id.clone(),
                    tool_name: tool_name.to_string(),
                });
            }
            result = handle.await_response() => result,
        };
        let result = match result {
            Ok(ServerResult::CallToolResult(result)) => result,
            Ok(_) => {
                return Err(McpError::UnexpectedResponse {
                    server_id: self.server_id.clone(),
                    operation: "call tool",
                });
            }
            Err(ServiceError::Timeout { .. }) => {
                return Err(McpError::CallToolTimeout {
                    server_id: self.server_id.clone(),
                    tool_name: tool_name.to_string(),
                    after: self.policy.call_tool_timeout,
                });
            }
            Err(error) => return Err(self.service_error(error, "call tool")),
        };
        project_tool_result(&result)
    }

    pub async fn close(&self) -> Result<(), McpError> {
        let mut service = self.service.write().await;
        match service
            .close_with_timeout(self.policy.shutdown_timeout)
            .await
        {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(McpError::ShutdownTimeout {
                server_id: self.server_id.clone(),
                after_secs: self.policy.shutdown_timeout.as_secs(),
            }),
            Err(_error)
                if matches!(
                    self.transport,
                    McpTransportKind::StreamableHttp | McpTransportKind::LegacySse
                ) =>
            {
                Err(McpError::RemoteTransport {
                    server_id: self.server_id.clone(),
                    operation: "close session",
                })
            }
            Err(error) => Err(McpError::Transport(error.to_string())),
        }
    }
}

pub type McpStdioClient = McpClient;

fn streamable_error_requires_oauth(error: &rmcp::service::ClientInitializeError) -> bool {
    matches!(
        streamable_http_initialization_error(error),
        Some(
            rmcp::transport::streamable_http_client::StreamableHttpError::AuthRequired(_)
                | rmcp::transport::streamable_http_client::StreamableHttpError::InsufficientScope(
                    _
                )
        )
    )
}

fn streamable_error_allows_legacy_fallback(error: &rmcp::service::ClientInitializeError) -> bool {
    let Some(error) = streamable_http_initialization_error(error) else {
        return false;
    };
    match error {
        rmcp::transport::streamable_http_client::StreamableHttpError::Client(error) => error
            .status()
            .is_some_and(|status| matches!(status.as_u16(), 400 | 404 | 405)),
        rmcp::transport::streamable_http_client::StreamableHttpError::UnexpectedServerResponse(
            message,
        ) => message
            .strip_prefix("HTTP ")
            .and_then(|message| message.split_whitespace().next())
            .and_then(|status| status.parse::<u16>().ok())
            .is_some_and(|status| matches!(status, 400 | 404 | 405)),
        _ => false,
    }
}

fn streamable_http_initialization_error(
    error: &rmcp::service::ClientInitializeError,
) -> Option<&rmcp::transport::streamable_http_client::StreamableHttpError<reqwest::Error>> {
    let rmcp::service::ClientInitializeError::TransportError { error, .. } = error else {
        return None;
    };
    error.error.downcast_ref::<
        rmcp::transport::streamable_http_client::StreamableHttpError<reqwest::Error>,
    >()
}

pub struct McpRunClients {
    clients: HashMap<String, Arc<McpClient>>,
    max_concurrent_servers: usize,
    callback_rx: parking_lot::Mutex<Option<UnboundedReceiver<McpRunClientRequest>>>,
}

struct McpClientHostContext {
    project_root: Option<PathBuf>,
    callback_tx: UnboundedSender<McpRunClientRequest>,
    callback_rx: Option<UnboundedReceiver<McpRunClientRequest>>,
}

fn handler_for_config(
    config: &McpServerConfig,
    host: Option<&McpClientHostContext>,
) -> client_handler::OpenFlowMcpClientHandler {
    match host {
        Some(host) => client_handler::OpenFlowMcpClientHandler::for_run(
            config.id.clone(),
            config.policy.clone(),
            host.project_root.as_deref(),
            host.callback_tx.clone(),
        ),
        None => client_handler::OpenFlowMcpClientHandler::disconnected(
            config.id.clone(),
            config.policy.clone(),
        ),
    }
}

impl std::fmt::Debug for McpRunClients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpRunClients")
            .field("server_ids", &self.clients.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl McpRunClients {
    pub async fn connect(settings: &McpSettings) -> (Self, Vec<McpSetupIssue>) {
        Self::connect_with_policy(settings, McpRuntimePolicy::default()).await
    }

    pub async fn connect_for_run(
        settings: &McpSettings,
        project_root: Option<&Path>,
    ) -> (Self, Vec<McpSetupIssue>) {
        let (callback_tx, callback_rx) = tokio::sync::mpsc::unbounded_channel();
        Self::connect_with_policy_and_host(
            settings,
            McpRuntimePolicy::default(),
            Some(McpClientHostContext {
                project_root: project_root.map(Path::to_path_buf),
                callback_tx,
                callback_rx: Some(callback_rx),
            }),
        )
        .await
    }

    async fn connect_with_policy(
        settings: &McpSettings,
        policy: McpRuntimePolicy,
    ) -> (Self, Vec<McpSetupIssue>) {
        Self::connect_with_policy_and_host(settings, policy, None).await
    }

    async fn connect_with_policy_and_host(
        settings: &McpSettings,
        policy: McpRuntimePolicy,
        mut host: Option<McpClientHostContext>,
    ) -> (Self, Vec<McpSetupIssue>) {
        let concurrency = policy.max_concurrent_servers.max(1);
        let mut setup_issues = Vec::new();
        let path = std::env::var_os("PATH");
        let mut configs = settings
            .servers
            .iter()
            .enumerate()
            .filter_map(|(index, server)| {
                if !server.enabled {
                    return None;
                }
                if !crate::mcp::trust::is_trusted(server) {
                    setup_issues.push((
                        index,
                        McpSetupIssue {
                            server_id: server.id.clone(),
                            stage: McpSetupStage::Trust,
                            error: McpError::Untrusted {
                                server_id: server.id.clone(),
                            },
                        },
                    ));
                    return None;
                }
                match crate::mcp::preflight::preflight(&server.connection, path.as_deref()) {
                    crate::mcp::preflight::McpPreflight::Ready { .. }
                    | crate::mcp::preflight::McpPreflight::RemoteReady { .. } => {
                        Some((index, server.clone()))
                    }
                    crate::mcp::preflight::McpPreflight::Missing { command, .. } => {
                        setup_issues.push((
                            index,
                            McpSetupIssue {
                                server_id: server.id.clone(),
                                stage: McpSetupStage::Preflight,
                                error: McpError::CommandNotFound { command },
                            },
                        ));
                        None
                    }
                    crate::mcp::preflight::McpPreflight::UnsupportedTransport { transport } => {
                        setup_issues.push((
                            index,
                            McpSetupIssue {
                                server_id: server.id.clone(),
                                stage: McpSetupStage::Preflight,
                                error: McpError::UnsupportedTransport {
                                    server_id: server.id.clone(),
                                    transport,
                                },
                            },
                        ));
                        None
                    }
                    crate::mcp::preflight::McpPreflight::InvalidRemote { reason } => {
                        setup_issues.push((
                            index,
                            McpSetupIssue {
                                server_id: server.id.clone(),
                                stage: McpSetupStage::Preflight,
                                error: McpError::HttpSecurity {
                                    server_id: server.id.clone(),
                                    reason,
                                },
                            },
                        ));
                        None
                    }
                }
            });
        let mut tasks = JoinSet::new();
        let mut task_servers = HashMap::new();
        while tasks.len() < concurrency {
            let Some((index, config)) = configs.next() else {
                break;
            };
            let server_id = config.id.clone();
            let handler = handler_for_config(&config, host.as_ref());
            let task = tasks.spawn(async move {
                McpClient::spawn_with_policy_and_handler(&config, policy, handler).await
            });
            task_servers.insert(task.id(), (index, server_id));
        }
        let mut results = Vec::new();
        while let Some(joined) = tasks.join_next_with_id().await {
            match joined {
                Ok((task_id, result)) => {
                    if let Some((index, server_id)) = task_servers.remove(&task_id) {
                        results.push((index, server_id, result));
                    }
                }
                Err(error) => {
                    if let Some((index, server_id)) = task_servers.remove(&error.id()) {
                        results.push((
                            index,
                            server_id,
                            Err(McpError::Transport(format!(
                                "MCP startup task failed: {error}"
                            ))),
                        ));
                    }
                }
            }
            if let Some((index, config)) = configs.next() {
                let server_id = config.id.clone();
                let handler = handler_for_config(&config, host.as_ref());
                let task = tasks.spawn(async move {
                    McpClient::spawn_with_policy_and_handler(&config, policy, handler).await
                });
                task_servers.insert(task.id(), (index, server_id));
            }
        }
        results.sort_by_key(|(index, _, _)| *index);

        let mut clients = HashMap::new();
        let mut issues = setup_issues;
        for (index, server_id, result) in results {
            let client = match result {
                Ok(client) => client,
                Err(error) => {
                    issues.push((
                        index,
                        McpSetupIssue {
                            server_id,
                            stage: McpSetupStage::Connect,
                            error,
                        },
                    ));
                    continue;
                }
            };
            clients.insert(server_id, Arc::new(client));
        }
        issues.sort_by_key(|(index, _)| *index);
        (
            Self {
                clients,
                max_concurrent_servers: concurrency,
                callback_rx: parking_lot::Mutex::new(
                    host.as_mut().and_then(|host| host.callback_rx.take()),
                ),
            },
            issues.into_iter().map(|(_, issue)| issue).collect(),
        )
    }

    pub fn take_client_request_receiver(&self) -> Option<UnboundedReceiver<McpRunClientRequest>> {
        self.callback_rx.lock().take()
    }

    pub async fn list_all_tool_definitions(&self) -> (Vec<ToolDefinition>, Vec<McpSetupIssue>) {
        let mut servers = Vec::with_capacity(self.clients.len());
        for (server_id, client) in &self.clients {
            servers.push((server_id.clone(), Arc::clone(client)));
        }
        servers.sort_by(|(left, _), (right, _)| left.cmp(right));
        let mut servers = servers.into_iter();
        let mut tasks = JoinSet::new();
        let mut task_servers = HashMap::new();
        while tasks.len() < self.max_concurrent_servers {
            let Some((server_id, client)) = servers.next() else {
                break;
            };
            let task = tasks.spawn(async move { client.list_tool_definitions().await });
            task_servers.insert(task.id(), server_id);
        }
        let mut results = Vec::new();
        while let Some(joined) = tasks.join_next_with_id().await {
            match joined {
                Ok((task_id, result)) => {
                    if let Some(server_id) = task_servers.remove(&task_id) {
                        results.push((server_id, result));
                    }
                }
                Err(error) => {
                    if let Some(server_id) = task_servers.remove(&error.id()) {
                        results.push((
                            server_id,
                            Err(McpError::Transport(format!(
                                "MCP tool-list task failed: {error}"
                            ))),
                        ));
                    }
                }
            }
            if let Some((server_id, client)) = servers.next() {
                let task = tasks.spawn(async move { client.list_tool_definitions().await });
                task_servers.insert(task.id(), server_id);
            }
        }

        let mut definitions = Vec::new();
        let mut issues = Vec::new();
        results.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (server_id, result) in results {
            match result {
                Ok(server_definitions) => definitions.extend(server_definitions),
                Err(error) => issues.push(McpSetupIssue {
                    server_id,
                    stage: McpSetupStage::ListTools,
                    error,
                }),
            }
        }
        (definitions, issues)
    }

    pub async fn resolve_workflow_context(&self, workflow: &mut engine::Workflow) {
        for node in &mut workflow.nodes {
            if !node.agent.mcp_context_snapshots.is_empty() {
                continue;
            }
            let mut snapshots =
                Vec::with_capacity(node.agent.mcp_resources.len() + node.agent.mcp_prompts.len());
            for selection in &node.agent.mcp_resources {
                let result = match self.clients.get(&selection.server_id) {
                    Some(client) => {
                        client
                            .read_resource_snapshot(&selection.uri, selection.max_bytes)
                            .await
                    }
                    None => Err(McpError::ServerNotConnected {
                        server_id: selection.server_id.clone(),
                    }),
                };
                snapshots.push(result.unwrap_or_else(|error| engine::McpContextSnapshot {
                    kind: engine::McpContextKind::Resource,
                    server_id: selection.server_id.clone(),
                    source: selection.uri.clone(),
                    title: None,
                    description: None,
                    mime_type: None,
                    content: String::new(),
                    original_size_bytes: 0,
                    included_size_bytes: 0,
                    truncated: false,
                    error: Some(error.to_string()),
                }));
            }
            for selection in &node.agent.mcp_prompts {
                let result = match self.clients.get(&selection.server_id) {
                    Some(client) => {
                        client
                            .get_prompt_snapshot(
                                &selection.name,
                                &selection.arguments,
                                selection.max_bytes,
                            )
                            .await
                    }
                    None => Err(McpError::ServerNotConnected {
                        server_id: selection.server_id.clone(),
                    }),
                };
                snapshots.push(result.unwrap_or_else(|error| engine::McpContextSnapshot {
                    kind: engine::McpContextKind::Prompt,
                    server_id: selection.server_id.clone(),
                    source: selection.name.clone(),
                    title: None,
                    description: None,
                    mime_type: None,
                    content: String::new(),
                    original_size_bytes: 0,
                    included_size_bytes: 0,
                    truncated: false,
                    error: Some(error.to_string()),
                }));
            }
            node.agent.mcp_context_snapshots = snapshots;
        }
    }

    pub async fn capability_catalog(
        &self,
        server_id: &str,
    ) -> Result<crate::mcp::capabilities::McpCapabilityCatalog, McpError> {
        self.clients
            .get(server_id)
            .ok_or_else(|| McpError::ServerNotConnected {
                server_id: server_id.to_string(),
            })?
            .capability_catalog()
            .await
    }

    pub async fn call_namespaced(
        &self,
        namespaced_name: &str,
        args: Value,
    ) -> Result<McpToolOutcome, McpError> {
        self.call_namespaced_with_cancel(namespaced_name, args, &CancellationToken::new())
            .await
    }

    pub async fn call_namespaced_with_cancel(
        &self,
        namespaced_name: &str,
        args: Value,
        cancel: &CancellationToken,
    ) -> Result<McpToolOutcome, McpError> {
        let (server_id, tool_name) = parse_namespaced_tool_name(namespaced_name)?;
        let client = self
            .clients
            .get(&server_id)
            .ok_or_else(|| McpError::ServerNotConnected {
                server_id: server_id.clone(),
            })?;
        client.call_tool_with_cancel(&tool_name, args, cancel).await
    }

    pub async fn call_namespaced_for_origin(
        &self,
        namespaced_name: &str,
        args: Value,
        cancel: &CancellationToken,
        origin: McpRequestOrigin,
    ) -> Result<McpToolOutcome, McpError> {
        let (server_id, tool_name) = parse_namespaced_tool_name(namespaced_name)?;
        let client = self
            .clients
            .get(&server_id)
            .ok_or_else(|| McpError::ServerNotConnected {
                server_id: server_id.clone(),
            })?;
        client
            .call_tool_with_origin(&tool_name, args, cancel, Some(origin))
            .await
    }

    pub async fn close(&self) -> Result<(), McpError> {
        let mut servers = Vec::with_capacity(self.clients.len());
        for (server_id, client) in &self.clients {
            servers.push((server_id.clone(), Arc::clone(client)));
        }
        servers.sort_by(|(left, _), (right, _)| left.cmp(right));
        let mut servers = servers.into_iter();
        let mut tasks = JoinSet::new();
        let mut task_servers = HashMap::new();
        while tasks.len() < self.max_concurrent_servers {
            let Some((server_id, client)) = servers.next() else {
                break;
            };
            let task = tasks.spawn(async move { client.close().await });
            task_servers.insert(task.id(), server_id);
        }
        let mut results = Vec::new();
        while let Some(joined) = tasks.join_next_with_id().await {
            match joined {
                Ok((task_id, result)) => {
                    if let Some(server_id) = task_servers.remove(&task_id) {
                        results.push((server_id, result));
                    }
                }
                Err(error) => {
                    if let Some(server_id) = task_servers.remove(&error.id()) {
                        results.push((
                            server_id,
                            Err(McpError::Transport(format!(
                                "MCP shutdown task failed: {error}"
                            ))),
                        ));
                    }
                }
            }
            if let Some((server_id, client)) = servers.next() {
                let task = tasks.spawn(async move { client.close().await });
                task_servers.insert(task.id(), server_id);
            }
        }
        results.sort_by(|(left, _), (right, _)| left.cmp(right));
        results
            .into_iter()
            .find_map(|(_, result)| result.err())
            .map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::model::{McpInstall, McpServerRecord, McpServerSource};

    fn stdio_server(
        id: &str,
        display_name: &str,
        command: &str,
        args: Vec<String>,
        env: std::collections::BTreeMap<String, String>,
        enabled: bool,
        trusted: bool,
    ) -> McpServerConfig {
        let mut server = McpServerRecord::new(
            id,
            display_name,
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: command.to_string(),
                args,
                environment: env
                    .into_iter()
                    .map(|(key, value)| (key, PersistedValue::Literal { value }))
                    .collect(),
            },
        );
        server.enabled = enabled;
        if trusted {
            crate::mcp::trust::approve_current(&mut server, chrono::Utc::now()).unwrap();
        }
        server
    }

    fn test_runtime_policy() -> McpRuntimePolicy {
        McpRuntimePolicy {
            startup_timeout: Duration::from_millis(300),
            list_tools_timeout: Duration::from_millis(100),
            capability_timeout: Duration::from_millis(100),
            call_tool_timeout: Duration::from_millis(25),
            shutdown_timeout: Duration::from_millis(100),
            max_concurrent_servers: 2,
        }
    }

    #[test]
    fn tool_result_projection_preserves_error_status_for_text() {
        let result: rmcp::model::CallToolResult = serde_json::from_value(serde_json::json!({
            "content": [
                {"type": "text", "text": "invalid input"},
                {"type": "text", "text": "try again"}
            ],
            "isError": true
        }))
        .unwrap();

        let projected = project_tool_result(&result).unwrap();

        assert_eq!(projected.content, "invalid input\ntry again");
        assert!(projected.is_error);
    }

    #[test]
    fn tool_result_projection_preserves_structured_and_non_text_content() {
        let result: rmcp::model::CallToolResult = serde_json::from_value(serde_json::json!({
            "content": [
                {"type": "text", "text": "before"},
                {"type": "image", "data": "aW1hZ2U=", "mimeType": "image/png"},
                {"type": "text", "text": "after"}
            ],
            "structuredContent": {"answer": 42}
        }))
        .unwrap();

        let projected = project_tool_result(&result).unwrap();
        let content: Value = serde_json::from_str(&projected.content).unwrap();

        assert_eq!(content["content"][1]["type"], "image");
        assert_eq!(content["content"][1]["mimeType"], "image/png");
        assert_eq!(content["structuredContent"]["answer"], 42);
        assert!(!projected.is_error);
    }

    #[test]
    fn discovered_mcp_tools_are_server_exclusive_by_default() {
        let tool: McpTool = serde_json::from_value(serde_json::json!({
            "name": "write_file",
            "description": "Write a file",
            "inputSchema": {"type": "object"}
        }))
        .expect("MCP tool");

        let definition =
            mcp_tool_to_definition("filesystem", &tool, &McpPolicy::default()).expect("definition");

        assert_eq!(definition.concurrency, ToolConcurrency::Exclusive);
    }

    #[tokio::test]
    async fn closing_empty_run_clients_is_idempotent() {
        let clients = McpRunClients {
            clients: HashMap::new(),
            max_concurrent_servers: MCP_MAX_CONCURRENT_SERVERS,
            callback_rx: parking_lot::Mutex::new(None),
        };

        clients.close().await.unwrap();
        clients.close().await.unwrap();
    }

    #[tokio::test]
    async fn mcp_resources_and_prompts_require_explicit_selection_and_freeze_bounded_context() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let (server_transport, client_transport) = tokio::io::duplex(16_384);
        let server = tokio::spawn(async move {
            let (server_read, mut server_write) = tokio::io::split(server_transport);
            let mut lines = BufReader::new(server_read).lines();
            let initialize: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("initialize request"),
            )
            .unwrap();
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": initialize["id"],
                "result": {
                    "protocolVersion": initialize["params"]["protocolVersion"],
                    "capabilities": {
                        "resources": {"subscribe": true},
                        "prompts": {}
                    },
                    "serverInfo": {"name": "context-test", "version": "1.0.0"}
                }
            });
            server_write
                .write_all(format!("{response}\n").as_bytes())
                .await
                .unwrap();
            let initialized: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("initialized notification"),
            )
            .unwrap();
            assert_eq!(initialized["method"], "notifications/initialized");

            let mut methods = Vec::new();
            for _ in 0..4 {
                let request: Value = serde_json::from_str(
                    &lines
                        .next_line()
                        .await
                        .unwrap()
                        .expect("capability request"),
                )
                .unwrap();
                let method = request["method"].as_str().unwrap().to_string();
                methods.push(method.clone());
                let result = match method.as_str() {
                    "resources/list" => serde_json::json!({
                        "resources": [{
                            "uri": "docs://guide",
                            "name": "guide",
                            "title": "Guide",
                            "description": "Trusted provenance, untrusted content",
                            "mimeType": "text/plain",
                            "size": 8
                        }]
                    }),
                    "prompts/list" => serde_json::json!({
                        "prompts": [{
                            "name": "review",
                            "description": "Review a topic",
                            "arguments": [{"name": "topic", "required": true}]
                        }]
                    }),
                    "resources/read" => {
                        assert_eq!(request["params"]["uri"], "docs://guide");
                        serde_json::json!({
                            "contents": [{
                                "uri": "docs://guide",
                                "mimeType": "text/plain",
                                "text": "abcdefgh"
                            }]
                        })
                    }
                    "prompts/get" => {
                        assert_eq!(request["params"]["name"], "review");
                        assert_eq!(request["params"]["arguments"]["topic"], "Rust");
                        serde_json::json!({
                            "description": "Rendered review",
                            "messages": [{
                                "role": "user",
                                "content": {"type": "text", "text": "Review Rust"}
                            }]
                        })
                    }
                    other => panic!("unexpected MCP request {other}"),
                };
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request["id"],
                    "result": result
                });
                server_write
                    .write_all(format!("{response}\n").as_bytes())
                    .await
                    .unwrap();
            }
            let eof = tokio::time::timeout(Duration::from_secs(1), lines.next_line())
                .await
                .expect("client must close transport")
                .unwrap();
            assert!(eof.is_none());
            methods
        });
        let handler =
            client_handler::OpenFlowMcpClientHandler::disconnected("docs", McpPolicy::default());
        let service = handler.clone().serve(client_transport).await.unwrap();
        let client = Arc::new(McpClient {
            service: RwLock::new(service),
            server_id: "docs".to_string(),
            transport: McpTransportKind::Stdio,
            policy: test_runtime_policy(),
            tool_policy: McpPolicy::default(),
            handler,
            callback_call_gate: Mutex::new(()),
        });
        let clients = McpRunClients {
            clients: HashMap::from([("docs".to_string(), client)]),
            max_concurrent_servers: 1,
            callback_rx: parking_lot::Mutex::new(None),
        };

        let catalog = clients.capability_catalog("docs").await.unwrap();
        assert_eq!(catalog.resources[0].uri, "docs://guide");
        assert!(catalog.resources[0].subscribable);
        assert_eq!(catalog.prompts[0].arguments[0].name, "topic");

        let mut workflow = engine::Workflow::new("context");
        let mut node = engine::Node::agent("context", 0.0, 0.0);
        node.agent.mcp_resources.push(engine::McpResourceSelection {
            server_id: "docs".to_string(),
            uri: "docs://guide".to_string(),
            max_bytes: 5,
        });
        node.agent.mcp_prompts.push(engine::McpPromptSelection {
            server_id: "docs".to_string(),
            name: "review".to_string(),
            arguments: std::collections::BTreeMap::from([(
                "topic".to_string(),
                "Rust".to_string(),
            )]),
            max_bytes: 256,
        });
        workflow.nodes.push(node);

        clients.resolve_workflow_context(&mut workflow).await;
        let snapshots = &workflow.nodes[0].agent.mcp_context_snapshots;
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].content, "abcde");
        assert!(snapshots[0].truncated);
        assert_eq!(snapshots[0].server_id, "docs");
        assert!(snapshots[1].content.contains("Review Rust"));
        clients.close().await.unwrap();

        let methods = server.await.unwrap();
        assert!(methods.contains(&"resources/list".to_string()));
        assert!(methods.contains(&"prompts/list".to_string()));
        assert!(!methods.contains(&"resources/subscribe".to_string()));
    }

    #[tokio::test]
    async fn connection_failure_is_reported_without_rejecting_other_setup() {
        let settings = McpSettings {
            servers: vec![stdio_server(
                "missing",
                "Missing",
                "/definitely/not/a/real/openflow-mcp-server",
                Vec::new(),
                Default::default(),
                true,
                true,
            )],
            discover_external: false,
            disabled_discovered_ids: Vec::new(),
            registry_base_url: McpSettings::default().registry_base_url,
        };

        let (clients, issues) = McpRunClients::connect(&settings).await;

        assert!(clients.clients.is_empty());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].server_id, "missing");
        assert_eq!(issues[0].stage, McpSetupStage::Preflight);
        assert_eq!(
            issues[0].error,
            McpError::CommandNotFound {
                command: "/definitely/not/a/real/openflow-mcp-server".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn enabled_untrusted_server_is_rejected_before_spawn() {
        let settings = McpSettings {
            servers: vec![stdio_server(
                "untrusted",
                "Untrusted",
                "/bin/sh",
                vec!["-c".to_string(), "exit 42".to_string()],
                Default::default(),
                true,
                false,
            )],
            discover_external: false,
            disabled_discovered_ids: Vec::new(),
            registry_base_url: McpSettings::default().registry_base_url,
        };

        let (clients, issues) = McpRunClients::connect(&settings).await;

        assert!(clients.clients.is_empty());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].stage, McpSetupStage::Trust);
        assert!(matches!(issues[0].error, McpError::Untrusted { .. }));
    }

    #[tokio::test]
    async fn stalled_server_startup_times_out() {
        let config = stdio_server(
            "stalled",
            "Stalled",
            "/bin/sh",
            vec!["-c".to_string(), "sleep 60".to_string()],
            Default::default(),
            true,
            true,
        );

        let error = McpStdioClient::spawn_with_timeout(&config, Duration::from_millis(25))
            .await
            .expect_err("stalled server should time out");

        assert!(matches!(
            error,
            McpError::StartupTimeout { ref server_id, .. } if server_id == "stalled"
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn enabled_servers_begin_startup_concurrently() {
        let dir = tempfile::tempdir().unwrap();
        let first_marker = dir.path().join("first-started");
        let second_marker = dir.path().join("second-started");
        let server = |id: &str, own: &std::path::Path, other: &std::path::Path| {
            stdio_server(
                id,
                id,
                "/bin/sh",
                vec![
                    "-c".to_string(),
                    "touch \"$OWN_MARKER\"; while [ ! -f \"$OTHER_MARKER\" ]; do sleep 0.01; done"
                        .to_string(),
                ],
                std::collections::BTreeMap::from([
                    ("OWN_MARKER".to_string(), own.to_string_lossy().into_owned()),
                    (
                        "OTHER_MARKER".to_string(),
                        other.to_string_lossy().into_owned(),
                    ),
                ]),
                true,
                true,
            )
        };
        let settings = McpSettings {
            servers: vec![
                server("first", &first_marker, &second_marker),
                server("second", &second_marker, &first_marker),
            ],
            discover_external: false,
            disabled_discovered_ids: Vec::new(),
            registry_base_url: McpSettings::default().registry_base_url,
        };

        let (clients, issues) =
            McpRunClients::connect_with_policy(&settings, test_runtime_policy()).await;

        assert!(clients.clients.is_empty());
        assert_eq!(issues.len(), 2);
        assert!(issues
            .iter()
            .all(|issue| !matches!(issue.error, McpError::StartupTimeout { .. })));
    }

    #[tokio::test]
    async fn sampling_callback_is_bound_to_originating_tool_call() {
        use rmcp::model::{CreateMessageResult, Role, SamplingMessage, SamplingMessageContent};
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
        let server = tokio::spawn(async move {
            let (server_read, mut server_write) = tokio::io::split(server_transport);
            let mut lines = BufReader::new(server_read).lines();
            let initialize: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .expect("read initialize")
                    .expect("initialize request"),
            )
            .expect("valid initialize request");
            assert!(initialize["params"]["capabilities"]["sampling"].is_object());
            let initialize_response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": initialize["id"],
                "result": {
                    "protocolVersion": initialize["params"]["protocolVersion"],
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "callback-test-server", "version": "1.0.0"}
                }
            });
            server_write
                .write_all(format!("{initialize_response}\n").as_bytes())
                .await
                .expect("write initialize response");

            let initialized: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .expect("read initialized")
                    .expect("initialized notification"),
            )
            .expect("valid initialized notification");
            assert_eq!(initialized["method"], "notifications/initialized");

            let tool_call: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .expect("read tool call")
                    .expect("tool call request"),
            )
            .expect("valid tool call request");
            let sampling_params = rmcp::model::CreateMessageRequestParams::new(
                vec![SamplingMessage::new(
                    Role::User,
                    SamplingMessageContent::text("Summarize this result"),
                )],
                64,
            );
            let callback = serde_json::json!({
                "jsonrpc": "2.0",
                "id": "sample-1",
                "method": "sampling/createMessage",
                "params": sampling_params,
            });
            server_write
                .write_all(format!("{callback}\n").as_bytes())
                .await
                .expect("write sampling callback");

            let callback_response: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .expect("read callback response")
                    .expect("sampling callback response"),
            )
            .expect("valid callback response");
            let tool_result = serde_json::json!({
                "jsonrpc": "2.0",
                "id": tool_call["id"],
                "result": {
                    "content": [{"type": "text", "text": "done"}],
                    "isError": false
                }
            });
            server_write
                .write_all(format!("{tool_result}\n").as_bytes())
                .await
                .expect("write tool result");
            callback_response
        });

        let policy = McpPolicy {
            allow_sampling: true,
            sampling_max_requests_per_run: 1,
            sampling_max_tokens_per_request: 64,
            sampling_max_total_tokens_per_run: 64,
            ..McpPolicy::default()
        };
        let (callback_tx, mut callback_rx) = tokio::sync::mpsc::unbounded_channel();
        let handler = client_handler::OpenFlowMcpClientHandler::for_run(
            "callback",
            policy.clone(),
            None,
            callback_tx,
        );
        let service = handler.clone().serve(client_transport).await.unwrap();
        let client = Arc::new(McpClient {
            service: RwLock::new(service),
            server_id: "callback".to_string(),
            transport: McpTransportKind::Stdio,
            policy: McpRuntimePolicy {
                call_tool_timeout: Duration::from_secs(1),
                ..test_runtime_policy()
            },
            tool_policy: policy,
            handler,
            callback_call_gate: Mutex::new(()),
        });
        let call_client = Arc::clone(&client);
        let call = tokio::spawn(async move {
            call_client
                .call_tool_with_origin(
                    "search",
                    serde_json::json!({}),
                    &CancellationToken::new(),
                    Some(McpRequestOrigin {
                        node_id: "research".into(),
                        tool_call_id: "tool-call-7".to_string(),
                        tool_name: "mcp_8_callback_search".to_string(),
                    }),
                )
                .await
        });

        let request = tokio::time::timeout(Duration::from_secs(1), callback_rx.recv())
            .await
            .expect("callback timeout")
            .expect("sampling callback");
        assert_eq!(
            request.pending.node_id,
            engine::NodeId("research".to_string())
        );
        assert_eq!(request.pending.tool_call_id, "tool-call-7");
        assert_eq!(request.pending.tool_name, "mcp_8_callback_search");
        assert!(matches!(
            &request.payload,
            McpRunClientRequestPayload::Sampling(params) if params.max_tokens == 64
        ));
        request
            .response_tx
            .send(Ok(McpRunClientResponse::Sampling(
                CreateMessageResult::new(
                    SamplingMessage::new(Role::Assistant, SamplingMessageContent::text("summary")),
                    "host-model".to_string(),
                ),
            )))
            .expect("send callback response");

        call.await.unwrap().expect("tool call result");
        let callback_response = server.await.unwrap();
        assert_eq!(callback_response["id"], "sample-1");
        assert_eq!(callback_response["result"]["model"], "host-model");
    }

    #[tokio::test]
    async fn tool_call_timeout_notifies_server_cancellation() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            let (server_read, mut server_write) = tokio::io::split(server_transport);
            let mut lines = BufReader::new(server_read).lines();
            let initialize: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("initialize request"),
            )
            .unwrap();
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": initialize["id"],
                "result": {
                    "protocolVersion": initialize["params"]["protocolVersion"],
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "slow-test-server", "version": "1.0.0"}
                }
            });
            server_write
                .write_all(format!("{response}\n").as_bytes())
                .await
                .unwrap();

            let initialized: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("initialized notification"),
            )
            .unwrap();
            assert_eq!(initialized["method"], "notifications/initialized");

            let tool_call: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().expect("tool call request"))
                    .unwrap();
            let cancelled: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("cancelled notification"),
            )
            .unwrap();
            (tool_call, cancelled)
        });
        let handler =
            client_handler::OpenFlowMcpClientHandler::disconnected("slow", McpPolicy::default());
        let service = handler.clone().serve(client_transport).await.unwrap();
        let client = McpStdioClient {
            service: RwLock::new(service),
            server_id: "slow".to_string(),
            transport: McpTransportKind::Stdio,
            policy: test_runtime_policy(),
            tool_policy: McpPolicy::default(),
            handler,
            callback_call_gate: Mutex::new(()),
        };
        let metadata = client.server_metadata().await.expect("initialize metadata");
        assert_eq!(metadata.server_name, "slow-test-server");
        assert_eq!(metadata.server_version, "1.0.0");
        assert_eq!(metadata.capabilities, ["tools"]);

        let error = client
            .call_tool("wait_forever", serde_json::json!({}))
            .await
            .expect_err("slow call should time out");
        let (tool_call, cancelled) = server.await.unwrap();

        assert_eq!(tool_call["method"], "tools/call");
        assert_eq!(cancelled["method"], "notifications/cancelled");
        assert_eq!(cancelled["params"]["requestId"], tool_call["id"]);
        assert!(matches!(
            error,
            McpError::CallToolTimeout {
                ref server_id,
                ref tool_name,
                ..
            } if server_id == "slow" && tool_name == "wait_forever"
        ));
    }

    #[tokio::test]
    async fn run_cancellation_notifies_server_for_in_flight_tool_call() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio_util::sync::CancellationToken;

        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            let (server_read, mut server_write) = tokio::io::split(server_transport);
            let mut lines = BufReader::new(server_read).lines();
            let initialize: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("initialize request"),
            )
            .unwrap();
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": initialize["id"],
                "result": {
                    "protocolVersion": initialize["params"]["protocolVersion"],
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "cancel-test-server", "version": "1.0.0"}
                }
            });
            server_write
                .write_all(format!("{response}\n").as_bytes())
                .await
                .unwrap();

            let initialized: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("initialized notification"),
            )
            .unwrap();
            assert_eq!(initialized["method"], "notifications/initialized");

            let tool_call: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().expect("tool call request"))
                    .unwrap();
            let cancelled: Value = serde_json::from_str(
                &lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("cancelled notification"),
            )
            .unwrap();
            (tool_call, cancelled)
        });
        let handler = client_handler::OpenFlowMcpClientHandler::disconnected(
            "cancelled",
            McpPolicy::default(),
        );
        let service = handler.clone().serve(client_transport).await.unwrap();
        let client = McpStdioClient {
            service: RwLock::new(service),
            server_id: "cancelled".to_string(),
            transport: McpTransportKind::Stdio,
            policy: test_runtime_policy(),
            tool_policy: McpPolicy::default(),
            handler,
            callback_call_gate: Mutex::new(()),
        };
        let cancel = CancellationToken::new();
        let cancel_after_send = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            cancel_after_send.cancel();
        });

        let error = client
            .call_tool_with_cancel("wait_forever", serde_json::json!({}), &cancel)
            .await
            .expect_err("run cancellation should stop the call");
        let (tool_call, cancelled) = server.await.unwrap();

        assert_eq!(tool_call["method"], "tools/call");
        assert_eq!(cancelled["method"], "notifications/cancelled");
        assert_eq!(cancelled["params"]["requestId"], tool_call["id"]);
        assert!(matches!(
            error,
            McpError::CallToolCancelled {
                ref server_id,
                ref tool_name,
            } if server_id == "cancelled" && tool_name == "wait_forever"
        ));
    }

    #[test]
    fn namespaced_tool_name_is_readable_and_reversible() {
        assert_eq!(
            namespaced_tool_name("gh", "search").unwrap(),
            "mcp_2_gh_search"
        );
        let encoded = namespaced_tool_name("bad/id", "search.items_with_underscores").unwrap();
        assert_eq!(
            parse_namespaced_tool_name(&encoded).unwrap(),
            (
                "bad/id".to_string(),
                "search.items_with_underscores".to_string()
            )
        );
    }

    #[test]
    fn namespaced_tool_name_is_provider_safe() {
        let name = namespaced_tool_name("massive", "search").expect("MCP tool name");
        assert!(
            name.chars().all(
                |character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            ),
            "provider-rejected MCP tool name: {name}"
        );
    }

    #[test]
    fn parse_namespaced_tool_name_splits_server_and_tool() {
        assert_eq!(
            parse_namespaced_tool_name("mcp/gh/search").unwrap(),
            ("gh".to_string(), "search".to_string())
        );
    }

    #[tokio::test]
    #[ignore = "requires STEP_MCP_LIVE=1"]
    async fn stdio_client_round_trips_tool_results() {
        if std::env::var("STEP_MCP_LIVE").ok().as_deref() != Some("1") {
            return;
        }
        let client = McpStdioClient::spawn(&stdio_server(
            "everything",
            "Everything",
            "npx",
            vec![
                "-y".into(),
                "@modelcontextprotocol/server-everything@2026.7.4".into(),
            ],
            Default::default(),
            true,
            true,
        ))
        .await
        .expect("spawn");
        let tools = client.list_tool_definitions().await.expect("list");
        assert!(tools
            .iter()
            .any(|tool| tool.name == "mcp_10_everything_echo"));

        let echo = client
            .call_tool(
                "echo",
                serde_json::json!({ "message": "OpenFlow live MCP" }),
            )
            .await
            .expect("call echo");
        assert_eq!(echo.content, "Echo: OpenFlow live MCP");
        assert!(!echo.is_error);

        let invalid = client
            .call_tool("echo", serde_json::json!({}))
            .await
            .expect("invalid args return a tool result");
        assert!(invalid.is_error);

        let image = client
            .call_tool("get-tiny-image", serde_json::json!({}))
            .await
            .expect("call image tool");
        let image: Value = serde_json::from_str(&image.content).expect("rich result JSON");
        assert_eq!(image["content"][1]["type"], "image");

        client.close().await.expect("close");
    }
}

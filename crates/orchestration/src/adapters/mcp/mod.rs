//! Stdio MCP client adapter — spawn servers, list tools, call tools.

mod discover;

pub use discover::{
    effective_mcp_servers, parse_mcp_servers_json, parse_mcp_servers_json_with_diagnostics,
    scan_external_mcp_for_api, McpParseDiagnostic, McpParseResult,
};

use crate::settings::model::{McpServerConfig, McpSettings};
use engine::{ToolConcurrency, ToolDefinition, ToolTier};
use rmcp::{
    model::{CallToolRequestParams, Tool as McpTool},
    service::RunningService,
    transport::TokioChildProcess,
    RoleClient, ServiceExt,
};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;
use tokio::sync::RwLock;

const MCP_PREFIX: &str = "mcp/";
const MCP_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum McpError {
    #[error("invalid MCP tool segment `{segment}`: must not contain '/'")]
    InvalidSegment { segment: String },
    #[error("invalid namespaced MCP tool name `{name}`")]
    InvalidNamespacedName { name: String },
    #[error("MCP server `{server_id}` is not connected")]
    ServerNotConnected { server_id: String },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSetupStage {
    Connect,
    ListTools,
}

impl std::fmt::Display for McpSetupStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect => f.write_str("connect"),
            Self::ListTools => f.write_str("list tools"),
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
    Ok(format!("{MCP_PREFIX}{server_id}/{tool_name}"))
}

pub fn parse_namespaced_tool_name(name: &str) -> Result<(&str, &str), McpError> {
    let rest = name
        .strip_prefix(MCP_PREFIX)
        .ok_or_else(|| McpError::InvalidNamespacedName {
            name: name.to_string(),
        })?;
    let (server_id, tool_name) =
        rest.split_once('/')
            .ok_or_else(|| McpError::InvalidNamespacedName {
                name: name.to_string(),
            })?;
    if server_id.is_empty() || tool_name.is_empty() {
        return Err(McpError::InvalidNamespacedName {
            name: name.to_string(),
        });
    }
    Ok((server_id, tool_name))
}

fn validate_segment(segment: &str) -> Result<(), McpError> {
    if segment.is_empty() || segment.contains('/') {
        return Err(McpError::InvalidSegment {
            segment: segment.to_string(),
        });
    }
    Ok(())
}

fn mcp_tool_to_definition(server_id: &str, tool: &McpTool) -> Result<ToolDefinition, McpError> {
    Ok(ToolDefinition {
        name: namespaced_tool_name(server_id, tool.name.as_ref())?,
        description: tool
            .description
            .as_ref()
            .map(|description| description.to_string())
            .unwrap_or_else(|| tool.name.to_string()),
        input_schema: serde_json::Value::Object(tool.input_schema.as_ref().clone()),
        tier: ToolTier::Write,
        concurrency: ToolConcurrency::Exclusive,
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

pub struct McpStdioClient {
    service: RwLock<RunningService<RoleClient, ()>>,
    server_id: String,
}

impl std::fmt::Debug for McpStdioClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpStdioClient")
            .field("server_id", &self.server_id)
            .finish_non_exhaustive()
    }
}

impl McpStdioClient {
    pub async fn spawn(config: &McpServerConfig) -> Result<Self, McpError> {
        let mut command = Command::new(&config.command);
        command.args(&config.args);
        for (key, value) in &config.env {
            command.env(key, value);
        }
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let service = ()
            .serve(
                TokioChildProcess::new(command)
                    .map_err(|error| McpError::Transport(error.to_string()))?,
            )
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;

        Ok(Self {
            service: RwLock::new(service),
            server_id: config.id.clone(),
        })
    }

    pub async fn list_tool_definitions(&self) -> Result<Vec<ToolDefinition>, McpError> {
        let service = self.service.read().await;
        let tools = service
            .list_all_tools()
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;
        tools
            .into_iter()
            .map(|tool| mcp_tool_to_definition(&self.server_id, &tool))
            .collect()
    }

    pub async fn list_tool_names(&self) -> Result<Vec<String>, McpError> {
        Ok(self
            .list_tool_definitions()
            .await?
            .into_iter()
            .map(|definition| definition.name)
            .collect())
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
    ) -> Result<McpToolOutcome, McpError> {
        let mut params = CallToolRequestParams::new(tool_name.to_string());
        if let Some(arguments) = args.as_object().cloned() {
            params = params.with_arguments(arguments);
        }
        let service = self.service.read().await;
        let result = service
            .call_tool(params)
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;
        project_tool_result(&result)
    }

    pub async fn close(&self) -> Result<(), McpError> {
        let close = async {
            let mut service = self.service.write().await;
            service.close().await
        };
        match tokio::time::timeout(MCP_SHUTDOWN_TIMEOUT, close).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(McpError::Transport(error.to_string())),
            Err(_) => Err(McpError::ShutdownTimeout {
                server_id: self.server_id.clone(),
                after_secs: MCP_SHUTDOWN_TIMEOUT.as_secs(),
            }),
        }
    }
}

pub struct McpRunClients {
    clients: HashMap<String, McpStdioClient>,
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
        let mut clients = HashMap::new();
        let mut issues = Vec::new();
        for config in settings.servers.iter().filter(|server| server.enabled) {
            let client = match McpStdioClient::spawn(config).await {
                Ok(client) => client,
                Err(error) => {
                    issues.push(McpSetupIssue {
                        server_id: config.id.clone(),
                        stage: McpSetupStage::Connect,
                        error,
                    });
                    continue;
                }
            };
            clients.insert(config.id.clone(), client);
        }
        (Self { clients }, issues)
    }

    pub async fn list_all_tool_definitions(&self) -> (Vec<ToolDefinition>, Vec<McpSetupIssue>) {
        let mut definitions = Vec::new();
        let mut issues = Vec::new();
        for (server_id, client) in &self.clients {
            match client.list_tool_definitions().await {
                Ok(server_definitions) => definitions.extend(server_definitions),
                Err(error) => issues.push(McpSetupIssue {
                    server_id: server_id.clone(),
                    stage: McpSetupStage::ListTools,
                    error,
                }),
            }
        }
        (definitions, issues)
    }

    pub async fn call_namespaced(
        &self,
        namespaced_name: &str,
        args: Value,
    ) -> Result<McpToolOutcome, McpError> {
        let (server_id, tool_name) = parse_namespaced_tool_name(namespaced_name)?;
        let client = self
            .clients
            .get(server_id)
            .ok_or_else(|| McpError::ServerNotConnected {
                server_id: server_id.to_string(),
            })?;
        client.call_tool(tool_name, args).await
    }

    pub async fn close(&self) -> Result<(), McpError> {
        let mut first_error = None;
        for client in self.clients.values() {
            if let Err(error) = client.close().await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let definition = mcp_tool_to_definition("filesystem", &tool).expect("definition");

        assert_eq!(definition.concurrency, ToolConcurrency::Exclusive);
    }

    #[tokio::test]
    async fn closing_empty_run_clients_is_idempotent() {
        let clients = McpRunClients {
            clients: HashMap::new(),
        };

        clients.close().await.unwrap();
        clients.close().await.unwrap();
    }

    #[tokio::test]
    async fn connection_failure_is_reported_without_rejecting_other_setup() {
        let settings = McpSettings {
            servers: vec![McpServerConfig {
                id: "missing".to_string(),
                display_name: "Missing".to_string(),
                command: "/definitely/not/a/real/openflow-mcp-server".to_string(),
                args: Vec::new(),
                env: Default::default(),
                enabled: true,
            }],
            discover_external: false,
            disabled_discovered_ids: Vec::new(),
        };

        let (clients, issues) = McpRunClients::connect(&settings).await;

        assert!(clients.clients.is_empty());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].server_id, "missing");
        assert_eq!(issues[0].stage, McpSetupStage::Connect);
    }

    #[test]
    fn namespaced_tool_name_rejects_slashes_in_segments() {
        assert_eq!(
            namespaced_tool_name("gh", "search").unwrap(),
            "mcp/gh/search"
        );
        assert!(namespaced_tool_name("bad/id", "search").is_err());
    }

    #[test]
    fn parse_namespaced_tool_name_splits_server_and_tool() {
        assert_eq!(
            parse_namespaced_tool_name("mcp/gh/search").unwrap(),
            ("gh", "search")
        );
    }

    #[tokio::test]
    #[ignore = "requires STEP_MCP_LIVE=1"]
    async fn stdio_client_round_trips_tool_results() {
        if std::env::var("STEP_MCP_LIVE").ok().as_deref() != Some("1") {
            return;
        }
        let client = McpStdioClient::spawn(&McpServerConfig {
            id: "everything".into(),
            display_name: "Everything".into(),
            command: "npx".into(),
            args: vec![
                "-y".into(),
                "@modelcontextprotocol/server-everything@2026.7.4".into(),
            ],
            env: Default::default(),
            enabled: true,
        })
        .await
        .expect("spawn");
        let tools = client.list_tool_definitions().await.expect("list");
        assert!(tools.iter().any(|tool| tool.name == "mcp/everything/echo"));

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

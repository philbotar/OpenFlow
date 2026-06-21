//! Stdio MCP client adapter — spawn servers, list tools, call tools.

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
use thiserror::Error;
use tokio::process::Command;

const MCP_PREFIX: &str = "mcp/";

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
        concurrency: ToolConcurrency::Shared,
    })
}

fn format_tool_result(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|text| text.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct McpStdioClient {
    service: RunningService<RoleClient, ()>,
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
            service,
            server_id: config.id.clone(),
        })
    }

    pub async fn list_tool_definitions(&self) -> Result<Vec<ToolDefinition>, McpError> {
        let tools = self
            .service
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

    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<String, McpError> {
        let mut params = CallToolRequestParams::new(tool_name.to_string());
        if let Some(arguments) = args.as_object().cloned() {
            params = params.with_arguments(arguments);
        }
        let result = self
            .service
            .call_tool(params)
            .await
            .map_err(|error| McpError::Transport(error.to_string()))?;
        Ok(format_tool_result(&result))
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
    pub async fn connect(settings: &McpSettings) -> Result<Self, McpError> {
        let mut clients = HashMap::new();
        for config in settings.servers.iter().filter(|server| server.enabled) {
            let client = McpStdioClient::spawn(config).await?;
            clients.insert(config.id.clone(), client);
        }
        Ok(Self { clients })
    }

    pub async fn list_all_tool_definitions(&self) -> Result<Vec<ToolDefinition>, McpError> {
        let mut definitions = Vec::new();
        for client in self.clients.values() {
            definitions.extend(client.list_tool_definitions().await?);
        }
        Ok(definitions)
    }

    pub async fn call_namespaced(
        &self,
        namespaced_name: &str,
        args: Value,
    ) -> Result<String, McpError> {
        let (server_id, tool_name) = parse_namespaced_tool_name(namespaced_name)?;
        let client = self
            .clients
            .get(server_id)
            .ok_or_else(|| McpError::ServerNotConnected {
                server_id: server_id.to_string(),
            })?;
        client.call_tool(tool_name, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn stdio_client_lists_tools() {
        if std::env::var("STEP_MCP_LIVE").ok().as_deref() != Some("1") {
            return;
        }
        let client = McpStdioClient::spawn(&McpServerConfig {
            id: "time".into(),
            display_name: "time".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-time".into()],
            env: Default::default(),
            enabled: true,
        })
        .await
        .expect("spawn");
        let tools = client.list_tool_definitions().await.expect("list");
        assert!(!tools.is_empty());
    }
}

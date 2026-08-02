use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const DEFAULT_MCP_REGISTRY_BASE_URL: &str = "https://registry.modelcontextprotocol.io/v0.1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpCatalogQuery {
    pub search: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogPage {
    pub catalog_base_url: String,
    pub catalog_label: String,
    pub servers: Vec<McpCatalogServer>,
    pub next_cursor: Option<String>,
    pub count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogServer {
    pub name: String,
    pub title: Option<String>,
    pub description: String,
    pub version: String,
    pub repository_url: Option<String>,
    pub website_url: Option<String>,
    pub is_latest: Option<bool>,
    pub packages: Vec<McpCatalogPackage>,
    pub remotes: Vec<McpCatalogRemote>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogPackage {
    pub registry_type: String,
    pub identifier: String,
    pub version: Option<String>,
    pub runtime_hint: Option<String>,
    pub transport_type: String,
    pub runtime_arguments: Vec<McpCatalogArgument>,
    pub package_arguments: Vec<McpCatalogArgument>,
    pub inputs: Vec<McpCatalogInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogRemote {
    pub transport_type: String,
    pub url: Option<String>,
    pub inputs: Vec<McpCatalogInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogArgument {
    pub argument_type: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub default: Option<String>,
    pub description: Option<String>,
    pub required: bool,
    pub secret: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub default: Option<String>,
    pub required: bool,
    pub secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpCatalogError {
    #[error("MCP Registry request failed: {0}")]
    Request(String),
}

#[async_trait]
pub trait McpCatalog: Send + Sync {
    async fn search(&self, query: &McpCatalogQuery) -> Result<McpCatalogPage, McpCatalogError>;

    async fn versions(&self, server_name: &str) -> Result<McpCatalogPage, McpCatalogError>;

    async fn exact_version(
        &self,
        server_name: &str,
        version: &str,
    ) -> Result<McpCatalogServer, McpCatalogError>;
}

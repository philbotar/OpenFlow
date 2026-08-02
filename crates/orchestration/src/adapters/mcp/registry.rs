use crate::mcp::catalog::{
    McpCatalog, McpCatalogArgument, McpCatalogError, McpCatalogInput, McpCatalogPackage,
    McpCatalogPage, McpCatalogQuery, McpCatalogRemote, McpCatalogServer,
};
use async_trait::async_trait;
use reqwest::Url;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;
use thiserror::Error;

pub const MCP_REGISTRY_DEFAULT_BASE_URL: &str = crate::mcp::catalog::DEFAULT_MCP_REGISTRY_BASE_URL;
pub const MCP_REGISTRY_PREVIEW_LABEL: &str = "Preview";

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Read-only client for a Registry v0.1-compatible API.
///
/// Registry metadata describes provenance and installation options. It does not establish that a
/// server, package, or publisher is safe.
#[derive(Debug, Clone)]
pub struct McpRegistryClient {
    http: reqwest::Client,
    base_url: String,
    request_timeout: Duration,
}

impl Default for McpRegistryClient {
    fn default() -> Self {
        Self::new(MCP_REGISTRY_DEFAULT_BASE_URL, DEFAULT_REQUEST_TIMEOUT)
            .expect("the built-in MCP Registry URL and timeout must be valid")
    }
}

impl McpRegistryClient {
    pub fn new(base_url: &str, request_timeout: Duration) -> Result<Self, McpRegistryError> {
        if request_timeout.is_zero() {
            return Err(McpRegistryError::InvalidRequestTimeout);
        }
        let base_url = base_url.trim().trim_end_matches('/');
        let parsed = Url::parse(base_url).map_err(|_| McpRegistryError::InvalidBaseUrl)?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(McpRegistryError::InvalidBaseUrl);
        }
        let http = reqwest::Client::builder()
            .timeout(request_timeout)
            .build()
            .map_err(|_| McpRegistryError::ClientBuildFailed)?;
        Ok(Self {
            http,
            base_url: base_url.to_string(),
            request_timeout,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn list_servers(
        &self,
        params: &RegistryListParams,
    ) -> Result<RegistryServerList, McpRegistryError> {
        let mut url = self.endpoint("/servers")?;
        if params.search.is_some() || params.cursor.is_some() || params.limit.is_some() {
            let mut query = url.query_pairs_mut();
            if let Some(search) = params.search.as_deref() {
                query.append_pair("search", search);
            }
            if let Some(cursor) = params.cursor.as_deref() {
                query.append_pair("cursor", cursor);
            }
            if let Some(limit) = params.limit {
                query.append_pair("limit", &limit.to_string());
            }
        }
        self.get_json(url).await
    }

    pub async fn list_server_versions(
        &self,
        server_name: &str,
    ) -> Result<RegistryServerList, McpRegistryError> {
        let server_name = required_path_segment("server name", server_name)?;
        let url = self.endpoint(&format!(
            "/servers/{}/versions",
            encode_path_segment(server_name)
        ))?;
        self.get_json(url).await
    }

    pub async fn get_server_version(
        &self,
        server_name: &str,
        version: &str,
    ) -> Result<RegistryServerResponse, McpRegistryError> {
        let server_name = required_path_segment("server name", server_name)?;
        let version = required_path_segment("version", version)?;
        if version == "latest" {
            return Err(McpRegistryError::VersionMustBeExact);
        }
        let url = self.endpoint(&format!(
            "/servers/{}/versions/{}",
            encode_path_segment(server_name),
            encode_path_segment(version)
        ))?;
        self.get_json(url).await
    }

    fn endpoint(&self, path: &str) -> Result<Url, McpRegistryError> {
        Url::parse(&format!("{}{path}", self.base_url))
            .map_err(|_| McpRegistryError::InvalidBaseUrl)
    }

    async fn get_json<T>(&self, url: Url) -> Result<T, McpRegistryError>
    where
        T: DeserializeOwned,
    {
        let request = async {
            let response = self
                .http
                .get(url)
                .send()
                .await
                .map_err(sanitize_reqwest_error)?;
            if !response.status().is_success() {
                return Err(McpRegistryError::HttpStatus {
                    status: response.status().as_u16(),
                });
            }
            response.json::<T>().await.map_err(sanitize_response_error)
        };
        tokio::time::timeout(self.request_timeout, request)
            .await
            .map_err(|_| McpRegistryError::RequestTimeout)?
    }
}

#[async_trait]
impl McpCatalog for McpRegistryClient {
    async fn search(&self, query: &McpCatalogQuery) -> Result<McpCatalogPage, McpCatalogError> {
        let response = self
            .list_servers(&RegistryListParams {
                search: query.search.clone(),
                cursor: query.cursor.clone(),
                limit: query.limit,
            })
            .await
            .map_err(catalog_error)?;
        Ok(self.catalog_page(response))
    }

    async fn versions(&self, server_name: &str) -> Result<McpCatalogPage, McpCatalogError> {
        let response = self
            .list_server_versions(server_name)
            .await
            .map_err(catalog_error)?;
        Ok(self.catalog_page(response))
    }

    async fn exact_version(
        &self,
        server_name: &str,
        version: &str,
    ) -> Result<McpCatalogServer, McpCatalogError> {
        self.get_server_version(server_name, version)
            .await
            .map(catalog_server)
            .map_err(catalog_error)
    }
}

impl McpRegistryClient {
    fn catalog_page(&self, response: RegistryServerList) -> McpCatalogPage {
        let next_cursor = response
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.next_cursor.clone());
        let count = response
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.count);
        McpCatalogPage {
            catalog_base_url: self.base_url.clone(),
            catalog_label: MCP_REGISTRY_PREVIEW_LABEL.to_string(),
            servers: response.servers.into_iter().map(catalog_server).collect(),
            next_cursor,
            count,
        }
    }
}

fn catalog_error(error: McpRegistryError) -> McpCatalogError {
    McpCatalogError::Request(error.to_string())
}

fn catalog_server(response: RegistryServerResponse) -> McpCatalogServer {
    let detail = response.server;
    McpCatalogServer {
        name: detail.name,
        title: detail.title,
        description: detail.description,
        version: detail.version,
        repository_url: detail.repository.map(|repository| repository.url),
        website_url: detail.website_url,
        is_latest: response
            .metadata
            .official
            .and_then(|metadata| metadata.is_latest),
        packages: detail.packages.into_iter().map(catalog_package).collect(),
        remotes: detail.remotes.into_iter().map(catalog_remote).collect(),
    }
}

fn catalog_package(package: RegistryPackage) -> McpCatalogPackage {
    let mut inputs = package
        .environment_variables
        .into_iter()
        .map(catalog_input)
        .collect::<Vec<_>>();
    inputs.extend(package.transport.headers.into_iter().map(catalog_input));
    inputs.extend(package.transport.variables.into_values().map(catalog_input));
    McpCatalogPackage {
        registry_type: package.registry_type,
        identifier: package.identifier,
        version: package.version,
        runtime_hint: package.runtime_hint,
        transport_type: package.transport.transport_type,
        runtime_arguments: package
            .runtime_arguments
            .into_iter()
            .map(catalog_argument)
            .collect(),
        package_arguments: package
            .package_arguments
            .into_iter()
            .map(catalog_argument)
            .collect(),
        inputs,
    }
}

fn catalog_remote(remote: RegistryRemote) -> McpCatalogRemote {
    let mut inputs = remote
        .headers
        .into_iter()
        .map(catalog_input)
        .collect::<Vec<_>>();
    inputs.extend(remote.variables.into_values().map(catalog_input));
    McpCatalogRemote {
        transport_type: remote.transport_type,
        url: remote.url,
        inputs,
    }
}

fn catalog_argument(argument: RegistryArgument) -> McpCatalogArgument {
    McpCatalogArgument {
        argument_type: argument.argument_type,
        name: argument.name,
        value: argument.value,
        default: argument.default,
        description: argument.description,
        required: argument.is_required.unwrap_or(false),
        secret: argument.is_secret.unwrap_or(false),
    }
}

fn catalog_input(input: RegistryInput) -> McpCatalogInput {
    McpCatalogInput {
        name: input.name,
        description: input.description,
        default: input.default,
        required: input.is_required.unwrap_or(false),
        secret: input.is_secret.unwrap_or(false),
    }
}

fn required_path_segment<'a>(
    parameter: &'static str,
    value: &'a str,
) -> Result<&'a str, McpRegistryError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(McpRegistryError::MissingPathParameter { parameter });
    }
    Ok(value)
}

fn encode_path_segment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

fn sanitize_reqwest_error(error: reqwest::Error) -> McpRegistryError {
    if error.is_timeout() {
        McpRegistryError::RequestTimeout
    } else {
        McpRegistryError::RequestFailed
    }
}

fn sanitize_response_error(error: reqwest::Error) -> McpRegistryError {
    if error.is_timeout() {
        McpRegistryError::RequestTimeout
    } else {
        McpRegistryError::InvalidResponse
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum McpRegistryError {
    #[error("invalid MCP Registry base URL")]
    InvalidBaseUrl,
    #[error("MCP Registry request timeout must be greater than zero")]
    InvalidRequestTimeout,
    #[error("failed to build MCP Registry HTTP client")]
    ClientBuildFailed,
    #[error("missing MCP Registry path parameter `{parameter}`")]
    MissingPathParameter { parameter: &'static str },
    #[error("MCP Registry version must be exact; `latest` is not accepted")]
    VersionMustBeExact,
    #[error("MCP Registry request timed out")]
    RequestTimeout,
    #[error("MCP Registry request failed")]
    RequestFailed,
    #[error("MCP Registry returned HTTP {status}")]
    HttpStatus { status: u16 },
    #[error("MCP Registry returned an invalid response")]
    InvalidResponse,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistryListParams {
    pub search: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RegistryServerList {
    pub servers: Vec<RegistryServerResponse>,
    #[serde(default)]
    pub metadata: Option<RegistryPageMetadata>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPageMetadata {
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub count: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RegistryServerResponse {
    pub server: RegistryServerDetail,
    #[serde(rename = "_meta", default)]
    pub metadata: RegistryResponseMetadata,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct RegistryResponseMetadata {
    #[serde(
        rename = "io.modelcontextprotocol.registry/official",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub official: Option<RegistryOfficialMetadata>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryOfficialMetadata {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub status_message: Option<String>,
    #[serde(default)]
    pub status_changed_at: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub is_latest: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryServerDetail {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub repository: Option<RegistryRepository>,
    pub version: String,
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub icons: Vec<RegistryIcon>,
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub packages: Vec<RegistryPackage>,
    #[serde(default)]
    pub remotes: Vec<RegistryRemote>,
    #[serde(rename = "_meta", default)]
    pub metadata: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryRepository {
    pub url: String,
    pub source: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub subfolder: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryIcon {
    pub src: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub sizes: Vec<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPackage {
    pub registry_type: String,
    #[serde(default)]
    pub registry_base_url: Option<String>,
    pub identifier: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub file_sha256: Option<String>,
    #[serde(default)]
    pub runtime_hint: Option<String>,
    pub transport: RegistryTransport,
    #[serde(default)]
    pub runtime_arguments: Vec<RegistryArgument>,
    #[serde(default)]
    pub package_arguments: Vec<RegistryArgument>,
    #[serde(default)]
    pub environment_variables: Vec<RegistryInput>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryTransport {
    #[serde(rename = "type")]
    pub transport_type: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Vec<RegistryInput>,
    #[serde(default)]
    pub variables: BTreeMap<String, RegistryInput>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub type RegistryRemote = RegistryTransport;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryArgument {
    #[serde(rename = "type")]
    pub argument_type: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value_hint: Option<String>,
    #[serde(default)]
    pub is_repeated: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_required: Option<bool>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub is_secret: Option<bool>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, RegistryInput>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_required: Option<bool>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub is_secret: Option<bool>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, RegistryInput>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    struct MockReply {
        status: u16,
        body: &'static str,
        headers: &'static [(&'static str, &'static str)],
        delay: Duration,
    }

    impl MockReply {
        fn json(body: &'static str) -> Self {
            Self {
                status: 200,
                body,
                headers: &[],
                delay: Duration::ZERO,
            }
        }
    }

    async fn spawn_mock(replies: Vec<MockReply>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut targets = Vec::with_capacity(replies.len());
            for reply in replies {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut chunk = [0_u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut chunk).await.unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..read]);
                }
                let request = String::from_utf8(request).unwrap();
                let target = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap()
                    .to_string();
                targets.push(target);

                tokio::time::sleep(reply.delay).await;
                let reason = if reply.status == 200 {
                    "OK"
                } else {
                    "Internal Server Error"
                };
                let mut response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                    reply.status,
                    reason,
                    reply.body.len()
                );
                for (name, value) in reply.headers {
                    response.push_str(name);
                    response.push_str(": ");
                    response.push_str(value);
                    response.push_str("\r\n");
                }
                response.push_str("\r\n");
                response.push_str(reply.body);
                let _ = stream.write_all(response.as_bytes()).await;
            }
            targets
        });
        (format!("http://{address}/v0.1"), task)
    }

    #[test]
    fn registry_defaults_are_explicitly_preview() {
        assert_eq!(
            MCP_REGISTRY_DEFAULT_BASE_URL,
            "https://registry.modelcontextprotocol.io/v0.1"
        );
        assert_eq!(MCP_REGISTRY_PREVIEW_LABEL, "Preview");
        assert_eq!(
            McpRegistryClient::default().base_url(),
            MCP_REGISTRY_DEFAULT_BASE_URL
        );
    }

    #[tokio::test]
    async fn mcp_registry_search_encodes_query_cursor_and_retains_unknown_fields() {
        let body = r#"{
            "servers": [{
                "server": {
                    "name": "io.example/files",
                    "description": "File tools",
                    "version": "1.2.3",
                    "packages": [{
                        "registryType": "npm",
                        "identifier": "@example/files",
                        "version": "1.2.3",
                        "transport": {"type": "stdio", "futureTransport": true},
                        "futurePackage": 7
                    }],
                    "futureServer": {"kept": true}
                },
                "_meta": {
                    "io.modelcontextprotocol.registry/official": {
                        "status": "active",
                        "isLatest": true
                    },
                    "futureRegistry": "kept"
                },
                "futureEnvelope": "kept"
            }],
            "metadata": {
                "nextCursor": "opaque+/=",
                "count": 1,
                "futurePage": true
            },
            "futureList": "kept"
        }"#;
        let (base_url, requests) =
            spawn_mock(vec![MockReply::json(body), MockReply::json(body)]).await;
        let client = McpRegistryClient::new(&base_url, Duration::from_secs(1)).unwrap();

        let page = client
            .list_servers(&RegistryListParams {
                search: Some("file system/日本".to_string()),
                cursor: Some("opaque+/=".to_string()),
                limit: Some(25),
            })
            .await
            .unwrap();
        let catalog_page = client
            .search(&McpCatalogQuery {
                search: Some("file system/日本".to_string()),
                cursor: Some("opaque+/=".to_string()),
                limit: Some(25),
            })
            .await
            .unwrap();

        let targets = requests.await.unwrap();
        assert_eq!(
            targets,
            vec![
                "/v0.1/servers?search=file+system%2F%E6%97%A5%E6%9C%AC&cursor=opaque%2B%2F%3D&limit=25",
                "/v0.1/servers?search=file+system%2F%E6%97%A5%E6%9C%AC&cursor=opaque%2B%2F%3D&limit=25",
            ]
        );
        assert_eq!(catalog_page.catalog_label, MCP_REGISTRY_PREVIEW_LABEL);
        assert_eq!(catalog_page.next_cursor.as_deref(), Some("opaque+/="));
        assert_eq!(catalog_page.servers[0].name, "io.example/files");
        assert_eq!(catalog_page.servers[0].packages[0].registry_type, "npm");
        assert_eq!(
            page.metadata.as_ref().unwrap().next_cursor.as_deref(),
            Some("opaque+/=")
        );
        assert_eq!(page.extra["futureList"], serde_json::json!("kept"));
        assert_eq!(
            page.metadata.as_ref().unwrap().extra["futurePage"],
            serde_json::json!(true)
        );
        assert_eq!(
            page.servers[0].server.extra["futureServer"],
            serde_json::json!({"kept": true})
        );
        assert_eq!(
            page.servers[0].server.packages[0].extra["futurePackage"],
            serde_json::json!(7)
        );
        assert_eq!(
            page.servers[0].server.packages[0].transport.extra["futureTransport"],
            serde_json::json!(true)
        );
        assert_eq!(
            page.servers[0].extra["futureEnvelope"],
            serde_json::json!("kept")
        );
        assert_eq!(
            page.servers[0].metadata.extra["futureRegistry"],
            serde_json::json!("kept")
        );
    }

    #[tokio::test]
    async fn mcp_registry_lists_versions_and_fetches_exact_version() {
        let versions = r#"{
            "servers": [
                {"server": {"name": "io.example/files", "description": "Files", "version": "2.0.0"}},
                {"server": {"name": "io.example/files", "description": "Files", "version": "1.0.0"}}
            ],
            "metadata": {"count": 2}
        }"#;
        let detail = r#"{
            "server": {
                "name": "io.example/files",
                "description": "Files",
                "version": "1.0.0+build.7"
            },
            "_meta": {"io.modelcontextprotocol.registry/official": {"status": "active"}}
        }"#;
        let (base_url, requests) =
            spawn_mock(vec![MockReply::json(versions), MockReply::json(detail)]).await;
        let client = McpRegistryClient::new(&base_url, Duration::from_secs(1)).unwrap();

        let page = client
            .list_server_versions("io.example/files")
            .await
            .unwrap();
        let server = client
            .get_server_version("io.example/files", "1.0.0+build.7")
            .await
            .unwrap();

        assert_eq!(page.metadata.unwrap().count, Some(2));
        assert_eq!(server.server.version, "1.0.0+build.7");
        assert_eq!(
            requests.await.unwrap(),
            vec![
                "/v0.1/servers/io.example%2Ffiles/versions",
                "/v0.1/servers/io.example%2Ffiles/versions/1.0.0%2Bbuild.7"
            ]
        );
    }

    #[tokio::test]
    async fn mcp_registry_non_success_error_omits_body_headers_and_url() {
        let secret = "registry-secret-body-token";
        let (base_url, requests) = spawn_mock(vec![MockReply {
            status: 500,
            body: "registry-secret-body-token",
            headers: &[("X-Registry-Secret", "registry-secret-header-token")],
            delay: Duration::ZERO,
        }])
        .await;
        let client = McpRegistryClient::new(&base_url, Duration::from_secs(1)).unwrap();

        let error = client
            .list_servers(&RegistryListParams::default())
            .await
            .unwrap_err();
        let message = error.to_string();

        assert_eq!(error, McpRegistryError::HttpStatus { status: 500 });
        assert!(!message.contains(secret));
        assert!(!message.contains("registry-secret-header-token"));
        assert!(!message.contains(&base_url));
        assert_eq!(requests.await.unwrap(), vec!["/v0.1/servers"]);
    }

    #[tokio::test]
    async fn mcp_registry_request_timeout_is_bounded() {
        let (base_url, requests) = spawn_mock(vec![MockReply {
            status: 200,
            body: r#"{"servers": []}"#,
            headers: &[],
            delay: Duration::from_millis(200),
        }])
        .await;
        let client = McpRegistryClient::new(&base_url, Duration::from_millis(20)).unwrap();

        let error = client
            .list_servers(&RegistryListParams::default())
            .await
            .unwrap_err();

        assert_eq!(error, McpRegistryError::RequestTimeout);
        assert_eq!(requests.await.unwrap(), vec!["/v0.1/servers"]);
    }
}

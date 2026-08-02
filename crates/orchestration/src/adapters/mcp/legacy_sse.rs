use super::http::resolved_headers_with_oauth;
use super::http_security::secure_http_endpoint;
use super::McpError;
use crate::mcp::model::McpConnection;
use crate::settings::model::McpServerConfig;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::header::{HeaderMap, ACCEPT, CONTENT_TYPE};
use reqwest::{Client, Url};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::RoleClient;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct LegacySseTransport {
    client: Client,
    post_url: Url,
    headers: HeaderMap,
    inbound: mpsc::Receiver<ServerJsonRpcMessage>,
    cancel: CancellationToken,
    stream_task: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacySseError {
    #[error("legacy MCP SSE connection failed")]
    Connect,
    #[error("legacy MCP SSE response was invalid")]
    InvalidResponse,
    #[error("legacy MCP SSE endpoint event was missing or invalid")]
    MissingEndpoint,
    #[error("legacy MCP SSE message send failed")]
    Send,
}

pub async fn legacy_sse_transport(
    config: &McpServerConfig,
) -> Result<LegacySseTransport, McpError> {
    let McpConnection::LegacySse {
        url,
        allow_localhost,
        headers,
        auth,
    } = &config.connection
    else {
        return Err(McpError::UnsupportedTransport {
            server_id: config.id.clone(),
            transport: config.connection.transport_kind(),
        });
    };
    let secured = secure_http_endpoint(url, *allow_localhost)
        .await
        .map_err(|error| McpError::HttpSecurity {
            server_id: config.id.clone(),
            reason: error.to_string(),
        })?;
    let headers = resolved_headers_with_oauth(config, headers, auth).await?;
    connect_legacy_sse(secured.client, secured.url, headers, *allow_localhost)
        .await
        .map_err(|_| McpError::RemoteTransport {
            server_id: config.id.clone(),
            operation: "legacy SSE connect",
        })
}

pub async fn legacy_sse_transport_from_streamable(
    config: &McpServerConfig,
) -> Result<LegacySseTransport, McpError> {
    let McpConnection::StreamableHttp {
        url,
        allow_localhost,
        headers,
        auth,
    } = &config.connection
    else {
        return Err(McpError::UnsupportedTransport {
            server_id: config.id.clone(),
            transport: config.connection.transport_kind(),
        });
    };
    let secured = secure_http_endpoint(url, *allow_localhost)
        .await
        .map_err(|error| McpError::HttpSecurity {
            server_id: config.id.clone(),
            reason: error.to_string(),
        })?;
    let headers = resolved_headers_with_oauth(config, headers, auth).await?;
    connect_legacy_sse(secured.client, secured.url, headers, *allow_localhost)
        .await
        .map_err(|_| McpError::RemoteTransport {
            server_id: config.id.clone(),
            operation: "legacy SSE fallback connect",
        })
}

async fn connect_legacy_sse(
    client: Client,
    base_url: Url,
    headers: HeaderMap,
    allow_localhost: bool,
) -> Result<LegacySseTransport, LegacySseError> {
    let response = client
        .get(base_url.clone())
        .headers(headers.clone())
        .header(ACCEPT, "text/event-stream")
        .send()
        .await
        .map_err(|_| LegacySseError::Connect)?;
    if !response.status().is_success()
        || !response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream"))
    {
        return Err(LegacySseError::InvalidResponse);
    }
    let mut events = response.bytes_stream().eventsource();
    let endpoint = tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(event) = events.next().await {
            let event = event.map_err(|_| LegacySseError::InvalidResponse)?;
            if event.event == "endpoint" {
                return Ok(event.data);
            }
        }
        Err(LegacySseError::MissingEndpoint)
    })
    .await
    .map_err(|_| LegacySseError::MissingEndpoint)??;
    let post_url = same_origin_endpoint(&base_url, &endpoint, allow_localhost)?;
    let (sender, inbound) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let stream_cancel = cancel.clone();
    let stream_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                () = stream_cancel.cancelled() => break,
                event = events.next() => {
                    let Some(Ok(event)) = event else { break; };
                    if event.event != "message" && !event.event.is_empty() {
                        continue;
                    }
                    let Ok(message) = serde_json::from_str::<ServerJsonRpcMessage>(&event.data) else {
                        break;
                    };
                    if sender.send(message).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    Ok(LegacySseTransport {
        client,
        post_url,
        headers,
        inbound,
        cancel,
        stream_task: Some(stream_task),
    })
}

fn same_origin_endpoint(
    base: &Url,
    endpoint: &str,
    allow_localhost: bool,
) -> Result<Url, LegacySseError> {
    let endpoint = base
        .join(endpoint)
        .map_err(|_| LegacySseError::MissingEndpoint)?;
    super::http_security::validate_endpoint_url(endpoint.as_str(), allow_localhost)
        .map_err(|_| LegacySseError::MissingEndpoint)?;
    if endpoint.scheme() != base.scheme()
        || endpoint.host_str() != base.host_str()
        || endpoint.port_or_known_default() != base.port_or_known_default()
    {
        return Err(LegacySseError::MissingEndpoint);
    }
    Ok(endpoint)
}

impl Transport<RoleClient> for LegacySseTransport {
    type Error = LegacySseError;

    fn send(
        &mut self,
        item: ClientJsonRpcMessage,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        let client = self.client.clone();
        let post_url = self.post_url.clone();
        let headers = self.headers.clone();
        async move {
            let response = client
                .post(post_url)
                .headers(headers)
                .json(&item)
                .send()
                .await
                .map_err(|_| LegacySseError::Send)?;
            if response.status().is_success() {
                Ok(())
            } else {
                Err(LegacySseError::Send)
            }
        }
    }

    async fn receive(&mut self) -> Option<ServerJsonRpcMessage> {
        self.inbound.recv().await
    }

    fn close(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        self.cancel.cancel();
        let task = self.stream_task.take();
        async move {
            if let Some(task) = task {
                let _ = task.await;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::model::{
        McpAuth, McpInstall, McpServerRecord, McpServerSource, PersistedValue,
    };
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::broadcast;

    #[derive(Debug, Clone)]
    struct RequestRecord {
        method: String,
        headers: BTreeMap<String, String>,
        body: String,
    }

    #[test]
    fn legacy_sse_message_endpoint_cannot_change_origin() {
        let base = Url::parse("https://mcp.example.test/sse").unwrap();
        assert_eq!(
            same_origin_endpoint(&base, "https://internal.example.test/messages", false),
            Err(LegacySseError::MissingEndpoint)
        );
        assert_eq!(
            same_origin_endpoint(&base, "/messages?id=abc", false)
                .unwrap()
                .as_str(),
            "https://mcp.example.test/messages?id=abc"
        );
    }

    #[tokio::test]
    async fn streamable_404_falls_back_to_legacy_sse_with_static_headers() {
        let (url, requests, stop, fixture) = spawn_legacy_fixture().await;
        let server = McpServerRecord::new(
            "legacy-fallback",
            "Legacy fallback",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::StreamableHttp {
                url,
                allow_localhost: true,
                headers: BTreeMap::from([(
                    "X-Api-Key".to_string(),
                    PersistedValue::Secret {
                        secret_ref: "mcp-secret:v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_string(),
                        resolved_value: Some("static-secret".to_string()),
                    },
                )]),
                auth: McpAuth::None,
            },
        );

        let client = super::super::McpClient::spawn(&server).await.unwrap();
        assert_eq!(
            client.list_tool_names().await.unwrap(),
            vec!["mcp_15_legacy-fallback_legacy__echo"]
        );
        client.close().await.unwrap();
        stop.cancel();
        fixture.await.unwrap();

        let requests = requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.body.contains("\"method\":\"initialize\""))
                .count(),
            2,
            "one failed Streamable HTTP initialize plus one legacy SSE initialize"
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.method == "GET")
                .count(),
            1
        );
        assert!(requests.iter().all(|request| {
            request.headers.get("x-api-key").map(String::as_str) == Some("static-secret")
        }));
    }

    async fn spawn_legacy_fixture() -> (
        String,
        Arc<Mutex<Vec<RequestRecord>>>,
        CancellationToken,
        JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let legacy_started = Arc::new(AtomicBool::new(false));
        let (messages, _) = broadcast::channel::<String>(16);
        let stop = CancellationToken::new();
        let task_stop = stop.clone();
        let task_requests = requests.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                let accepted = tokio::select! {
                    biased;
                    () = task_stop.cancelled() => break,
                    accepted = listener.accept() => accepted,
                };
                let Ok((socket, _)) = accepted else { break };
                let request_log = task_requests.clone();
                let started = legacy_started.clone();
                let sender = messages.clone();
                let connection_stop = task_stop.clone();
                connections.spawn(async move {
                    handle_legacy_connection(socket, request_log, started, sender, connection_stop)
                        .await;
                });
            }
            while connections.join_next().await.is_some() {}
        });
        (format!("http://{address}/mcp"), requests, stop, task)
    }

    async fn handle_legacy_connection(
        mut socket: TcpStream,
        requests: Arc<Mutex<Vec<RequestRecord>>>,
        legacy_started: Arc<AtomicBool>,
        messages: broadcast::Sender<String>,
        stop: CancellationToken,
    ) {
        let Some(request) = read_request(&mut socket).await else {
            return;
        };
        requests.lock().unwrap().push(request.clone());
        if request.method == "GET" {
            legacy_started.store(true, Ordering::SeqCst);
            let mut receiver = messages.subscribe();
            if socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\nevent: endpoint\ndata: /messages\r\n\r\n",
                )
                .await
                .is_err()
            {
                return;
            }
            loop {
                tokio::select! {
                    biased;
                    () = stop.cancelled() => break,
                    message = receiver.recv() => {
                        let Ok(message) = message else { break };
                        let event = format!("event: message\ndata: {message}\r\n\r\n");
                        if socket.write_all(event.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                }
            }
            return;
        }
        if !legacy_started.load(Ordering::SeqCst) {
            let _ = socket
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            return;
        }

        let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
        let method = body.get("method").and_then(serde_json::Value::as_str);
        if let Some(id) = body.get("id") {
            let result = match method {
                Some("initialize") => serde_json::json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "legacy-fixture", "version": "1.0.0"}
                }),
                Some("tools/list") => serde_json::json!({
                    "tools": [{
                        "name": "legacy_echo",
                        "description": "Fixture tool",
                        "inputSchema": {"type": "object"}
                    }]
                }),
                _ => serde_json::json!({}),
            };
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            });
            let _ = messages.send(serde_json::to_string(&response).unwrap());
        }
        let _ = socket
            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
    }

    async fn read_request(socket: &mut TcpStream) -> Option<RequestRecord> {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        let header_end = loop {
            let read = socket.read(&mut buffer).await.ok()?;
            if read == 0 {
                return None;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
            if bytes.len() > 64 * 1024 {
                return None;
            }
        };
        let header_text = String::from_utf8_lossy(&bytes[..header_end]);
        let mut lines = header_text.lines();
        let method = lines.next()?.split_whitespace().next()?.to_string();
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
            .collect::<BTreeMap<_, _>>();
        let content_length = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        while bytes.len() < header_end + content_length {
            let read = socket.read(&mut buffer).await.ok()?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        Some(RequestRecord {
            method,
            headers,
            body: String::from_utf8_lossy(
                &bytes[header_end..bytes.len().min(header_end + content_length)],
            )
            .to_string(),
        })
    }
}

use super::http_security::secure_http_endpoint;
use super::oauth_http::McpOAuthHttpAdapter;
use super::oauth_store::SystemOAuthStore;
use super::McpError;
use crate::mcp::model::{McpAuth, McpConnection, PersistedValue};
use crate::mcp::ports::{McpOAuthFlowPort, McpOAuthSecureStore};
use crate::settings::model::McpServerConfig;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rmcp::transport::common::client_side_sse::{ExponentialBackoff, SseRetryPolicy};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const SSE_RECONNECT_MAX_ATTEMPTS: usize = 3;
const SSE_RECONNECT_BASE_DELAY: Duration = Duration::from_millis(250);

pub async fn streamable_http_transport(
    config: &McpServerConfig,
) -> Result<StreamableHttpClientTransport<reqwest::Client>, McpError> {
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
    let custom_headers = resolved_headers_with_oauth(config, headers, auth).await?;
    let mut transport_config =
        StreamableHttpClientTransportConfig::with_uri(secured.url.to_string())
            .custom_headers(
                custom_headers
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect::<HashMap<_, _>>(),
            )
            .reinit_on_expired_session(false);
    transport_config.retry_config = bounded_sse_retry_policy();
    Ok(StreamableHttpClientTransport::with_client(
        secured.client,
        transport_config,
    ))
}

pub(crate) async fn resolved_headers_with_oauth(
    config: &McpServerConfig,
    headers: &std::collections::BTreeMap<String, PersistedValue>,
    auth: &McpAuth,
) -> Result<HeaderMap, McpError> {
    let store = SystemOAuthStore::default();
    resolved_headers_with_oauth_ports(config, headers, auth, &store, &McpOAuthHttpAdapter).await
}

async fn resolved_headers_with_oauth_ports(
    config: &McpServerConfig,
    headers: &std::collections::BTreeMap<String, PersistedValue>,
    auth: &McpAuth,
    store: &dyn McpOAuthSecureStore,
    flow: &dyn McpOAuthFlowPort,
) -> Result<HeaderMap, McpError> {
    let mut resolved = resolved_headers(&config.id, headers, auth)?;
    let McpAuth::OAuth {
        client_id,
        issuer,
        credential_ref,
        ..
    } = auth
    else {
        return Ok(resolved);
    };
    let credential_ref = credential_ref
        .as_deref()
        .ok_or_else(|| McpError::OAuthRequired {
            server_id: config.id.clone(),
        })?;
    let mut credentials = store
        .load_credentials(credential_ref)
        .await
        .map_err(|_| McpError::OAuthRequired {
            server_id: config.id.clone(),
        })?
        .ok_or_else(|| McpError::OAuthRequired {
            server_id: config.id.clone(),
        })?;
    let connection_url = match &config.connection {
        McpConnection::StreamableHttp { url, .. } | McpConnection::LegacySse { url, .. } => url,
        McpConnection::Stdio { .. } => {
            return Err(McpError::OAuthRequired {
                server_id: config.id.clone(),
            })
        }
    };
    let identity_matches = credentials.client_id == *client_id
        && issuer
            .as_deref()
            .is_none_or(|issuer| urls_equal(issuer, &credentials.issuer))
        && urls_equal(connection_url, &credentials.resource);
    if !identity_matches {
        return Err(McpError::OAuthRequired {
            server_id: config.id.clone(),
        });
    }
    if credentials.requires_refresh(chrono::Utc::now()) {
        credentials = flow
            .refresh(&credentials)
            .await
            .map_err(|_| McpError::OAuthRequired {
                server_id: config.id.clone(),
            })?;
        store
            .save_credentials(credential_ref, &credentials)
            .await
            .map_err(|_| McpError::OAuthRequired {
                server_id: config.id.clone(),
            })?;
    }
    insert_header(
        &config.id,
        &mut resolved,
        "Authorization",
        &format!("Bearer {}", credentials.access_token),
    )?;
    Ok(resolved)
}

fn normalized_url(value: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(value).ok()?;
    url.set_fragment(None);
    if url.path() == "/" && url.query().is_none() {
        url.set_path("");
    }
    Some(url.to_string())
}

fn urls_equal(left: &str, right: &str) -> bool {
    normalized_url(left)
        .zip(normalized_url(right))
        .is_some_and(|(left, right)| left == right)
}

fn bounded_sse_retry_policy() -> Arc<dyn SseRetryPolicy> {
    let mut retry = ExponentialBackoff::default();
    retry.max_times = Some(SSE_RECONNECT_MAX_ATTEMPTS);
    retry.base_duration = SSE_RECONNECT_BASE_DELAY;
    Arc::new(retry)
}

pub(crate) fn resolved_headers(
    server_id: &str,
    headers: &std::collections::BTreeMap<String, PersistedValue>,
    auth: &McpAuth,
) -> Result<HeaderMap, McpError> {
    let mut resolved = HeaderMap::new();
    for (name, value) in headers {
        insert_header(
            server_id,
            &mut resolved,
            name,
            persisted_value(server_id, value)?,
        )?;
    }
    match auth {
        McpAuth::None => {}
        McpAuth::Static {
            header_name,
            scheme,
            secret_ref,
            resolved_value,
        } => {
            let value = resolved_value
                .as_deref()
                .ok_or_else(|| McpError::SecretUnavailable {
                    server_id: server_id.to_string(),
                    secret_ref: secret_ref.clone(),
                })?;
            let value = scheme
                .as_deref()
                .map_or_else(|| value.to_string(), |scheme| format!("{scheme} {value}"));
            insert_header(server_id, &mut resolved, header_name, &value)?;
        }
        McpAuth::OAuth { .. } => {}
    }
    Ok(resolved)
}

fn persisted_value<'a>(server_id: &str, value: &'a PersistedValue) -> Result<&'a str, McpError> {
    value
        .runtime_value()
        .ok_or_else(|| McpError::SecretUnavailable {
            server_id: server_id.to_string(),
            secret_ref: match value {
                PersistedValue::Secret { secret_ref, .. } => secret_ref.clone(),
                PersistedValue::Literal { .. } => "unknown".to_string(),
            },
        })
}

fn insert_header(
    server_id: &str,
    headers: &mut HeaderMap,
    name: &str,
    value: &str,
) -> Result<(), McpError> {
    let name =
        HeaderName::from_bytes(name.as_bytes()).map_err(|_| McpError::InvalidHttpHeader {
            server_id: server_id.to_string(),
        })?;
    let value = HeaderValue::from_str(value).map_err(|_| McpError::InvalidHttpHeader {
        server_id: server_id.to_string(),
    })?;
    headers.insert(name, value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::model::{
        McpConnection, McpInstall, McpServerRecord, McpServerSource, PersistedValue,
    };
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio_util::sync::CancellationToken;

    #[derive(Debug, Clone)]
    struct HttpRequestRecord {
        method: String,
        headers: BTreeMap<String, String>,
        body: String,
    }

    #[derive(Default)]
    struct MemoryOAuthStore {
        credentials: tokio::sync::Mutex<Option<crate::mcp::oauth::McpOAuthCredentials>>,
    }

    #[async_trait::async_trait]
    impl crate::mcp::ports::McpOAuthSecureStore for MemoryOAuthStore {
        async fn load_credentials(
            &self,
            _credential_ref: &str,
        ) -> Result<Option<crate::mcp::oauth::McpOAuthCredentials>, crate::mcp::oauth::McpOAuthError>
        {
            Ok(self.credentials.lock().await.clone())
        }

        async fn save_credentials(
            &self,
            _credential_ref: &str,
            credentials: &crate::mcp::oauth::McpOAuthCredentials,
        ) -> Result<(), crate::mcp::oauth::McpOAuthError> {
            *self.credentials.lock().await = Some(credentials.clone());
            Ok(())
        }

        async fn clear_credentials(
            &self,
            _credential_ref: &str,
        ) -> Result<(), crate::mcp::oauth::McpOAuthError> {
            *self.credentials.lock().await = None;
            Ok(())
        }

        async fn save_state(
            &self,
            _state_ref: &str,
            _serialized: &str,
        ) -> Result<(), crate::mcp::oauth::McpOAuthError> {
            Ok(())
        }

        async fn clear_state(
            &self,
            _state_ref: &str,
        ) -> Result<(), crate::mcp::oauth::McpOAuthError> {
            Ok(())
        }
    }

    struct RefreshingOAuthFlow;

    #[async_trait::async_trait]
    impl crate::mcp::ports::McpOAuthFlowPort for RefreshingOAuthFlow {
        async fn begin(
            &self,
            _operation_id: &str,
            _request: &crate::mcp::oauth::McpOAuthRequest,
            _cancellation: CancellationToken,
        ) -> Result<crate::mcp::ports::McpOAuthPendingAuthorization, crate::mcp::oauth::McpOAuthError>
        {
            Err(crate::mcp::oauth::McpOAuthError::Cancelled)
        }

        async fn refresh(
            &self,
            credentials: &crate::mcp::oauth::McpOAuthCredentials,
        ) -> Result<crate::mcp::oauth::McpOAuthCredentials, crate::mcp::oauth::McpOAuthError>
        {
            let mut refreshed = credentials.clone();
            refreshed.access_token = "rotated-access".to_string();
            refreshed.refresh_token = Some("rotated-refresh".to_string());
            refreshed.expires_at = Some(chrono::Utc::now() + chrono::Duration::hours(1));
            Ok(refreshed)
        }

        async fn revoke(
            &self,
            _credentials: &crate::mcp::oauth::McpOAuthCredentials,
        ) -> Result<(), crate::mcp::oauth::McpOAuthError> {
            Ok(())
        }
    }

    #[test]
    fn static_http_headers_resolve_values_without_exposing_them_in_errors() {
        let headers = BTreeMap::from([(
            "X-Tenant".to_string(),
            PersistedValue::Secret {
                secret_ref:
                    "mcp-secret:v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                resolved_value: Some("tenant-secret".to_string()),
            },
        )]);
        let auth = McpAuth::Static {
            header_name: "Authorization".to_string(),
            scheme: Some("Bearer".to_string()),
            secret_ref:
                "mcp-secret:v1:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            resolved_value: Some("auth-secret".to_string()),
        };

        let resolved = resolved_headers("remote", &headers, &auth).unwrap();

        assert_eq!(resolved["X-Tenant"], "tenant-secret");
        assert_eq!(resolved["Authorization"], "Bearer auth-secret");
    }

    #[test]
    fn missing_http_secret_reports_only_opaque_ref() {
        let secret_ref =
            "mcp-secret:v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let headers = BTreeMap::from([(
            "X-Token".to_string(),
            PersistedValue::Secret {
                secret_ref: secret_ref.to_string(),
                resolved_value: None,
            },
        )]);

        let error = resolved_headers("remote", &headers, &McpAuth::None).unwrap_err();

        assert!(error.to_string().contains(secret_ref));
        assert!(!error.to_string().contains("X-Token"));
    }

    #[tokio::test]
    async fn oauth_header_refreshes_expiring_token_and_rotates_secure_credentials() {
        let credential_ref =
            "mcp-secret:v1:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        let mut server = remote_server("https://mcp.example.test/mcp");
        let McpConnection::StreamableHttp { auth, headers, .. } = &mut server.connection else {
            panic!("remote connection")
        };
        headers.clear();
        *auth = McpAuth::OAuth {
            client_id: "openflow".to_string(),
            scopes: vec!["tools.read".to_string()],
            issuer: Some("https://auth.example.test".to_string()),
            credential_ref: Some(credential_ref.to_string()),
        };
        let store = MemoryOAuthStore {
            credentials: tokio::sync::Mutex::new(Some(crate::mcp::oauth::McpOAuthCredentials {
                schema_version: 1,
                issuer: "https://auth.example.test".to_string(),
                client_id: "openflow".to_string(),
                client_secret: None,
                token_endpoint: "https://auth.example.test/token".to_string(),
                revocation_endpoint: None,
                token_endpoint_auth_method: "none".to_string(),
                resource: "https://mcp.example.test/mcp".to_string(),
                access_token: "expired-access".to_string(),
                refresh_token: Some("refresh-secret".to_string()),
                token_type: "Bearer".to_string(),
                granted_scopes: vec!["tools.read".to_string()],
                expires_at: Some(chrono::Utc::now()),
            })),
        };
        let McpConnection::StreamableHttp { headers, auth, .. } = &server.connection else {
            panic!("remote connection")
        };

        let resolved =
            resolved_headers_with_oauth_ports(&server, headers, auth, &store, &RefreshingOAuthFlow)
                .await
                .unwrap();

        assert_eq!(resolved["Authorization"], "Bearer rotated-access");
        let stored = store.credentials.lock().await.clone().unwrap();
        assert_eq!(stored.access_token, "rotated-access");
        assert_eq!(stored.refresh_token.as_deref(), Some("rotated-refresh"));
    }

    #[test]
    fn streamable_http_sse_reconnect_is_bounded() {
        let retry = bounded_sse_retry_policy();

        assert_eq!(retry.retry(0), Some(Duration::from_millis(250)));
        assert_eq!(retry.retry(1), Some(Duration::from_millis(500)));
        assert_eq!(retry.retry(2), Some(Duration::from_millis(1_000)));
        assert_eq!(retry.retry(3), None);
    }

    #[tokio::test]
    async fn streamable_http_negotiates_session_headers_and_deletes_on_close() {
        let (url, requests, stop, fixture) = spawn_http_fixture(false).await;
        let client = super::super::McpClient::spawn(&remote_server(&url))
            .await
            .unwrap();

        assert!(client.list_tool_names().await.unwrap().is_empty());
        client.close().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        stop.cancel();
        fixture.await.unwrap();

        let requests = requests.lock().unwrap();
        assert!(requests.iter().any(|request| request.method == "DELETE"));
        assert!(requests.iter().all(|request| {
            request.headers.get("x-api-key").map(String::as_str) == Some("static-secret")
        }));
        let list = requests
            .iter()
            .find(|request| request.body.contains("tools/list"))
            .expect("tools/list request");
        assert_eq!(
            list.headers.get("mcp-session-id").map(String::as_str),
            Some("session-1")
        );
        assert!(list.headers.contains_key("mcp-protocol-version"));
    }

    #[tokio::test]
    async fn expired_http_session_does_not_reinitialize_or_replay_tool_call() {
        let (url, requests, stop, fixture) = spawn_http_fixture(true).await;
        let client = super::super::McpClient::spawn(&remote_server(&url))
            .await
            .unwrap();

        let error = client
            .call_tool("dangerous_write", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(matches!(error, McpError::RemoteTransport { .. }));
        let _ = client.close().await;
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        stop.cancel();
        fixture.await.unwrap();

        let requests = requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.body.contains("\"method\":\"initialize\""))
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.body.contains("dangerous_write"))
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn streamable_http_surfaces_oauth_challenge() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let fixture = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_http_request(&mut socket).await.unwrap();
            socket
                .write_all(
                    b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Bearer resource_metadata=\"https://mcp.example.test/.well-known/oauth-protected-resource\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
        });
        let error =
            super::super::McpClient::spawn(&remote_server(&format!("http://{address}/mcp")))
                .await
                .expect_err("OAuth challenge must reject unauthenticated startup");
        fixture.await.unwrap();

        assert!(matches!(
            error,
            McpError::OAuthRequired { ref server_id } if server_id == "remote"
        ));
    }

    fn remote_server(url: &str) -> McpServerRecord {
        McpServerRecord::new(
            "remote",
            "Remote",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::StreamableHttp {
                url: url.to_string(),
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
        )
    }

    async fn spawn_http_fixture(
        expire_tool_call: bool,
    ) -> (
        String,
        Arc<Mutex<Vec<HttpRequestRecord>>>,
        CancellationToken,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let task_requests = requests.clone();
        let stop = CancellationToken::new();
        let task_stop = stop.clone();
        let task = tokio::spawn(async move {
            loop {
                let accepted = tokio::select! {
                    biased;
                    () = task_stop.cancelled() => break,
                    accepted = listener.accept() => accepted,
                };
                let Ok((mut socket, _)) = accepted else { break };
                let Some(request) = read_http_request(&mut socket).await else {
                    continue;
                };
                let response = fixture_response(&request, expire_tool_call);
                task_requests.lock().unwrap().push(request);
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        (format!("http://{address}/mcp"), requests, stop, task)
    }

    async fn read_http_request(socket: &mut tokio::net::TcpStream) -> Option<HttpRequestRecord> {
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
        let body = String::from_utf8_lossy(
            &bytes[header_end..bytes.len().min(header_end + content_length)],
        )
        .to_string();
        Some(HttpRequestRecord {
            method,
            headers,
            body,
        })
    }

    fn fixture_response(request: &HttpRequestRecord, expire_tool_call: bool) -> String {
        if request.method == "GET" {
            return empty_response("405 Method Not Allowed");
        }
        if request.method == "DELETE" {
            return empty_response("200 OK");
        }
        let body: serde_json::Value = serde_json::from_str(&request.body).unwrap_or_default();
        let method = body.get("method").and_then(serde_json::Value::as_str);
        if method == Some("notifications/initialized") {
            return empty_response("202 Accepted");
        }
        if method == Some("tools/call") && expire_tool_call {
            return empty_response("404 Not Found");
        }
        let id = body.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let result = match method {
            Some("initialize") => serde_json::json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "http-fixture", "version": "1.0.0"}
            }),
            Some("tools/list") => serde_json::json!({"tools": []}),
            Some("tools/call") => serde_json::json!({"content": [{"type": "text", "text": "ok"}]}),
            _ => serde_json::json!({}),
        };
        let response = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
        let response = serde_json::to_string(&response).unwrap();
        let session = if method == Some("initialize") {
            "Mcp-Session-Id: session-1\r\n"
        } else {
            ""
        };
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{session}Content-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        )
    }

    fn empty_response(status: &str) -> String {
        format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    }
}

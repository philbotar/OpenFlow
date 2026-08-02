use super::http_security::{is_loopback_host, secure_http_endpoint, validate_endpoint_url};
use crate::mcp::oauth::{
    McpOAuthCredentials, McpOAuthError, McpOAuthPublicConfig, McpOAuthRequest,
    MCP_OAUTH_CALLBACK_TIMEOUT_SECS,
};
use crate::mcp::ports::{mcp_secret_ref, McpOAuthFlowPort, McpOAuthPendingAuthorization};
use base64::Engine;
use chrono::{Duration as ChronoDuration, Utc};
use futures::StreamExt;
use reqwest::header::{HeaderMap, WWW_AUTHENTICATE};
use reqwest::{Method, StatusCode, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

const OAUTH_HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const OAUTH_BODY_LIMIT: usize = 1024 * 1024;
const OAUTH_CALLBACK_REQUEST_LIMIT: usize = 16 * 1024;
const OAUTH_CALLBACK_PATH: &str = "/oauth/callback";

#[derive(Debug, Default, Clone, Copy)]
pub struct McpOAuthHttpAdapter;

#[derive(Debug, Deserialize)]
struct ProtectedResourceMetadata {
    resource: Option<String>,
    authorization_servers: Vec<String>,
    #[serde(default)]
    scopes_supported: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AuthorizationServerMetadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: Option<String>,
    revocation_endpoint: Option<String>,
    #[serde(default)]
    scopes_supported: Vec<String>,
    #[serde(default)]
    response_types_supported: Vec<String>,
    #[serde(default)]
    code_challenge_methods_supported: Vec<String>,
    #[serde(default)]
    token_endpoint_auth_methods_supported: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ClientRegistrationRequest<'a> {
    client_name: &'a str,
    application_type: &'a str,
    redirect_uris: Vec<&'a str>,
    grant_types: Vec<&'a str>,
    response_types: Vec<&'a str>,
    token_endpoint_auth_method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClientRegistrationResponse {
    client_id: String,
    client_secret: Option<String>,
    token_endpoint_auth_method: Option<String>,
}

#[derive(Clone)]
struct OAuthClientRegistration {
    client_id: String,
    client_secret: Option<String>,
    token_endpoint_auth_method: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    scope: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingState {
    state: String,
    pkce_verifier: String,
    redirect_uri: String,
    issuer: String,
    created_at: chrono::DateTime<Utc>,
}

struct Discovery {
    resource: String,
    scopes: Vec<String>,
    metadata: AuthorizationServerMetadata,
}

#[async_trait::async_trait]
impl McpOAuthFlowPort for McpOAuthHttpAdapter {
    async fn begin(
        &self,
        operation_id: &str,
        request: &McpOAuthRequest,
        cancellation: CancellationToken,
    ) -> Result<McpOAuthPendingAuthorization, McpOAuthError> {
        let discovery = discover(request).await?;
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| McpOAuthError::InvalidCallback)?;
        let callback_port = listener
            .local_addr()
            .map_err(|_| McpOAuthError::InvalidCallback)?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{callback_port}{OAUTH_CALLBACK_PATH}");
        let registration = configure_or_register_client(request, &discovery, &redirect_uri).await?;
        let state = random_secret();
        let pkce_verifier = random_secret();
        let pkce_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(pkce_verifier.as_bytes()));
        let authorization_url = authorization_url(
            &discovery,
            &registration,
            &redirect_uri,
            &state,
            &pkce_challenge,
        )?;
        let credential_ref = mcp_secret_ref(&request.server_id, "oauth.credentials")
            .map_err(|_| McpOAuthError::SecureStorage)?
            .to_string();
        let state_ref = mcp_secret_ref(&request.server_id, &format!("oauth.state.{operation_id}"))
            .map_err(|_| McpOAuthError::SecureStorage)?
            .to_string();
        let serialized_state = serde_json::to_string(&PendingState {
            state: state.clone(),
            pkce_verifier: pkce_verifier.clone(),
            redirect_uri: redirect_uri.clone(),
            issuer: discovery.metadata.issuer.clone(),
            created_at: Utc::now(),
        })
        .map_err(|_| McpOAuthError::SecureStorage)?;
        let public_config = McpOAuthPublicConfig {
            client_id: registration.client_id.clone(),
            scopes: discovery.scopes.clone(),
            issuer: discovery.metadata.issuer.clone(),
            credential_ref: credential_ref.clone(),
        };
        let allow_localhost = request.allow_localhost;
        let completion = Box::pin(async move {
            let code = wait_for_callback(listener, &state, cancellation).await?;
            exchange_authorization_code(
                &discovery,
                &registration,
                &redirect_uri,
                &pkce_verifier,
                &code,
                allow_localhost,
            )
            .await
        });
        Ok(McpOAuthPendingAuthorization {
            authorization_url,
            public_config,
            credential_ref,
            state_ref,
            serialized_state,
            completion,
        })
    }

    async fn refresh(
        &self,
        credentials: &McpOAuthCredentials,
    ) -> Result<McpOAuthCredentials, McpOAuthError> {
        refresh_credentials(
            credentials,
            localhost_allowed_for_resource(&credentials.resource),
        )
        .await
    }

    async fn revoke(&self, credentials: &McpOAuthCredentials) -> Result<(), McpOAuthError> {
        let Some(revocation_endpoint) = credentials.revocation_endpoint.as_deref() else {
            return Ok(());
        };
        let token = credentials
            .refresh_token
            .as_deref()
            .unwrap_or(&credentials.access_token);
        let mut form = vec![
            ("token", token.to_string()),
            ("client_id", credentials.client_id.clone()),
        ];
        add_client_secret(&mut form, credentials);
        let response = send_form(
            revocation_endpoint,
            localhost_allowed_for_resource(&credentials.resource),
            &form,
            credentials,
        )
        .await
        .map_err(|_| McpOAuthError::Revoke)?;
        if response.status.is_success() {
            Ok(())
        } else {
            Err(McpOAuthError::Revoke)
        }
    }
}

async fn discover(request: &McpOAuthRequest) -> Result<Discovery, McpOAuthError> {
    let resource_url = validate_endpoint_url(&request.resource_url, request.allow_localhost)
        .map_err(|_| McpOAuthError::Discovery)?;
    let (challenge_metadata, challenge_scope) =
        resource_challenge(&resource_url, request.allow_localhost).await?;
    let metadata_urls = challenge_metadata
        .into_iter()
        .chain(protected_resource_metadata_urls(&resource_url));
    let mut protected = None;
    for url in metadata_urls {
        if let Some(candidate) =
            fetch_optional_json::<ProtectedResourceMetadata>(url.as_str(), request.allow_localhost)
                .await?
        {
            protected = Some(candidate);
            break;
        }
    }
    let protected = protected.ok_or(McpOAuthError::Discovery)?;
    if let Some(resource) = protected.resource.as_deref() {
        let declared = canonical_resource(resource)?;
        let expected = canonical_resource(resource_url.as_str())?;
        if declared != expected {
            return Err(McpOAuthError::Discovery);
        }
    }
    if protected.authorization_servers.is_empty() {
        return Err(McpOAuthError::Discovery);
    }
    let issuer = request
        .expected_issuer
        .as_deref()
        .map_or_else(
            || protected.authorization_servers.first().cloned(),
            |expected| {
                protected
                    .authorization_servers
                    .iter()
                    .find(|issuer| {
                        canonical_resource(issuer).ok() == canonical_resource(expected).ok()
                    })
                    .cloned()
            },
        )
        .ok_or(McpOAuthError::Discovery)?;
    let metadata = discover_authorization_server(&issuer, request.allow_localhost).await?;
    validate_authorization_metadata(&metadata, &issuer, request.allow_localhost).await?;
    let scopes = if !request.requested_scopes.is_empty() {
        request.requested_scopes.clone()
    } else if !challenge_scope.is_empty() {
        challenge_scope
    } else if !protected.scopes_supported.is_empty() {
        protected.scopes_supported
    } else {
        metadata.scopes_supported.clone()
    };
    Ok(Discovery {
        resource: canonical_resource(resource_url.as_str())?,
        scopes: dedupe_scopes(scopes),
        metadata,
    })
}

async fn resource_challenge(
    resource_url: &Url,
    allow_localhost: bool,
) -> Result<(Option<Url>, Vec<String>), McpOAuthError> {
    let response = send(
        Method::GET,
        resource_url.as_str(),
        allow_localhost,
        None,
        None,
    )
    .await
    .map_err(|_| McpOAuthError::Discovery)?;
    let mut metadata_url = None;
    let mut scopes = Vec::new();
    for header in response.headers.get_all(WWW_AUTHENTICATE) {
        let Ok(header) = header.to_str() else {
            continue;
        };
        if metadata_url.is_none() {
            metadata_url = auth_param(header, "resource_metadata")
                .and_then(|value| resource_url.join(&value).ok());
        }
        if scopes.is_empty() {
            scopes = auth_param(header, "scope")
                .map(|scope| scope.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default();
        }
    }
    Ok((metadata_url, scopes))
}

fn protected_resource_metadata_urls(resource: &Url) -> Vec<Url> {
    let path = resource.path().trim_matches('/');
    let mut urls = Vec::new();
    if !path.is_empty() {
        let mut url = resource.clone();
        url.set_query(None);
        url.set_path(&format!("/.well-known/oauth-protected-resource/{path}"));
        urls.push(url);
    }
    let mut root = resource.clone();
    root.set_query(None);
    root.set_path("/.well-known/oauth-protected-resource");
    if !urls.contains(&root) {
        urls.push(root);
    }
    urls
}

async fn discover_authorization_server(
    issuer: &str,
    allow_localhost: bool,
) -> Result<AuthorizationServerMetadata, McpOAuthError> {
    let issuer =
        validate_endpoint_url(issuer, allow_localhost).map_err(|_| McpOAuthError::Discovery)?;
    for url in authorization_metadata_urls(&issuer) {
        if let Some(metadata) =
            fetch_optional_json::<AuthorizationServerMetadata>(url.as_str(), allow_localhost)
                .await?
        {
            return Ok(metadata);
        }
    }
    Err(McpOAuthError::Discovery)
}

fn authorization_metadata_urls(issuer: &Url) -> Vec<Url> {
    let path = issuer.path().trim_matches('/');
    let mut paths = if path.is_empty() {
        vec![
            "/.well-known/oauth-authorization-server".to_string(),
            "/.well-known/openid-configuration".to_string(),
        ]
    } else {
        vec![
            format!("/.well-known/oauth-authorization-server/{path}"),
            format!("/.well-known/openid-configuration/{path}"),
            format!("/{path}/.well-known/openid-configuration"),
        ]
    };
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            let mut url = issuer.clone();
            url.set_query(None);
            url.set_path(&path);
            url
        })
        .collect()
}

async fn validate_authorization_metadata(
    metadata: &AuthorizationServerMetadata,
    expected_issuer: &str,
    allow_localhost: bool,
) -> Result<(), McpOAuthError> {
    if canonical_resource(&metadata.issuer)? != canonical_resource(expected_issuer)? {
        return Err(McpOAuthError::Discovery);
    }
    if !metadata.response_types_supported.is_empty()
        && !metadata
            .response_types_supported
            .iter()
            .any(|value| value == "code")
    {
        return Err(McpOAuthError::Discovery);
    }
    if !metadata
        .code_challenge_methods_supported
        .iter()
        .any(|method| method == "S256")
    {
        return Err(McpOAuthError::PkceRequired);
    }
    for endpoint in [
        Some(metadata.authorization_endpoint.as_str()),
        Some(metadata.token_endpoint.as_str()),
        metadata.registration_endpoint.as_deref(),
        metadata.revocation_endpoint.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        secure_http_endpoint(endpoint, allow_localhost)
            .await
            .map_err(|_| McpOAuthError::Discovery)?;
    }
    Ok(())
}

async fn configure_or_register_client(
    request: &McpOAuthRequest,
    discovery: &Discovery,
    redirect_uri: &str,
) -> Result<OAuthClientRegistration, McpOAuthError> {
    if !request.client_id.trim().is_empty() {
        if !discovery
            .metadata
            .token_endpoint_auth_methods_supported
            .is_empty()
            && !discovery
                .metadata
                .token_endpoint_auth_methods_supported
                .iter()
                .any(|method| method == "none")
        {
            return Err(McpOAuthError::RegistrationUnavailable);
        }
        return Ok(OAuthClientRegistration {
            client_id: request.client_id.trim().to_string(),
            client_secret: None,
            token_endpoint_auth_method: "none".to_string(),
        });
    }
    let endpoint = discovery
        .metadata
        .registration_endpoint
        .as_deref()
        .ok_or(McpOAuthError::RegistrationUnavailable)?;
    let request_body = ClientRegistrationRequest {
        client_name: "OpenFlow",
        application_type: "native",
        redirect_uris: vec![redirect_uri],
        grant_types: vec!["authorization_code", "refresh_token"],
        response_types: vec!["code"],
        token_endpoint_auth_method: "none",
        scope: (!discovery.scopes.is_empty()).then(|| discovery.scopes.join(" ")),
    };
    let body = serde_json::to_vec(&request_body).map_err(|_| McpOAuthError::Registration)?;
    let response = send(
        Method::POST,
        endpoint,
        request.allow_localhost,
        Some("application/json"),
        Some(body),
    )
    .await
    .map_err(|_| McpOAuthError::Registration)?;
    if !response.status.is_success() {
        return Err(McpOAuthError::Registration);
    }
    let response: ClientRegistrationResponse =
        serde_json::from_slice(&response.body).map_err(|_| McpOAuthError::Registration)?;
    if response.client_id.trim().is_empty() {
        return Err(McpOAuthError::Registration);
    }
    let client_secret = response.client_secret.filter(|secret| !secret.is_empty());
    let token_endpoint_auth_method = response.token_endpoint_auth_method.unwrap_or_else(|| {
        if client_secret.is_some() {
            "client_secret_post".to_string()
        } else {
            "none".to_string()
        }
    });
    if !matches!(
        token_endpoint_auth_method.as_str(),
        "none" | "client_secret_post" | "client_secret_basic"
    ) {
        return Err(McpOAuthError::Registration);
    }
    if !discovery
        .metadata
        .token_endpoint_auth_methods_supported
        .is_empty()
        && !discovery
            .metadata
            .token_endpoint_auth_methods_supported
            .iter()
            .any(|method| method == &token_endpoint_auth_method)
    {
        return Err(McpOAuthError::Registration);
    }
    Ok(OAuthClientRegistration {
        client_id: response.client_id,
        client_secret,
        token_endpoint_auth_method,
    })
}

fn authorization_url(
    discovery: &Discovery,
    registration: &OAuthClientRegistration,
    redirect_uri: &str,
    state: &str,
    pkce_challenge: &str,
) -> Result<String, McpOAuthError> {
    let mut url = Url::parse(&discovery.metadata.authorization_endpoint)
        .map_err(|_| McpOAuthError::Discovery)?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("response_type", "code");
        query.append_pair("client_id", &registration.client_id);
        query.append_pair("redirect_uri", redirect_uri);
        query.append_pair("state", state);
        query.append_pair("code_challenge", pkce_challenge);
        query.append_pair("code_challenge_method", "S256");
        query.append_pair("resource", &discovery.resource);
        if !discovery.scopes.is_empty() {
            query.append_pair("scope", &discovery.scopes.join(" "));
        }
    }
    Ok(url.to_string())
}

async fn wait_for_callback(
    listener: TcpListener,
    expected_state: &str,
    cancellation: CancellationToken,
) -> Result<String, McpOAuthError> {
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(MCP_OAUTH_CALLBACK_TIMEOUT_SECS);
    loop {
        let accepted = tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(McpOAuthError::Cancelled),
            result = tokio::time::timeout_at(deadline, listener.accept()) => {
                result.map_err(|_| McpOAuthError::CallbackTimeout)?
                    .map_err(|_| McpOAuthError::InvalidCallback)?
            }
        };
        let (mut socket, address) = accepted;
        if !address.ip().is_loopback() {
            let _ = callback_response(&mut socket, "403 Forbidden", "Rejected").await;
            continue;
        }
        match parse_callback(&mut socket, expected_state).await {
            Ok(code) => {
                callback_response(
                    &mut socket,
                    "200 OK",
                    "Authorization complete. Return to OpenFlow.",
                )
                .await?;
                return Ok(code);
            }
            Err(McpOAuthError::AuthorizationDenied) => {
                callback_response(&mut socket, "400 Bad Request", "Authorization denied.").await?;
                return Err(McpOAuthError::AuthorizationDenied);
            }
            Err(error) => {
                let _ =
                    callback_response(&mut socket, "400 Bad Request", "Invalid OAuth callback.")
                        .await;
                if matches!(
                    error,
                    McpOAuthError::StateMismatch | McpOAuthError::InvalidCallback
                ) {
                    continue;
                }
                return Err(error);
            }
        }
    }
}

async fn parse_callback(
    socket: &mut TcpStream,
    expected_state: &str,
) -> Result<String, McpOAuthError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 2048];
    loop {
        let read = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut buffer))
            .await
            .map_err(|_| McpOAuthError::InvalidCallback)?
            .map_err(|_| McpOAuthError::InvalidCallback)?;
        if read == 0 {
            return Err(McpOAuthError::InvalidCallback);
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if bytes.len() > OAUTH_CALLBACK_REQUEST_LIMIT {
            return Err(McpOAuthError::InvalidCallback);
        }
    }
    let request = std::str::from_utf8(&bytes).map_err(|_| McpOAuthError::InvalidCallback)?;
    let request_line = request
        .lines()
        .next()
        .ok_or(McpOAuthError::InvalidCallback)?;
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("GET") {
        return Err(McpOAuthError::InvalidCallback);
    }
    let target = parts.next().ok_or(McpOAuthError::InvalidCallback)?;
    let url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| McpOAuthError::InvalidCallback)?;
    if url.path() != OAUTH_CALLBACK_PATH {
        return Err(McpOAuthError::InvalidCallback);
    }
    let query = url
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    if query.get("state").map(|value| value.as_ref()) != Some(expected_state) {
        return Err(McpOAuthError::StateMismatch);
    }
    if query.contains_key("error") {
        return Err(McpOAuthError::AuthorizationDenied);
    }
    query
        .get("code")
        .filter(|code| !code.is_empty())
        .map(ToString::to_string)
        .ok_or(McpOAuthError::InvalidCallback)
}

async fn callback_response(
    socket: &mut TcpStream,
    status: &str,
    message: &str,
) -> Result<(), McpOAuthError> {
    let body = format!("<!doctype html><title>OpenFlow MCP OAuth</title><p>{message}</p>");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    socket
        .write_all(response.as_bytes())
        .await
        .map_err(|_| McpOAuthError::InvalidCallback)
}

async fn exchange_authorization_code(
    discovery: &Discovery,
    registration: &OAuthClientRegistration,
    redirect_uri: &str,
    pkce_verifier: &str,
    code: &str,
    allow_localhost: bool,
) -> Result<McpOAuthCredentials, McpOAuthError> {
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
        ("redirect_uri", redirect_uri.to_string()),
        ("code_verifier", pkce_verifier.to_string()),
        ("resource", discovery.resource.clone()),
        ("client_id", registration.client_id.clone()),
    ];
    add_registration_secret(&mut form, registration);
    let temporary = credentials_for_request(discovery, registration);
    let response = send_form(
        &discovery.metadata.token_endpoint,
        allow_localhost,
        &form,
        &temporary,
    )
    .await
    .map_err(|_| McpOAuthError::TokenExchange)?;
    if !response.status.is_success() {
        return Err(McpOAuthError::TokenExchange);
    }
    let token: TokenResponse =
        serde_json::from_slice(&response.body).map_err(|_| McpOAuthError::TokenExchange)?;
    credentials_from_token(discovery, registration, token, None)
}

async fn refresh_credentials(
    credentials: &McpOAuthCredentials,
    allow_localhost: bool,
) -> Result<McpOAuthCredentials, McpOAuthError> {
    let refresh_token = credentials
        .refresh_token
        .as_deref()
        .filter(|token| !token.is_empty())
        .ok_or(McpOAuthError::Refresh)?;
    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
        ("resource", credentials.resource.clone()),
        ("client_id", credentials.client_id.clone()),
    ];
    add_client_secret(&mut form, credentials);
    let response = send_form(
        &credentials.token_endpoint,
        allow_localhost,
        &form,
        credentials,
    )
    .await
    .map_err(|_| McpOAuthError::Refresh)?;
    if !response.status.is_success() {
        return Err(McpOAuthError::Refresh);
    }
    let token: TokenResponse =
        serde_json::from_slice(&response.body).map_err(|_| McpOAuthError::Refresh)?;
    if !token.token_type.eq_ignore_ascii_case("bearer") || token.access_token.is_empty() {
        return Err(McpOAuthError::Refresh);
    }
    let mut refreshed = credentials.clone();
    refreshed.access_token = token.access_token;
    refreshed.token_type = "Bearer".to_string();
    if token
        .refresh_token
        .as_deref()
        .is_some_and(|token| !token.is_empty())
    {
        refreshed.refresh_token = token.refresh_token;
    }
    if let Some(scope) = token.scope {
        refreshed.granted_scopes =
            dedupe_scopes(scope.split_whitespace().map(str::to_string).collect());
    }
    refreshed.expires_at = token
        .expires_in
        .map(|seconds| Utc::now() + ChronoDuration::seconds(seconds.min(i64::MAX as u64) as i64));
    Ok(refreshed)
}

fn credentials_for_request(
    discovery: &Discovery,
    registration: &OAuthClientRegistration,
) -> McpOAuthCredentials {
    McpOAuthCredentials {
        schema_version: 1,
        issuer: discovery.metadata.issuer.clone(),
        client_id: registration.client_id.clone(),
        client_secret: registration.client_secret.clone(),
        token_endpoint: discovery.metadata.token_endpoint.clone(),
        revocation_endpoint: discovery.metadata.revocation_endpoint.clone(),
        token_endpoint_auth_method: registration.token_endpoint_auth_method.clone(),
        resource: discovery.resource.clone(),
        access_token: String::new(),
        refresh_token: None,
        token_type: "Bearer".to_string(),
        granted_scopes: discovery.scopes.clone(),
        expires_at: None,
    }
}

fn credentials_from_token(
    discovery: &Discovery,
    registration: &OAuthClientRegistration,
    token: TokenResponse,
    previous_refresh_token: Option<String>,
) -> Result<McpOAuthCredentials, McpOAuthError> {
    if !token.token_type.eq_ignore_ascii_case("bearer") || token.access_token.is_empty() {
        return Err(McpOAuthError::TokenExchange);
    }
    let granted_scopes = token.scope.as_deref().map_or_else(
        || discovery.scopes.clone(),
        |scope| dedupe_scopes(scope.split_whitespace().map(str::to_string).collect()),
    );
    Ok(McpOAuthCredentials {
        schema_version: 1,
        issuer: discovery.metadata.issuer.clone(),
        client_id: registration.client_id.clone(),
        client_secret: registration.client_secret.clone(),
        token_endpoint: discovery.metadata.token_endpoint.clone(),
        revocation_endpoint: discovery.metadata.revocation_endpoint.clone(),
        token_endpoint_auth_method: registration.token_endpoint_auth_method.clone(),
        resource: discovery.resource.clone(),
        access_token: token.access_token,
        refresh_token: token.refresh_token.or(previous_refresh_token),
        token_type: "Bearer".to_string(),
        granted_scopes,
        expires_at: token.expires_in.map(|seconds| {
            Utc::now() + ChronoDuration::seconds(seconds.min(i64::MAX as u64) as i64)
        }),
    })
}

fn add_registration_secret(
    form: &mut Vec<(&'static str, String)>,
    registration: &OAuthClientRegistration,
) {
    if registration.token_endpoint_auth_method == "client_secret_post" {
        if let Some(secret) = registration.client_secret.as_deref() {
            form.push(("client_secret", secret.to_string()));
        }
    }
}

fn add_client_secret(form: &mut Vec<(&'static str, String)>, credentials: &McpOAuthCredentials) {
    if credentials.token_endpoint_auth_method == "client_secret_post" {
        if let Some(secret) = credentials.client_secret.as_deref() {
            form.push(("client_secret", secret.to_string()));
        }
    }
}

struct HttpResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

async fn fetch_optional_json<T: DeserializeOwned>(
    url: &str,
    allow_localhost: bool,
) -> Result<Option<T>, McpOAuthError> {
    let response = send(Method::GET, url, allow_localhost, None, None)
        .await
        .map_err(|_| McpOAuthError::Discovery)?;
    if response.status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status.is_success() {
        return Err(McpOAuthError::Discovery);
    }
    serde_json::from_slice(&response.body)
        .map(Some)
        .map_err(|_| McpOAuthError::Discovery)
}

async fn send_form(
    url: &str,
    allow_localhost: bool,
    form: &[(&'static str, String)],
    credentials: &McpOAuthCredentials,
) -> Result<HttpResponse, McpOAuthError> {
    let secured = secure_http_endpoint(url, allow_localhost)
        .await
        .map_err(|_| McpOAuthError::Discovery)?;
    let mut request = secured.client.post(secured.url).form(form);
    if credentials.token_endpoint_auth_method == "client_secret_basic" {
        let secret = credentials
            .client_secret
            .as_deref()
            .ok_or(McpOAuthError::TokenExchange)?;
        request = request.basic_auth(&credentials.client_id, Some(secret));
    }
    send_builder(request).await
}

async fn send(
    method: Method,
    url: &str,
    allow_localhost: bool,
    content_type: Option<&str>,
    body: Option<Vec<u8>>,
) -> Result<HttpResponse, McpOAuthError> {
    let secured = secure_http_endpoint(url, allow_localhost)
        .await
        .map_err(|_| McpOAuthError::Discovery)?;
    let mut request = secured
        .client
        .request(method, secured.url)
        .header("MCP-Protocol-Version", "2025-06-18");
    if let Some(content_type) = content_type {
        request = request.header(reqwest::header::CONTENT_TYPE, content_type);
    }
    if let Some(body) = body {
        request = request.body(body);
    }
    send_builder(request).await
}

async fn send_builder(request: reqwest::RequestBuilder) -> Result<HttpResponse, McpOAuthError> {
    let response = tokio::time::timeout(OAUTH_HTTP_TIMEOUT, request.send())
        .await
        .map_err(|_| McpOAuthError::Discovery)?
        .map_err(|_| McpOAuthError::Discovery)?;
    let status = response.status();
    let headers = response.headers().clone();
    if response
        .content_length()
        .is_some_and(|length| length > OAUTH_BODY_LIMIT as u64)
    {
        return Err(McpOAuthError::Discovery);
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = tokio::time::timeout(OAUTH_HTTP_TIMEOUT, stream.next())
        .await
        .map_err(|_| McpOAuthError::Discovery)?
    {
        let chunk = chunk.map_err(|_| McpOAuthError::Discovery)?;
        if body.len().saturating_add(chunk.len()) > OAUTH_BODY_LIMIT {
            return Err(McpOAuthError::Discovery);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn canonical_resource(resource: &str) -> Result<String, McpOAuthError> {
    let mut url = Url::parse(resource).map_err(|_| McpOAuthError::Discovery)?;
    if url.fragment().is_some() || url.host_str().is_none() {
        return Err(McpOAuthError::Discovery);
    }
    url.set_fragment(None);
    if url.path() == "/" && url.query().is_none() {
        url.set_path("");
    }
    Ok(url.to_string())
}

fn auth_param(header: &str, key: &str) -> Option<String> {
    let lower = header.to_ascii_lowercase();
    let marker = format!("{}=", key.to_ascii_lowercase());
    let start = lower.find(&marker)? + marker.len();
    let rest = header[start..].trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        let mut escaped = false;
        let mut value = String::new();
        for character in quoted.chars() {
            if escaped {
                value.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                return Some(value);
            } else {
                value.push(character);
            }
        }
        None
    } else {
        Some(
            rest.split(|character: char| character == ',' || character.is_whitespace())
                .next()?
                .to_string(),
        )
    }
}

fn dedupe_scopes(scopes: Vec<String>) -> Vec<String> {
    scopes
        .into_iter()
        .filter(|scope| !scope.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn random_secret() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn localhost_allowed_for_resource(resource: &str) -> bool {
    Url::parse(resource)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .is_some_and(|host| is_loopback_host(&host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    struct RequestRecord {
        method: String,
        path: String,
        body: String,
    }

    #[test]
    fn oauth_refresh_preserves_localhost_approval_for_subdomains() {
        assert!(localhost_allowed_for_resource("http://auth.localhost/mcp"));
        assert!(localhost_allowed_for_resource("http://LOCALHOST/mcp"));
        assert!(localhost_allowed_for_resource("http://127.0.0.1/mcp"));
        assert!(localhost_allowed_for_resource("http://[::1]/mcp"));
        assert!(!localhost_allowed_for_resource("https://example.com/mcp"));
        assert!(!localhost_allowed_for_resource("not a URL"));
    }

    #[test]
    fn oauth_header_parser_handles_quoted_values_without_leaking_adjacent_params() {
        let header = "Bearer realm=\"mcp\", resource_metadata=\"https://mcp.example.test/.well-known/oauth-protected-resource\", scope=\"tools.read tools.write\"";
        assert_eq!(
            auth_param(header, "resource_metadata").as_deref(),
            Some("https://mcp.example.test/.well-known/oauth-protected-resource")
        );
        assert_eq!(
            auth_param(header, "scope").as_deref(),
            Some("tools.read tools.write")
        );
    }

    #[test]
    fn oauth_discovery_paths_follow_rfc_path_insertion_order() {
        let issuer = Url::parse("https://auth.example.test/tenant").unwrap();
        let paths = authorization_metadata_urls(&issuer)
            .into_iter()
            .map(|url| url.path().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "/.well-known/oauth-authorization-server/tenant",
                "/.well-known/openid-configuration/tenant",
                "/tenant/.well-known/openid-configuration",
            ]
        );
    }

    #[tokio::test]
    async fn oauth_flow_discovers_registers_checks_state_refreshes_and_revokes() {
        let (base_url, requests, stop, fixture) = spawn_oauth_fixture().await;
        let request = McpOAuthRequest {
            server_id: "hosted".to_string(),
            resource_url: format!("{base_url}/mcp"),
            allow_localhost: true,
            client_id: String::new(),
            requested_scopes: Vec::new(),
            expected_issuer: None,
        };

        let pending = McpOAuthHttpAdapter
            .begin("operation-1", &request, CancellationToken::new())
            .await
            .unwrap();
        let authorization_url = Url::parse(&pending.authorization_url).unwrap();
        let params = authorization_url
            .query_pairs()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(params.get("resource"), Some(&format!("{base_url}/mcp")));
        assert_eq!(
            params.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(params.get("scope").map(String::as_str), Some("tools.read"));
        let redirect_uri = params.get("redirect_uri").unwrap().clone();
        let state = params.get("state").unwrap().clone();
        assert!(!pending.serialized_state.contains("access-token"));
        let completion = tokio::spawn(pending.completion);

        send_oauth_callback(&redirect_uri, "wrong-state", "wrong-code").await;
        assert!(!completion.is_finished());
        send_oauth_callback(&redirect_uri, &state, "authorization-code").await;
        let credentials = completion.await.unwrap().unwrap();

        assert_eq!(credentials.client_id, "dynamic-openflow");
        assert_eq!(credentials.access_token, "access-token-1");
        assert_eq!(
            credentials.refresh_token.as_deref(),
            Some("refresh-token-1")
        );
        assert_eq!(credentials.granted_scopes, vec!["tools.read"]);
        let refreshed = McpOAuthHttpAdapter.refresh(&credentials).await.unwrap();
        assert_eq!(refreshed.access_token, "access-token-2");
        assert_eq!(refreshed.refresh_token.as_deref(), Some("refresh-token-2"));
        McpOAuthHttpAdapter.revoke(&refreshed).await.unwrap();

        stop.cancel();
        fixture.await.unwrap();
        let requests = requests.lock().unwrap();
        assert!(requests.iter().any(|request| request.path == "/mcp"));
        assert!(requests
            .iter()
            .any(|request| request.path == "/.well-known/oauth-protected-resource"));
        let registration = requests
            .iter()
            .find(|request| request.path == "/register")
            .expect("registration request");
        assert!(registration
            .body
            .contains("\"application_type\":\"native\""));
        let token_requests = requests
            .iter()
            .filter(|request| request.path == "/token")
            .collect::<Vec<_>>();
        assert_eq!(token_requests.len(), 2);
        assert!(token_requests[0].body.contains("code_verifier="));
        assert!(token_requests[0]
            .body
            .contains("resource=http%3A%2F%2F127.0.0.1"));
        assert!(token_requests[1].body.contains("grant_type=refresh_token"));
        assert!(requests.iter().any(|request| request.path == "/revoke"));
    }

    #[tokio::test]
    async fn oauth_flow_refuses_authorization_server_without_pkce_s256() {
        let (base_url, _requests, stop, fixture) = spawn_oauth_fixture_with_pkce(false).await;
        let result = McpOAuthHttpAdapter
            .begin(
                "operation-1",
                &McpOAuthRequest {
                    server_id: "hosted".to_string(),
                    resource_url: format!("{base_url}/mcp"),
                    allow_localhost: true,
                    client_id: "openflow".to_string(),
                    requested_scopes: Vec::new(),
                    expected_issuer: None,
                },
                CancellationToken::new(),
            )
            .await;
        let error = match result {
            Ok(_) => panic!("PKCE refusal expected"),
            Err(error) => error,
        };
        assert_eq!(error, McpOAuthError::PkceRequired);
        stop.cancel();
        fixture.await.unwrap();
    }

    async fn spawn_oauth_fixture() -> (
        String,
        Arc<Mutex<Vec<RequestRecord>>>,
        CancellationToken,
        tokio::task::JoinHandle<()>,
    ) {
        spawn_oauth_fixture_with_pkce(true).await
    }

    async fn spawn_oauth_fixture_with_pkce(
        supports_pkce: bool,
    ) -> (
        String,
        Arc<Mutex<Vec<RequestRecord>>>,
        CancellationToken,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let base_url = format!("http://{address}");
        let fixture_base_url = base_url.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let task_requests = Arc::clone(&requests);
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
                let Some(request) = read_fixture_request(&mut socket).await else {
                    continue;
                };
                let response = oauth_fixture_response(&request, &fixture_base_url, supports_pkce);
                task_requests.lock().unwrap().push(request);
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        (base_url, requests, stop, task)
    }

    async fn read_fixture_request(socket: &mut TcpStream) -> Option<RequestRecord> {
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
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let mut lines = headers.lines();
        let mut request_line = lines.next()?.split_whitespace();
        let method = request_line.next()?.to_string();
        let path = request_line.next()?.to_string();
        let content_length = lines
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
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
            path,
            body: String::from_utf8_lossy(
                &bytes[header_end..bytes.len().min(header_end + content_length)],
            )
            .to_string(),
        })
    }

    fn oauth_fixture_response(
        request: &RequestRecord,
        base_url: &str,
        supports_pkce: bool,
    ) -> String {
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/mcp") => format!(
                "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Bearer resource_metadata=\"{base_url}/.well-known/oauth-protected-resource\", scope=\"tools.read\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            ),
            ("GET", "/.well-known/oauth-protected-resource") => json_response(
                serde_json::json!({
                    "resource": format!("{base_url}/mcp"),
                    "authorization_servers": [base_url],
                    "scopes_supported": ["tools.read"]
                }),
            ),
            ("GET", "/.well-known/oauth-authorization-server") => {
                let pkce = if supports_pkce {
                    serde_json::json!(["S256"])
                } else {
                    serde_json::json!(["plain"])
                };
                json_response(serde_json::json!({
                    "issuer": base_url,
                    "authorization_endpoint": format!("{base_url}/authorize"),
                    "token_endpoint": format!("{base_url}/token"),
                    "registration_endpoint": format!("{base_url}/register"),
                    "revocation_endpoint": format!("{base_url}/revoke"),
                    "response_types_supported": ["code"],
                    "code_challenge_methods_supported": pkce,
                    "token_endpoint_auth_methods_supported": ["none"],
                    "scopes_supported": ["tools.read"]
                }))
            }
            ("POST", "/register") => json_response(serde_json::json!({
                "client_id": "dynamic-openflow",
                "token_endpoint_auth_method": "none"
            })),
            ("POST", "/token") if request.body.contains("grant_type=refresh_token") => {
                json_response(serde_json::json!({
                    "access_token": "access-token-2",
                    "refresh_token": "refresh-token-2",
                    "token_type": "Bearer",
                    "expires_in": 3600,
                    "scope": "tools.read"
                }))
            }
            ("POST", "/token") => json_response(serde_json::json!({
                "access_token": "access-token-1",
                "refresh_token": "refresh-token-1",
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": "tools.read"
            })),
            ("POST", "/revoke") => {
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
            }
            _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
        }
    }

    fn json_response(value: serde_json::Value) -> String {
        let body = serde_json::to_string(&value).unwrap();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    async fn send_oauth_callback(redirect_uri: &str, state: &str, code: &str) {
        let redirect = Url::parse(redirect_uri).unwrap();
        let address = format!(
            "{}:{}",
            redirect.host_str().unwrap(),
            redirect.port().unwrap()
        );
        let mut socket = TcpStream::connect(address).await.unwrap();
        let request = format!(
            "GET {}?state={state}&code={code} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            redirect.path()
        );
        socket.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        socket.read_to_end(&mut response).await.unwrap();
    }
}

use super::ports::{McpOAuthFlowPort, McpOAuthPendingAuthorization, McpOAuthSecureStore};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

pub const MCP_OAUTH_CALLBACK_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOAuthRequest {
    pub server_id: String,
    pub resource_url: String,
    pub allow_localhost: bool,
    pub client_id: String,
    pub requested_scopes: Vec<String>,
    pub expected_issuer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOAuthPublicConfig {
    pub client_id: String,
    pub scopes: Vec<String>,
    pub issuer: String,
    pub credential_ref: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthCredentials {
    pub schema_version: u32,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub token_endpoint: String,
    pub revocation_endpoint: Option<String>,
    pub token_endpoint_auth_method: String,
    pub resource: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub granted_scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for McpOAuthCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpOAuthCredentials")
            .field("schema_version", &self.schema_version)
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_endpoint", &self.token_endpoint)
            .field("revocation_endpoint", &self.revocation_endpoint)
            .field(
                "token_endpoint_auth_method",
                &self.token_endpoint_auth_method,
            )
            .field("resource", &self.resource)
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_type", &self.token_type)
            .field("granted_scopes", &self.granted_scopes)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl McpOAuthCredentials {
    #[must_use]
    pub fn requires_refresh(&self, now: DateTime<Utc>) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at <= now + chrono::Duration::seconds(30))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpOAuthStatusState {
    Disconnected,
    Connecting,
    Connected,
    ReauthorizationRequired,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthStatus {
    pub server_id: String,
    pub state: McpOAuthStatusState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl McpOAuthStatus {
    #[must_use]
    pub fn disconnected(server_id: impl Into<String>) -> Self {
        Self {
            server_id: server_id.into(),
            state: McpOAuthStatusState::Disconnected,
            client_id: None,
            issuer: None,
            credential_ref: None,
            granted_scopes: Vec::new(),
            expires_at: None,
            error: None,
        }
    }

    fn from_credentials(server_id: &str, credentials: &McpOAuthCredentials) -> Self {
        let expired_without_refresh = credentials.requires_refresh(Utc::now())
            && credentials
                .refresh_token
                .as_deref()
                .is_none_or(str::is_empty);
        Self {
            server_id: server_id.to_string(),
            state: if expired_without_refresh {
                McpOAuthStatusState::ReauthorizationRequired
            } else {
                McpOAuthStatusState::Connected
            },
            client_id: Some(credentials.client_id.clone()),
            issuer: Some(credentials.issuer.clone()),
            credential_ref: None,
            granted_scopes: credentials.granted_scopes.clone(),
            expires_at: credentials.expires_at,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOAuthStart {
    pub operation_id: String,
    pub authorization_url: String,
    pub public_config: McpOAuthPublicConfig,
    pub status: McpOAuthStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpOAuthError {
    #[error("MCP OAuth requires a remote HTTP connection")]
    InvalidConnection,
    #[error("MCP OAuth discovery failed")]
    Discovery,
    #[error("MCP OAuth server does not advertise PKCE S256 support")]
    PkceRequired,
    #[error("MCP OAuth client registration is unavailable; enter a client ID")]
    RegistrationUnavailable,
    #[error("MCP OAuth client registration failed")]
    Registration,
    #[error("MCP OAuth browser callback timed out")]
    CallbackTimeout,
    #[error("MCP OAuth browser callback was invalid")]
    InvalidCallback,
    #[error("MCP OAuth authorization state did not match")]
    StateMismatch,
    #[error("MCP OAuth authorization was denied")]
    AuthorizationDenied,
    #[error("MCP OAuth token exchange failed")]
    TokenExchange,
    #[error("MCP OAuth token refresh failed; re-authentication required")]
    Refresh,
    #[error("MCP OAuth token revocation failed")]
    Revoke,
    #[error("MCP OAuth secure storage failed")]
    SecureStorage,
    #[error("MCP OAuth operation was cancelled")]
    Cancelled,
}

#[derive(Clone)]
struct OAuthOperation {
    operation_id: String,
    cancellation: CancellationToken,
    status: McpOAuthStatus,
}

#[derive(Clone)]
pub struct McpOAuthCoordinator {
    flow: Arc<dyn McpOAuthFlowPort>,
    store: Arc<dyn McpOAuthSecureStore>,
    operations: Arc<Mutex<BTreeMap<String, OAuthOperation>>>,
    completion_lock: Arc<tokio::sync::Mutex<()>>,
}

impl std::fmt::Debug for McpOAuthCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpOAuthCoordinator")
            .finish_non_exhaustive()
    }
}

impl McpOAuthCoordinator {
    #[must_use]
    pub fn new(flow: Arc<dyn McpOAuthFlowPort>, store: Arc<dyn McpOAuthSecureStore>) -> Self {
        Self {
            flow,
            store,
            operations: Arc::new(Mutex::new(BTreeMap::new())),
            completion_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn start(&self, request: McpOAuthRequest) -> Result<McpOAuthStart, McpOAuthError> {
        let operation_id = uuid::Uuid::new_v4().to_string();
        let cancellation = CancellationToken::new();
        if let Some(previous) = self
            .operations
            .lock()
            .map_err(|_| McpOAuthError::SecureStorage)?
            .remove(&request.server_id)
        {
            previous.cancellation.cancel();
        }
        let pending = self
            .flow
            .begin(&operation_id, &request, cancellation.clone())
            .await?;
        self.store
            .save_state(&pending.state_ref, &pending.serialized_state)
            .await?;
        let status = McpOAuthStatus {
            server_id: request.server_id.clone(),
            state: McpOAuthStatusState::Connecting,
            client_id: Some(pending.public_config.client_id.clone()),
            issuer: Some(pending.public_config.issuer.clone()),
            credential_ref: Some(pending.public_config.credential_ref.clone()),
            granted_scopes: pending.public_config.scopes.clone(),
            expires_at: None,
            error: None,
        };
        self.operations
            .lock()
            .map_err(|_| McpOAuthError::SecureStorage)?
            .insert(
                request.server_id.clone(),
                OAuthOperation {
                    operation_id: operation_id.clone(),
                    cancellation,
                    status: status.clone(),
                },
            );
        let authorization_url = pending.authorization_url.clone();
        let public_config = pending.public_config.clone();
        self.spawn_completion(request.server_id.clone(), operation_id.clone(), pending);
        Ok(McpOAuthStart {
            operation_id,
            authorization_url,
            public_config,
            status,
        })
    }

    fn spawn_completion(
        &self,
        server_id: String,
        operation_id: String,
        pending: McpOAuthPendingAuthorization,
    ) {
        let store = Arc::clone(&self.store);
        let operations = Arc::clone(&self.operations);
        let completion_lock = Arc::clone(&self.completion_lock);
        tokio::spawn(async move {
            let McpOAuthPendingAuthorization {
                credential_ref,
                state_ref,
                completion,
                ..
            } = pending;
            let result = completion.await;
            let _completion_guard = completion_lock.lock().await;
            let current = operations
                .lock()
                .ok()
                .and_then(|operations| operations.get(&server_id).cloned())
                .is_some_and(|operation| operation.operation_id == operation_id);
            if !current {
                let _ = store.clear_state(&state_ref).await;
                return;
            }
            let status = match result {
                Ok(credentials) => {
                    match store.save_credentials(&credential_ref, &credentials).await {
                        Ok(()) => {
                            let mut status =
                                McpOAuthStatus::from_credentials(&server_id, &credentials);
                            status.credential_ref = Some(credential_ref.clone());
                            status
                        }
                        Err(error) => failed_status(&server_id, error),
                    }
                }
                Err(error) => failed_status(&server_id, error),
            };
            let _ = store.clear_state(&state_ref).await;
            if let Ok(mut operations) = operations.lock() {
                if let Some(operation) = operations.get_mut(&server_id) {
                    if operation.operation_id == operation_id {
                        operation.status = status;
                    }
                }
            }
        });
    }

    pub async fn status(
        &self,
        server_id: &str,
        credential_ref: Option<&str>,
    ) -> Result<McpOAuthStatus, McpOAuthError> {
        if let Some(status) = self
            .operations
            .lock()
            .map_err(|_| McpOAuthError::SecureStorage)?
            .get(server_id)
            .map(|operation| operation.status.clone())
        {
            return Ok(status);
        }
        let Some(credential_ref) = credential_ref else {
            return Ok(McpOAuthStatus::disconnected(server_id));
        };
        let mut status = self
            .store
            .load_credentials(credential_ref)
            .await?
            .as_ref()
            .map_or_else(
                || McpOAuthStatus::disconnected(server_id),
                |credentials| McpOAuthStatus::from_credentials(server_id, credentials),
            );
        if status.state != McpOAuthStatusState::Disconnected {
            status.credential_ref = Some(credential_ref.to_string());
        }
        Ok(status)
    }

    pub async fn disconnect(
        &self,
        server_id: &str,
        credential_ref: Option<&str>,
    ) -> Result<McpOAuthStatus, McpOAuthError> {
        let _completion_guard = self.completion_lock.lock().await;
        if let Some(operation) = self
            .operations
            .lock()
            .map_err(|_| McpOAuthError::SecureStorage)?
            .remove(server_id)
        {
            operation.cancellation.cancel();
        }
        let mut revoke_error = None;
        if let Some(credential_ref) = credential_ref {
            if let Some(credentials) = self.store.load_credentials(credential_ref).await? {
                if let Err(error) = self.flow.revoke(&credentials).await {
                    revoke_error = Some(error.to_string());
                }
            }
            self.store.clear_credentials(credential_ref).await?;
        }
        let state_ref = super::ports::mcp_secret_ref(server_id, "oauth.state")
            .map_err(|_| McpOAuthError::SecureStorage)?;
        self.store.clear_state(state_ref.as_str()).await?;
        let mut status = McpOAuthStatus::disconnected(server_id);
        status.error = revoke_error;
        Ok(status)
    }

    pub async fn refresh(
        &self,
        server_id: &str,
        credential_ref: &str,
    ) -> Result<McpOAuthStatus, McpOAuthError> {
        let credentials = self
            .store
            .load_credentials(credential_ref)
            .await?
            .ok_or(McpOAuthError::Refresh)?;
        let refreshed = self.flow.refresh(&credentials).await?;
        self.store
            .save_credentials(credential_ref, &refreshed)
            .await?;
        let mut status = McpOAuthStatus::from_credentials(server_id, &refreshed);
        status.credential_ref = Some(credential_ref.to_string());
        if let Ok(mut operations) = self.operations.lock() {
            operations.remove(server_id);
        }
        Ok(status)
    }
}

fn failed_status(server_id: &str, error: McpOAuthError) -> McpOAuthStatus {
    McpOAuthStatus {
        server_id: server_id.to_string(),
        state: if matches!(error, McpOAuthError::Refresh) {
            McpOAuthStatusState::ReauthorizationRequired
        } else {
            McpOAuthStatusState::Failed
        },
        issuer: None,
        client_id: None,
        credential_ref: None,
        granted_scopes: Vec::new(),
        expires_at: None,
        error: Some(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::ports::{McpOAuthFlowPort, McpOAuthPendingAuthorization, McpOAuthSecureStore};
    use std::collections::{BTreeMap, VecDeque};

    #[derive(Default)]
    struct MemoryOAuthStore {
        credentials: tokio::sync::Mutex<Option<McpOAuthCredentials>>,
        states: tokio::sync::Mutex<BTreeMap<String, String>>,
    }

    #[async_trait::async_trait]
    impl McpOAuthSecureStore for MemoryOAuthStore {
        async fn load_credentials(
            &self,
            _credential_ref: &str,
        ) -> Result<Option<McpOAuthCredentials>, McpOAuthError> {
            Ok(self.credentials.lock().await.clone())
        }

        async fn save_credentials(
            &self,
            _credential_ref: &str,
            credentials: &McpOAuthCredentials,
        ) -> Result<(), McpOAuthError> {
            *self.credentials.lock().await = Some(credentials.clone());
            Ok(())
        }

        async fn clear_credentials(&self, _credential_ref: &str) -> Result<(), McpOAuthError> {
            *self.credentials.lock().await = None;
            Ok(())
        }

        async fn save_state(&self, state_ref: &str, serialized: &str) -> Result<(), McpOAuthError> {
            self.states
                .lock()
                .await
                .insert(state_ref.to_string(), serialized.to_string());
            Ok(())
        }

        async fn clear_state(&self, state_ref: &str) -> Result<(), McpOAuthError> {
            self.states.lock().await.remove(state_ref);
            Ok(())
        }
    }

    struct ControlledOAuthFlow {
        completions: Mutex<
            VecDeque<tokio::sync::oneshot::Receiver<Result<McpOAuthCredentials, McpOAuthError>>>,
        >,
    }

    #[async_trait::async_trait]
    impl McpOAuthFlowPort for ControlledOAuthFlow {
        async fn begin(
            &self,
            operation_id: &str,
            request: &McpOAuthRequest,
            _cancellation: CancellationToken,
        ) -> Result<McpOAuthPendingAuthorization, McpOAuthError> {
            let completion = self
                .completions
                .lock()
                .unwrap()
                .pop_front()
                .expect("controlled completion");
            let credential_ref =
                super::super::ports::mcp_secret_ref(&request.server_id, "oauth.credentials")
                    .unwrap()
                    .to_string();
            let state_ref = super::super::ports::mcp_secret_ref(
                &request.server_id,
                &format!("oauth.state.{operation_id}"),
            )
            .unwrap()
            .to_string();
            Ok(McpOAuthPendingAuthorization {
                authorization_url: "https://auth.example.test/authorize".to_string(),
                public_config: McpOAuthPublicConfig {
                    client_id: request.client_id.clone(),
                    scopes: request.requested_scopes.clone(),
                    issuer: "https://auth.example.test".to_string(),
                    credential_ref: credential_ref.clone(),
                },
                credential_ref,
                state_ref,
                serialized_state: "state-secret".to_string(),
                completion: Box::pin(async move {
                    completion.await.map_err(|_| McpOAuthError::Cancelled)?
                }),
            })
        }

        async fn refresh(
            &self,
            credentials: &McpOAuthCredentials,
        ) -> Result<McpOAuthCredentials, McpOAuthError> {
            Ok(credentials.clone())
        }

        async fn revoke(&self, _credentials: &McpOAuthCredentials) -> Result<(), McpOAuthError> {
            Ok(())
        }
    }

    #[test]
    fn oauth_credentials_debug_and_status_never_expose_tokens() {
        let credentials = McpOAuthCredentials {
            schema_version: 1,
            issuer: "https://auth.example.test".to_string(),
            client_id: "openflow".to_string(),
            client_secret: Some("client-secret".to_string()),
            token_endpoint: "https://auth.example.test/token".to_string(),
            revocation_endpoint: None,
            token_endpoint_auth_method: "none".to_string(),
            resource: "https://mcp.example.test/mcp".to_string(),
            access_token: "access-secret".to_string(),
            refresh_token: Some("refresh-secret".to_string()),
            token_type: "Bearer".to_string(),
            granted_scopes: vec!["tools.read".to_string()],
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
        };

        let debug = format!("{credentials:?}");
        assert!(!debug.contains("client-secret"));
        assert!(!debug.contains("access-secret"));
        assert!(!debug.contains("refresh-secret"));
        let status = McpOAuthStatus::from_credentials("remote", &credentials);
        let api = serde_json::to_string(&status).unwrap();
        assert!(!api.contains("secret"));
        assert_eq!(status.state, McpOAuthStatusState::Connected);
    }

    #[tokio::test]
    async fn stale_oauth_completion_cannot_overwrite_newer_credentials_or_status() {
        let (first_sender, first_receiver) = tokio::sync::oneshot::channel();
        let (second_sender, second_receiver) = tokio::sync::oneshot::channel();
        let flow = Arc::new(ControlledOAuthFlow {
            completions: Mutex::new(VecDeque::from([first_receiver, second_receiver])),
        });
        let store = Arc::new(MemoryOAuthStore::default());
        let coordinator = McpOAuthCoordinator::new(flow, store.clone());
        let request = |client_id: &str| McpOAuthRequest {
            server_id: "hosted".to_string(),
            resource_url: "https://mcp.example.test/mcp".to_string(),
            allow_localhost: false,
            client_id: client_id.to_string(),
            requested_scopes: vec!["tools.read".to_string()],
            expected_issuer: None,
        };
        coordinator.start(request("first-client")).await.unwrap();
        let second = coordinator.start(request("second-client")).await.unwrap();
        first_sender
            .send(Ok(credentials("first-client", "first-token")))
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(store.credentials.lock().await.is_none());
        second_sender
            .send(Ok(credentials("second-client", "second-token")))
            .unwrap();
        for _ in 0..20 {
            let status = coordinator
                .status("hosted", Some(&second.public_config.credential_ref))
                .await
                .unwrap();
            if status.state == McpOAuthStatusState::Connected {
                assert_eq!(status.client_id.as_deref(), Some("second-client"));
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let stored = store.credentials.lock().await.clone().unwrap();
        assert_eq!(stored.client_id, "second-client");
        assert_eq!(stored.access_token, "second-token");
    }

    fn credentials(client_id: &str, access_token: &str) -> McpOAuthCredentials {
        McpOAuthCredentials {
            schema_version: 1,
            issuer: "https://auth.example.test".to_string(),
            client_id: client_id.to_string(),
            client_secret: None,
            token_endpoint: "https://auth.example.test/token".to_string(),
            revocation_endpoint: None,
            token_endpoint_auth_method: "none".to_string(),
            resource: "https://mcp.example.test/mcp".to_string(),
            access_token: access_token.to_string(),
            refresh_token: Some("refresh-token".to_string()),
            token_type: "Bearer".to_string(),
            granted_scopes: vec!["tools.read".to_string()],
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
        }
    }
}

use crate::mcp::oauth::{McpOAuthCredentials, McpOAuthError};
use crate::mcp::ports::SecretStore;
use crate::mcp::ports::{McpOAuthSecureStore, McpSecretRef};
use crate::settings::ports::SettingsStore;
use std::sync::Arc;

const MAX_OAUTH_SECRET_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub struct SettingsOAuthStore {
    settings: Arc<dyn SettingsStore>,
}

impl std::fmt::Debug for SettingsOAuthStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsOAuthStore")
            .finish_non_exhaustive()
    }
}

impl SettingsOAuthStore {
    #[must_use]
    pub fn new(settings: Arc<dyn SettingsStore>) -> Self {
        Self { settings }
    }

    async fn get(&self, secret_ref: &str) -> Result<Option<String>, McpOAuthError> {
        let secret_ref = parse_ref(secret_ref)?;
        let settings = Arc::clone(&self.settings);
        tokio::task::spawn_blocking(move || settings.get_mcp_secret(&secret_ref))
            .await
            .map_err(|_| McpOAuthError::SecureStorage)?
            .map_err(|_| McpOAuthError::SecureStorage)
    }

    async fn set(&self, secret_ref: &str, value: String) -> Result<(), McpOAuthError> {
        if value.len() > MAX_OAUTH_SECRET_BYTES {
            return Err(McpOAuthError::SecureStorage);
        }
        let secret_ref = parse_ref(secret_ref)?;
        let settings = Arc::clone(&self.settings);
        tokio::task::spawn_blocking(move || settings.set_mcp_secret(&secret_ref, &value))
            .await
            .map_err(|_| McpOAuthError::SecureStorage)?
            .map_err(|_| McpOAuthError::SecureStorage)
    }

    async fn clear(&self, secret_ref: &str) -> Result<(), McpOAuthError> {
        let secret_ref = parse_ref(secret_ref)?;
        let settings = Arc::clone(&self.settings);
        tokio::task::spawn_blocking(move || settings.delete_mcp_secret(&secret_ref))
            .await
            .map_err(|_| McpOAuthError::SecureStorage)?
            .map_err(|_| McpOAuthError::SecureStorage)
    }
}

#[async_trait::async_trait]
impl McpOAuthSecureStore for SettingsOAuthStore {
    async fn load_credentials(
        &self,
        credential_ref: &str,
    ) -> Result<Option<McpOAuthCredentials>, McpOAuthError> {
        self.get(credential_ref)
            .await?
            .map(|value| serde_json::from_str(&value).map_err(|_| McpOAuthError::SecureStorage))
            .transpose()
    }

    async fn save_credentials(
        &self,
        credential_ref: &str,
        credentials: &McpOAuthCredentials,
    ) -> Result<(), McpOAuthError> {
        let value = serde_json::to_string(credentials).map_err(|_| McpOAuthError::SecureStorage)?;
        self.set(credential_ref, value).await
    }

    async fn clear_credentials(&self, credential_ref: &str) -> Result<(), McpOAuthError> {
        self.clear(credential_ref).await
    }

    async fn save_state(&self, state_ref: &str, serialized: &str) -> Result<(), McpOAuthError> {
        self.set(state_ref, serialized.to_string()).await
    }

    async fn clear_state(&self, state_ref: &str) -> Result<(), McpOAuthError> {
        self.clear(state_ref).await
    }
}

fn parse_ref(secret_ref: &str) -> Result<McpSecretRef, McpOAuthError> {
    secret_ref
        .parse::<McpSecretRef>()
        .map_err(|_| McpOAuthError::SecureStorage)
}

#[derive(Clone)]
pub struct SystemOAuthStore {
    secrets: Arc<dyn SecretStore>,
}

impl std::fmt::Debug for SystemOAuthStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SystemOAuthStore")
            .finish_non_exhaustive()
    }
}

impl Default for SystemOAuthStore {
    fn default() -> Self {
        Self::new(Arc::new(super::FileSecretStore::new(
            super::FileSecretStore::default_path(),
        )))
    }
}

impl SystemOAuthStore {
    #[must_use]
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self {
        Self { secrets }
    }

    async fn get(&self, secret_ref: &str) -> Result<Option<String>, McpOAuthError> {
        let secret_ref = parse_ref(secret_ref)?;
        let secrets = Arc::clone(&self.secrets);
        tokio::task::spawn_blocking(move || secrets.get(&secret_ref))
            .await
            .map_err(|_| McpOAuthError::SecureStorage)?
            .map_err(|_| McpOAuthError::SecureStorage)
    }

    async fn set(&self, secret_ref: &str, value: String) -> Result<(), McpOAuthError> {
        if value.len() > MAX_OAUTH_SECRET_BYTES {
            return Err(McpOAuthError::SecureStorage);
        }
        let secret_ref = parse_ref(secret_ref)?;
        let secrets = Arc::clone(&self.secrets);
        tokio::task::spawn_blocking(move || secrets.set(&secret_ref, &value))
            .await
            .map_err(|_| McpOAuthError::SecureStorage)?
            .map_err(|_| McpOAuthError::SecureStorage)
    }

    async fn clear(&self, secret_ref: &str) -> Result<(), McpOAuthError> {
        let secret_ref = parse_ref(secret_ref)?;
        let secrets = Arc::clone(&self.secrets);
        tokio::task::spawn_blocking(move || secrets.delete(&secret_ref))
            .await
            .map_err(|_| McpOAuthError::SecureStorage)?
            .map_err(|_| McpOAuthError::SecureStorage)
    }
}

#[async_trait::async_trait]
impl McpOAuthSecureStore for SystemOAuthStore {
    async fn load_credentials(
        &self,
        credential_ref: &str,
    ) -> Result<Option<McpOAuthCredentials>, McpOAuthError> {
        self.get(credential_ref)
            .await?
            .map(|value| serde_json::from_str(&value).map_err(|_| McpOAuthError::SecureStorage))
            .transpose()
    }

    async fn save_credentials(
        &self,
        credential_ref: &str,
        credentials: &McpOAuthCredentials,
    ) -> Result<(), McpOAuthError> {
        let value = serde_json::to_string(credentials).map_err(|_| McpOAuthError::SecureStorage)?;
        self.set(credential_ref, value).await
    }

    async fn clear_credentials(&self, credential_ref: &str) -> Result<(), McpOAuthError> {
        self.clear(credential_ref).await
    }

    async fn save_state(&self, state_ref: &str, serialized: &str) -> Result<(), McpOAuthError> {
        self.set(state_ref, serialized.to_string()).await
    }

    async fn clear_state(&self, state_ref: &str) -> Result<(), McpOAuthError> {
        self.clear(state_ref).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::mcp::FileSecretStore;
    use tempfile::tempdir;

    #[test]
    fn oauth_store_rejects_non_opaque_refs() {
        assert!(matches!(
            parse_ref("keychain://openflow/mcp/token"),
            Err(McpOAuthError::SecureStorage)
        ));
    }

    #[tokio::test]
    async fn system_oauth_store_uses_injected_file_secret_store() {
        let dir = tempdir().unwrap();
        let secrets = Arc::new(FileSecretStore::new(dir.path().join("mcp-secrets.json")));
        let store = SystemOAuthStore::new(secrets.clone());
        let credential_ref =
            crate::mcp::ports::mcp_secret_ref("hosted", "oauth.credentials").unwrap();
        let credentials = McpOAuthCredentials {
            schema_version: 1,
            issuer: "https://auth.example.test".into(),
            client_id: "openflow".into(),
            client_secret: None,
            token_endpoint: "https://auth.example.test/token".into(),
            revocation_endpoint: None,
            token_endpoint_auth_method: "none".into(),
            resource: "https://mcp.example.test".into(),
            access_token: "access-token".into(),
            refresh_token: Some("refresh-token".into()),
            token_type: "Bearer".into(),
            expires_at: None,
            granted_scopes: vec!["tools.read".into()],
        };

        store
            .save_credentials(credential_ref.as_str(), &credentials)
            .await
            .unwrap();

        let loaded = store
            .load_credentials(credential_ref.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.access_token, credentials.access_token);
        assert_eq!(loaded.refresh_token, credentials.refresh_token);
        assert!(secrets.get(&credential_ref).unwrap().is_some());
    }
}

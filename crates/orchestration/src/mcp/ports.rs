use sha2::{Digest, Sha256};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use tokio_util::sync::CancellationToken;

use super::oauth::{McpOAuthCredentials, McpOAuthError, McpOAuthPublicConfig, McpOAuthRequest};

const MCP_SECRET_REF_PREFIX: &str = "mcp-secret:v1:";
const MCP_SECRET_REF_DOMAIN: &[u8] = b"io.openflow.mcp.secret-ref\0v1\0";
const MAX_SECRET_REF_COMPONENT_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretRefComponent {
    ServerId,
    Slot,
}

impl fmt::Display for SecretRefComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServerId => formatter.write_str("server ID"),
            Self::Slot => formatter.write_str("slot"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpSecretRefError {
    #[error("MCP secret ref {component} must not be empty")]
    Empty { component: SecretRefComponent },
    #[error("MCP secret ref {component} exceeds {max_bytes} bytes")]
    TooLong {
        component: SecretRefComponent,
        max_bytes: usize,
    },
    #[error("MCP secret ref {component} contains unsupported characters")]
    InvalidCharacters { component: SecretRefComponent },
    #[error("MCP secret ref has an invalid opaque reference format")]
    InvalidReference,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct McpSecretRef(String);

impl McpSecretRef {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for McpSecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("McpSecretRef")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for McpSecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for McpSecretRef {
    type Err = McpSecretRefError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let digest = value
            .strip_prefix(MCP_SECRET_REF_PREFIX)
            .ok_or(McpSecretRefError::InvalidReference)?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(McpSecretRefError::InvalidReference);
        }
        Ok(Self(value.to_string()))
    }
}

pub fn mcp_secret_ref(server_id: &str, slot: &str) -> Result<McpSecretRef, McpSecretRefError> {
    validate_secret_ref_component(server_id, SecretRefComponent::ServerId)?;
    validate_secret_ref_component(slot, SecretRefComponent::Slot)?;

    let mut hasher = Sha256::new();
    hasher.update(MCP_SECRET_REF_DOMAIN);
    update_length_prefixed(&mut hasher, server_id.as_bytes());
    update_length_prefixed(&mut hasher, slot.as_bytes());
    let digest = hasher.finalize();
    Ok(McpSecretRef(format!("{MCP_SECRET_REF_PREFIX}{digest:x}")))
}

fn validate_secret_ref_component(
    value: &str,
    component: SecretRefComponent,
) -> Result<(), McpSecretRefError> {
    if value.is_empty() {
        return Err(McpSecretRefError::Empty { component });
    }
    if value.len() > MAX_SECRET_REF_COMPONENT_BYTES {
        return Err(McpSecretRefError::TooLong {
            component,
            max_bytes: MAX_SECRET_REF_COMPONENT_BYTES,
        });
    }
    let valid = value.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'-' | b'_' | b'.'))
    });
    if !valid {
        return Err(McpSecretRefError::InvalidCharacters { component });
    }
    Ok(())
}

fn update_length_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretStoreOperation {
    Get,
    Set,
    Delete,
}

impl fmt::Display for SecretStoreOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => formatter.write_str("read"),
            Self::Set => formatter.write_str("write"),
            Self::Delete => formatter.write_str("delete"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretStoreError {
    #[error("MCP secret storage is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("MCP secret storage access was denied during {operation}")]
    AccessDenied { operation: SecretStoreOperation },
    #[error("MCP secret storage is unavailable during {operation}")]
    Unavailable { operation: SecretStoreOperation },
    #[error("MCP secret storage rejected a request during {operation}")]
    InvalidRequest { operation: SecretStoreOperation },
    #[error("MCP secret storage failed during {operation}")]
    Backend { operation: SecretStoreOperation },
}

pub trait SecretStore: Send + Sync {
    fn get(&self, secret_ref: &McpSecretRef) -> Result<Option<String>, SecretStoreError>;

    fn set(&self, secret_ref: &McpSecretRef, secret_value: &str) -> Result<(), SecretStoreError>;

    fn delete(&self, secret_ref: &McpSecretRef) -> Result<(), SecretStoreError>;
}

pub type McpOAuthCompletion =
    Pin<Box<dyn Future<Output = Result<McpOAuthCredentials, McpOAuthError>> + Send + 'static>>;

pub struct McpOAuthPendingAuthorization {
    pub authorization_url: String,
    pub public_config: McpOAuthPublicConfig,
    pub credential_ref: String,
    pub state_ref: String,
    pub serialized_state: String,
    pub completion: McpOAuthCompletion,
}

#[async_trait::async_trait]
pub trait McpOAuthFlowPort: Send + Sync {
    async fn begin(
        &self,
        operation_id: &str,
        request: &McpOAuthRequest,
        cancellation: CancellationToken,
    ) -> Result<McpOAuthPendingAuthorization, McpOAuthError>;

    async fn refresh(
        &self,
        credentials: &McpOAuthCredentials,
    ) -> Result<McpOAuthCredentials, McpOAuthError>;

    async fn revoke(&self, credentials: &McpOAuthCredentials) -> Result<(), McpOAuthError>;
}

#[async_trait::async_trait]
pub trait McpOAuthSecureStore: Send + Sync {
    async fn load_credentials(
        &self,
        credential_ref: &str,
    ) -> Result<Option<McpOAuthCredentials>, McpOAuthError>;

    async fn save_credentials(
        &self,
        credential_ref: &str,
        credentials: &McpOAuthCredentials,
    ) -> Result<(), McpOAuthError>;

    async fn clear_credentials(&self, credential_ref: &str) -> Result<(), McpOAuthError>;

    async fn save_state(&self, state_ref: &str, serialized: &str) -> Result<(), McpOAuthError>;

    async fn clear_state(&self, state_ref: &str) -> Result<(), McpOAuthError>;
}

#[cfg(test)]
mod tests {
    use super::{mcp_secret_ref, McpSecretRef, SecretStore, SecretStoreError};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemorySecretStore {
        values: Mutex<HashMap<McpSecretRef, String>>,
    }

    impl SecretStore for MemorySecretStore {
        fn get(&self, secret_ref: &McpSecretRef) -> Result<Option<String>, SecretStoreError> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(
            &self,
            secret_ref: &McpSecretRef,
            secret_value: &str,
        ) -> Result<(), SecretStoreError> {
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.clone(), secret_value.to_string());
            Ok(())
        }

        fn delete(&self, secret_ref: &McpSecretRef) -> Result<(), SecretStoreError> {
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    #[test]
    fn mcp_secret_ref_is_deterministic_opaque_and_parseable() {
        let first = mcp_secret_ref("filesystem-1", "env.API_TOKEN").unwrap();
        let second = mcp_secret_ref("filesystem-1", "env.API_TOKEN").unwrap();
        let different = mcp_secret_ref("filesystem-1", "env.OTHER_TOKEN").unwrap();

        assert_eq!(first, second);
        assert_ne!(first, different);
        assert!(first.as_str().starts_with("mcp-secret:v1:"));
        assert_eq!(first.as_str().len(), "mcp-secret:v1:".len() + 64);
        assert!(first
            .as_str()
            .strip_prefix("mcp-secret:v1:")
            .unwrap()
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase()));
        assert!(!first.as_str().contains("filesystem"));
        assert!(!first.as_str().contains("API_TOKEN"));
        assert_eq!(first.as_str().parse::<McpSecretRef>().unwrap(), first);
    }

    #[test]
    fn mcp_secret_ref_rejects_invalid_components_and_wire_values() {
        for invalid in ["", " has-space", "slash/value", "colon:value", "unicode-é"] {
            assert!(mcp_secret_ref(invalid, "env.API_TOKEN").is_err());
            assert!(mcp_secret_ref("server", invalid).is_err());
        }
        assert!(mcp_secret_ref(&"a".repeat(129), "env.API_TOKEN").is_err());
        assert!(mcp_secret_ref("server", &"a".repeat(129)).is_err());

        for invalid in [
            "",
            "mcp-secret:v2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "mcp-secret:v1:short",
            "mcp-secret:v1:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "mcp-secret:v1:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            assert!(invalid.parse::<McpSecretRef>().is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn mcp_secret_store_port_supports_object_safe_get_set_delete() {
        let store: Box<dyn SecretStore> = Box::new(MemorySecretStore::default());
        let secret_ref = mcp_secret_ref("filesystem", "env.API_TOKEN").unwrap();

        assert_eq!(store.get(&secret_ref).unwrap(), None);
        store.set(&secret_ref, "secret-value").unwrap();
        assert_eq!(
            store.get(&secret_ref).unwrap().as_deref(),
            Some("secret-value")
        );
        store.delete(&secret_ref).unwrap();
        assert_eq!(store.get(&secret_ref).unwrap(), None);
        store.delete(&secret_ref).unwrap();
    }
}

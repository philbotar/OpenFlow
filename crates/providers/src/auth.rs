use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthConfig {
    Bearer {
        api_key: Option<String>,
        required: bool,
    },
    Header {
        name: String,
        api_key: Option<String>,
        required: bool,
    },
    NoneAllowed,
    AwsCredentials {
        profile: Option<String>,
        region: String,
    },
}

/// Refreshable credentials issued by the `ChatGPT OAuth` flow.
///
/// The values are persisted as one unit because a refresh may rotate more than
/// just the access token. Debug output intentionally exposes presence and
/// expiry metadata only.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexOAuthCredentials {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: Option<String>,
    /// Token expiry as Unix seconds.
    pub expires_at: i64,
    pub account_id: String,
    pub email: Option<String>,
}

impl fmt::Debug for CodexOAuthCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodexOAuthCredentials")
            .field("access_token_present", &!self.access_token.is_empty())
            .field("refresh_token_present", &!self.refresh_token.is_empty())
            .field("id_token_present", &self.id_token.is_some())
            .field("expires_at", &self.expires_at)
            .field("account_id_present", &!self.account_id.is_empty())
            .field("email_present", &self.email.is_some())
            .finish()
    }
}

impl AuthConfig {
    #[must_use]
    pub const fn requires_key(&self) -> bool {
        match self {
            Self::Bearer { required, .. } | Self::Header { required, .. } => *required,
            Self::NoneAllowed | Self::AwsCredentials { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CodexOAuthCredentials;

    #[test]
    fn codex_credentials_round_trip_without_debugging_secrets() {
        let credentials = CodexOAuthCredentials {
            access_token: "access-sentinel".to_string(),
            refresh_token: "refresh-sentinel".to_string(),
            id_token: Some("id-sentinel".to_string()),
            expires_at: 1_800_000_000,
            account_id: "account-sentinel".to_string(),
            email: Some("person@example.com".to_string()),
        };

        let encoded = serde_json::to_string(&credentials);
        assert!(encoded.is_ok());
        let Ok(encoded) = encoded else {
            return;
        };
        let decoded: Result<CodexOAuthCredentials, _> = serde_json::from_str(&encoded);
        assert!(decoded.is_ok());
        let Ok(decoded) = decoded else {
            return;
        };
        assert_eq!(decoded, credentials);

        let debug = format!("{credentials:?}");
        for secret in [
            "access-sentinel",
            "refresh-sentinel",
            "id-sentinel",
            "account-sentinel",
            "person@example.com",
        ] {
            assert!(!debug.contains(secret), "debug output leaked {secret}");
        }
        assert!(debug.contains("expires_at"));
        assert!(debug.contains("id_token_present"));
        assert!(debug.contains("email_present"));
    }
}

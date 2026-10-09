use engine::AgentError;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
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
    pub(crate) fn http_auth(&self, label: &str) -> Result<(String, HeaderMap), AgentError> {
        match self {
            Self::Bearer { api_key, required } => {
                let key = extract_api_key(api_key.as_ref(), *required, label)?;
                Ok((key.unwrap_or_default(), HeaderMap::new()))
            }
            Self::Header {
                name,
                api_key,
                required,
            } => {
                let key = extract_api_key(api_key.as_ref(), *required, label)?;
                let mut headers = HeaderMap::new();
                if let Some(key) = key {
                    let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                        AgentError::Failed(format!("invalid auth header name `{name}`: {e}"))
                    })?;
                    let header_value = HeaderValue::from_str(&key).map_err(|e| {
                        AgentError::Failed(format!("invalid auth header value for `{name}`: {e}"))
                    })?;
                    headers.insert(header_name, header_value);
                }
                Ok((String::new(), headers))
            }
            Self::NoneAllowed => Ok((String::new(), HeaderMap::new())),
            Self::AwsCredentials { .. } => Err(AgentError::Failed(format!(
                "{label} HTTP auth is not configured for rig adapter"
            ))),
        }
    }
}

fn extract_api_key(
    api_key: Option<&String>,
    required: bool,
    label: &str,
) -> Result<Option<String>, AgentError> {
    let key = api_key.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    if key.is_none() && required {
        return Err(AgentError::Permanent(format!("{label} API key missing")));
    }
    Ok(key)
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

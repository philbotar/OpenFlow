use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

pub const MCP_SERVER_RECORD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ExactPackageVersion(String);

impl ExactPackageVersion {
    pub fn new(version: impl Into<String>) -> Result<Self, InvalidPackageVersion> {
        let version = version.into();
        if version.trim().is_empty() {
            return Err(InvalidPackageVersion::Empty);
        }
        let has_range_syntax = version
            .chars()
            .any(|character| character.is_whitespace() || "*^~<>=|,".contains(character));
        let looks_like_tag = !version.chars().any(|character| character.is_ascii_digit());
        if has_range_syntax || looks_like_tag {
            return Err(InvalidPackageVersion::Floating(version));
        }
        Ok(Self(version))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ExactPackageVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let version = String::deserialize(deserializer)?;
        Self::new(version).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ExactPackageVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidPackageVersion {
    #[error("package version must not be empty")]
    Empty,
    #[error("package version must be exact, got {0:?}")]
    Floating(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum McpServerSource {
    Manual,
    Imported {
        dialect: String,
        source_path: String,
    },
    Registry {
        catalog_base_url: String,
        server_name: String,
        version: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum McpInstall {
    External,
    Npm {
        package: String,
        version: ExactPackageVersion,
    },
    Pypi {
        package: String,
        version: ExactPackageVersion,
        executable: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInstallRevision {
    pub install: McpInstall,
    pub connection: McpConnection,
    pub installed_at: DateTime<Utc>,
    pub target_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInstallHistory {
    pub current: McpInstallRevision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<McpInstallRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PersistedValue {
    Literal {
        value: String,
    },
    Secret {
        secret_ref: String,
        #[serde(skip)]
        resolved_value: Option<String>,
    },
}

impl PersistedValue {
    #[must_use]
    pub fn runtime_value(&self) -> Option<&str> {
        match self {
            Self::Literal { value } => Some(value),
            Self::Secret { resolved_value, .. } => resolved_value.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum McpAuth {
    #[default]
    None,
    Static {
        header_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        scheme: Option<String>,
        secret_ref: String,
        #[serde(skip)]
        resolved_value: Option<String>,
    },
    #[serde(rename = "oauth")]
    OAuth {
        client_id: String,
        #[serde(default)]
        scopes: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        issuer: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        credential_ref: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpTransportKind {
    Stdio,
    StreamableHttp,
    LegacySse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum McpConnection {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        environment: BTreeMap<String, PersistedValue>,
    },
    StreamableHttp {
        url: String,
        #[serde(default)]
        allow_localhost: bool,
        #[serde(default)]
        headers: BTreeMap<String, PersistedValue>,
        #[serde(default)]
        auth: McpAuth,
    },
    LegacySse {
        url: String,
        #[serde(default)]
        allow_localhost: bool,
        #[serde(default)]
        headers: BTreeMap<String, PersistedValue>,
        #[serde(default)]
        auth: McpAuth,
    },
}

impl McpConnection {
    #[must_use]
    pub fn transport_kind(&self) -> McpTransportKind {
        match self {
            Self::Stdio { .. } => McpTransportKind::Stdio,
            Self::StreamableHttp { .. } => McpTransportKind::StreamableHttp,
            Self::LegacySse { .. } => McpTransportKind::LegacySse,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpToolAccess {
    Read,
    #[default]
    Write,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpToolConcurrency {
    Shared,
    #[default]
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct McpPolicy {
    pub default_tool_access: McpToolAccess,
    pub default_tool_concurrency: McpToolConcurrency,
    pub allow_roots: bool,
    pub allow_sampling: bool,
    pub allow_elicitation: bool,
    pub sampling_max_requests_per_run: u32,
    pub sampling_max_tokens_per_request: u32,
    pub sampling_max_total_tokens_per_run: u32,
    pub elicitation_max_requests_per_run: u32,
    /// `None` keeps every discovered tool available to the existing approval mode.
    pub enabled_tools: Option<BTreeSet<String>>,
}

impl Default for McpPolicy {
    fn default() -> Self {
        Self {
            default_tool_access: McpToolAccess::Write,
            default_tool_concurrency: McpToolConcurrency::Exclusive,
            allow_roots: false,
            allow_sampling: false,
            allow_elicitation: false,
            sampling_max_requests_per_run: 4,
            sampling_max_tokens_per_request: 4_096,
            sampling_max_total_tokens_per_run: 8_192,
            elicitation_max_requests_per_run: 8,
            enabled_tools: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct McpTrust {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRecord {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub source: McpServerSource,
    pub install: McpInstall,
    pub connection: McpConnection,
    #[serde(default)]
    pub trust: McpTrust,
    #[serde(default)]
    pub policy: McpPolicy,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_history: Option<McpInstallHistory>,
}

impl McpServerRecord {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        source: McpServerSource,
        install: McpInstall,
        connection: McpConnection,
    ) -> Self {
        Self {
            schema_version: MCP_SERVER_RECORD_VERSION,
            id: id.into(),
            display_name: display_name.into(),
            source,
            install,
            connection,
            trust: McpTrust::default(),
            policy: McpPolicy::default(),
            enabled: false,
            install_history: None,
        }
    }

    /// Returns an IPC-safe copy. Secret refs remain; literal values are blanked.
    #[must_use]
    pub fn redacted(&self) -> Self {
        let mut record = self.clone();
        let values = match &mut record.connection {
            McpConnection::Stdio { environment, .. } => environment,
            McpConnection::StreamableHttp { headers, .. }
            | McpConnection::LegacySse { headers, .. } => headers,
        };
        for value in values.values_mut() {
            match value {
                PersistedValue::Literal { value } => value.clear(),
                PersistedValue::Secret { resolved_value, .. } => *resolved_value = None,
            }
        }
        match &mut record.connection {
            McpConnection::StreamableHttp {
                auth: McpAuth::Static { resolved_value, .. },
                ..
            }
            | McpConnection::LegacySse {
                auth: McpAuth::Static { resolved_value, .. },
                ..
            } => *resolved_value = None,
            _ => {}
        }
        record
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExactPackageVersion, McpAuth, McpConnection, McpInstall, McpPolicy, McpServerRecord,
        McpServerSource, McpToolAccess, McpToolConcurrency, PersistedValue,
        MCP_SERVER_RECORD_VERSION,
    };
    use std::collections::BTreeMap;

    #[test]
    fn package_versions_must_be_exact() {
        assert!(ExactPackageVersion::new("1.2.3").is_ok());

        for invalid in [
            "", "   ", "latest", "LATEST", "next", "beta", "*", "^1.2.3", "~1.2.3", ">=1", "<2",
            "=1.2.3", "1 || 2", "1.2, 1.3", "1.2 1.3",
        ] {
            assert!(
                ExactPackageVersion::new(invalid).is_err(),
                "{invalid:?} must not be accepted as an exact package version"
            );
        }

        for invalid_json in [r#"""#, r#""latest""#, r#""*""#] {
            assert!(serde_json::from_str::<ExactPackageVersion>(invalid_json).is_err());
        }
    }

    #[test]
    fn record_separates_versioned_domain_components() {
        let record = McpServerRecord::new(
            "filesystem",
            "Filesystem",
            McpServerSource::Registry {
                catalog_base_url: "https://registry.example.test".to_string(),
                server_name: "filesystem".to_string(),
                version: "2026.8.1".to_string(),
            },
            McpInstall::Npm {
                package: "@modelcontextprotocol/server-filesystem".to_string(),
                version: ExactPackageVersion::new("1.2.3").unwrap(),
            },
            McpConnection::Stdio {
                command: "npx".to_string(),
                args: vec!["--yes".to_string()],
                environment: BTreeMap::new(),
            },
        );

        assert_eq!(record.schema_version, MCP_SERVER_RECORD_VERSION);
        assert!(!record.enabled);
        assert_eq!(record.policy.default_tool_access, McpToolAccess::Write);
        assert_eq!(
            record.policy.default_tool_concurrency,
            McpToolConcurrency::Exclusive
        );
        assert!(!record.policy.allow_roots);
        assert!(!record.policy.allow_sampling);
        assert!(!record.policy.allow_elicitation);
        assert!(record.policy.enabled_tools.is_none());

        let json = serde_json::to_value(record).unwrap();
        assert_eq!(json["source"]["type"], "registry");
        assert!(json["source"].get("publisherVerified").is_none());
        assert_eq!(json["install"]["type"], "npm");
        assert_eq!(json["connection"]["type"], "stdio");
        assert_eq!(json["schemaVersion"], MCP_SERVER_RECORD_VERSION);
    }

    #[test]
    fn secret_values_serialize_as_refs_without_value_fields() {
        let secret = PersistedValue::Secret {
            secret_ref: "keychain://mcp/filesystem/token".to_string(),
            resolved_value: Some("must-not-serialize".to_string()),
        };
        let json = serde_json::to_value(secret).unwrap();

        assert_eq!(json["type"], "secret");
        assert_eq!(json["secretRef"], "keychain://mcp/filesystem/token");
        assert!(json.get("value").is_none());
    }

    #[test]
    fn http_auth_stores_metadata_and_secret_refs_only() {
        let auth = McpAuth::OAuth {
            client_id: "openflow".to_string(),
            scopes: vec!["mcp:tools".to_string()],
            issuer: Some("https://auth.example.test".to_string()),
            credential_ref: Some("keychain://mcp/example/oauth".to_string()),
        };
        let json = serde_json::to_value(auth).unwrap();
        let object = json.as_object().unwrap();

        assert_eq!(object.get("type").unwrap(), "oauth");
        assert!(!object.contains_key("accessToken"));
        assert!(!object.contains_key("refreshToken"));
        assert!(!object.contains_key("clientSecret"));
    }

    #[test]
    fn conservative_policy_is_the_deserialization_default() {
        let policy: McpPolicy = serde_json::from_str("{}").unwrap();

        assert_eq!(policy, McpPolicy::default());
    }
}

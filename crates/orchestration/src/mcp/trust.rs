use crate::mcp::model::{McpConnection, McpInstall, McpPolicy, McpServerRecord, McpServerSource};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintMaterial<'a> {
    source: &'a McpServerSource,
    install: &'a McpInstall,
    connection: &'a McpConnection,
    policy: &'a McpPolicy,
}

#[derive(Debug, thiserror::Error)]
pub enum FingerprintError {
    #[error("failed to serialize MCP fingerprint material: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub fn current_fingerprint(record: &McpServerRecord) -> Result<String, FingerprintError> {
    let material = FingerprintMaterial {
        source: &record.source,
        install: &record.install,
        connection: &record.connection,
        policy: &record.policy,
    };
    let canonical = serde_json::to_vec(&material)?;
    let digest = Sha256::digest(canonical);
    Ok(format!("{digest:x}"))
}

pub fn approve_current(
    record: &mut McpServerRecord,
    approved_at: DateTime<Utc>,
) -> Result<String, FingerprintError> {
    let fingerprint = current_fingerprint(record)?;
    record.trust.approved_fingerprint = Some(fingerprint.clone());
    record.trust.approved_at = Some(approved_at);
    Ok(fingerprint)
}

#[must_use]
pub fn is_trusted(record: &McpServerRecord) -> bool {
    let Ok(current) = current_fingerprint(record) else {
        return false;
    };
    record.trust.approved_fingerprint.as_deref() == Some(current.as_str())
}

#[cfg(test)]
mod tests {
    use super::{approve_current, current_fingerprint, is_trusted};
    use crate::mcp::model::{
        McpConnection, McpInstall, McpServerRecord, McpServerSource, PersistedValue,
    };
    use chrono::{TimeZone, Utc};
    use std::collections::BTreeMap;

    fn record_with_environment(environment: BTreeMap<String, PersistedValue>) -> McpServerRecord {
        McpServerRecord::new(
            "server-id",
            "Server",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: "server".to_string(),
                args: vec!["--serve".to_string()],
                environment,
            },
        )
    }

    #[test]
    fn fingerprint_is_deterministic_for_canonical_maps() {
        let mut first_environment = BTreeMap::new();
        first_environment.insert(
            "Z_SECRET".to_string(),
            PersistedValue::Secret {
                secret_ref: "keychain://mcp/z".to_string(),
                resolved_value: Some("first-secret".to_string()),
            },
        );
        first_environment.insert(
            "A_MODE".to_string(),
            PersistedValue::Literal {
                value: "safe".to_string(),
            },
        );

        let mut second_environment = BTreeMap::new();
        second_environment.insert(
            "A_MODE".to_string(),
            PersistedValue::Literal {
                value: "safe".to_string(),
            },
        );
        second_environment.insert(
            "Z_SECRET".to_string(),
            PersistedValue::Secret {
                secret_ref: "keychain://mcp/z".to_string(),
                resolved_value: Some("different-resolved-secret".to_string()),
            },
        );

        assert_eq!(
            current_fingerprint(&record_with_environment(first_environment)).unwrap(),
            current_fingerprint(&record_with_environment(second_environment)).unwrap()
        );
    }

    #[test]
    fn approval_matches_only_the_current_security_relevant_config() {
        let approved_at = Utc.with_ymd_and_hms(2026, 8, 1, 10, 0, 0).unwrap();
        let mut record = record_with_environment(BTreeMap::new());

        let approved = approve_current(&mut record, approved_at).unwrap();
        assert_eq!(
            record.trust.approved_fingerprint.as_deref(),
            Some(approved.as_str())
        );
        assert!(is_trusted(&record));

        record.display_name = "Renamed".to_string();
        record.enabled = true;
        record.trust.approved_at = Some(Utc.with_ymd_and_hms(2027, 1, 2, 3, 4, 5).unwrap());
        assert!(is_trusted(&record));

        let McpConnection::Stdio { args, .. } = &mut record.connection else {
            panic!("test record must use stdio");
        };
        args.push("--write".to_string());
        assert!(!is_trusted(&record));
    }
}

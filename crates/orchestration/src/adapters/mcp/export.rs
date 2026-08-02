use crate::mcp::model::McpServerRecord;
use serde::{Deserialize, Serialize};

pub const OPENFLOW_MCP_EXPORT_FORMAT: &str = "openflow.mcp";
pub const OPENFLOW_MCP_EXPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenFlowMcpExport {
    format: String,
    schema_version: u32,
    servers: Vec<McpServerRecord>,
}

/// Exports deterministic OpenFlow MCP JSON without resolved or literal values.
///
/// Secret references remain so another OpenFlow install can identify required
/// inputs. OAuth access and refresh tokens are not part of the normalized model.
pub fn export_canonical_mcp_json(servers: &[McpServerRecord]) -> Result<String, serde_json::Error> {
    let mut servers = servers
        .iter()
        .map(McpServerRecord::redacted)
        .collect::<Vec<_>>();
    servers.sort_by(|left, right| {
        (&left.id, &left.display_name).cmp(&(&right.id, &right.display_name))
    });

    let document = OpenFlowMcpExport {
        format: OPENFLOW_MCP_EXPORT_FORMAT.to_string(),
        schema_version: OPENFLOW_MCP_EXPORT_SCHEMA_VERSION,
        servers,
    };
    let mut json = serde_json::to_string_pretty(&document)?;
    json.push('\n');
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::model::{McpAuth, McpConnection, McpInstall, McpServerSource, PersistedValue};
    use std::collections::BTreeMap;

    #[test]
    fn canonical_export_is_deterministic_and_contains_refs_not_values() {
        let stdio = McpServerRecord::new(
            "z-stdio",
            "Stdio",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: "server".to_string(),
                args: vec!["--stdio".to_string()],
                environment: BTreeMap::from([
                    (
                        "API_TOKEN".to_string(),
                        PersistedValue::Secret {
                            secret_ref: "mcp-secret:v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                                .to_string(),
                            resolved_value: Some("resolved-must-not-export".to_string()),
                        },
                    ),
                    (
                        "PUBLIC_LABEL".to_string(),
                        PersistedValue::Literal {
                            value: "literal-must-not-export".to_string(),
                        },
                    ),
                ]),
            },
        );
        let remote = McpServerRecord::new(
            "a-remote",
            "Remote",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::StreamableHttp {
                url: "https://mcp.example.test/api".to_string(),
                allow_localhost: false,
                headers: BTreeMap::new(),
                auth: McpAuth::Static {
                    header_name: "Authorization".to_string(),
                    scheme: Some("Bearer".to_string()),
                    secret_ref: "mcp-secret:v1:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                    resolved_value: Some("oauth-token-must-not-export".to_string()),
                },
            },
        );

        let first = export_canonical_mcp_json(&[stdio.clone(), remote.clone()]).unwrap();
        let second = export_canonical_mcp_json(&[remote, stdio]).unwrap();

        assert_eq!(first, second);
        assert!(first.contains(OPENFLOW_MCP_EXPORT_FORMAT));
        assert!(first.contains("mcp-secret:v1:aaaaaaaa"));
        assert!(first.contains("mcp-secret:v1:bbbbbbbb"));
        assert!(!first.contains("resolved-must-not-export"));
        assert!(!first.contains("literal-must-not-export"));
        assert!(!first.contains("oauth-token-must-not-export"));
        assert!(first.find("a-remote").unwrap() < first.find("z-stdio").unwrap());
    }

    #[test]
    fn canonical_export_round_trips_redacted_normalized_records() {
        let record = McpServerRecord::new(
            "filesystem",
            "Filesystem",
            McpServerSource::Imported {
                dialect: "claude".to_string(),
                source_path: "/project/.mcp.json".to_string(),
            },
            McpInstall::External,
            McpConnection::Stdio {
                command: "npx".to_string(),
                args: vec!["server-filesystem".to_string()],
                environment: BTreeMap::from([(
                    "ROOT".to_string(),
                    PersistedValue::Literal {
                        value: "/private/project".to_string(),
                    },
                )]),
            },
        );

        let json = export_canonical_mcp_json(std::slice::from_ref(&record)).unwrap();
        let decoded: OpenFlowMcpExport = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.format, OPENFLOW_MCP_EXPORT_FORMAT);
        assert_eq!(decoded.schema_version, OPENFLOW_MCP_EXPORT_SCHEMA_VERSION);
        assert_eq!(decoded.servers, vec![record.redacted()]);
    }
}

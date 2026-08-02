//! ponytail: fixed path table — extend `candidate_paths` to add providers.

use crate::mcp::model::{
    McpAuth, McpConnection, McpInstall, McpServerRecord, McpServerSource, PersistedValue,
};
use crate::mcp::ports::mcp_secret_ref;
use crate::settings::model::{McpServerConfig, McpSettings};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Full config + provenance retained for explicit import preview.
struct ScannedServer {
    config: McpServerConfig,
    source: String,
    source_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpParseDiagnostic {
    pub server_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpParseResult {
    pub servers: Vec<McpServerConfig>,
    pub diagnostics: Vec<McpParseDiagnostic>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum McpImportError {
    #[error("invalid MCP JSON: {0}")]
    InvalidJson(String),
    #[error("MCP config must contain a supported servers object")]
    MissingServers,
    #[error("unterminated Markdown code fence")]
    UnterminatedFence,
    #[error("unsupported OpenFlow MCP export schema version `{0}`")]
    UnsupportedOpenFlowVersion(u64),
}

pub fn import_mcp_servers_json(content: &str) -> Result<McpParseResult, McpImportError> {
    let content = strip_optional_markdown_fence(content)?;
    let value: Value = serde_json::from_str(content)
        .map_err(|error| McpImportError::InvalidJson(error.to_string()))?;
    if value.get("format").and_then(Value::as_str)
        == Some(crate::adapters::mcp::OPENFLOW_MCP_EXPORT_FORMAT)
    {
        return parse_openflow_export(&value);
    }
    if let Some(servers) = value.get("servers").and_then(Value::as_object) {
        return Ok(parse_server_object(
            servers,
            "vscode",
            "",
            ServerObjectDialect::VsCode,
        ));
    }
    parse_mcp_servers_json_with_diagnostics(content).ok_or(McpImportError::MissingServers)
}

fn parse_openflow_export(value: &Value) -> Result<McpParseResult, McpImportError> {
    let version = value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or(McpImportError::MissingServers)?;
    if version != u64::from(crate::adapters::mcp::OPENFLOW_MCP_EXPORT_SCHEMA_VERSION) {
        return Err(McpImportError::UnsupportedOpenFlowVersion(version));
    }
    let servers = value
        .get("servers")
        .and_then(Value::as_array)
        .ok_or(McpImportError::MissingServers)?;
    let mut parsed = McpParseResult {
        servers: Vec::new(),
        diagnostics: Vec::new(),
    };
    for (index, value) in servers.iter().enumerate() {
        match serde_json::from_value::<McpServerRecord>(value.clone()) {
            Ok(mut server)
                if server.schema_version == crate::mcp::model::MCP_SERVER_RECORD_VERSION
                    && !server.id.trim().is_empty() =>
            {
                server.source = McpServerSource::Imported {
                    dialect: "openflow".to_string(),
                    source_path: String::new(),
                };
                server.enabled = false;
                server.trust = Default::default();
                parsed.servers.push(server);
            }
            Ok(server) => parsed.diagnostics.push(McpParseDiagnostic {
                server_id: server.id,
                message: "record has an unsupported schema version or empty ID".to_string(),
            }),
            Err(_) => parsed.diagnostics.push(McpParseDiagnostic {
                server_id: format!("servers[{index}]"),
                message: "record has an invalid OpenFlow MCP shape".to_string(),
            }),
        }
    }
    parsed.servers.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(parsed)
}

#[derive(Debug, Clone, Copy)]
enum ServerObjectDialect {
    Vendor,
    VsCode,
}

fn parse_server_object(
    servers: &serde_json::Map<String, Value>,
    dialect: &str,
    source_path: &str,
    object_dialect: ServerObjectDialect,
) -> McpParseResult {
    let mut parsed = McpParseResult {
        servers: Vec::new(),
        diagnostics: Vec::new(),
    };
    for (id, config) in servers {
        let Some(config) = config.as_object() else {
            parsed.diagnostics.push(McpParseDiagnostic {
                server_id: id.clone(),
                message: "entry has invalid object type".to_string(),
            });
            continue;
        };
        let connection = match object_dialect {
            ServerObjectDialect::Vendor => {
                parse_vendor_connection(id, config, &mut parsed.diagnostics)
            }
            ServerObjectDialect::VsCode => {
                parse_vscode_connection(id, config, &mut parsed.diagnostics)
            }
        };
        let Some(connection) = connection else {
            continue;
        };
        parsed.servers.push(McpServerRecord::new(
            id,
            id,
            McpServerSource::Imported {
                dialect: dialect.to_string(),
                source_path: source_path.to_string(),
            },
            McpInstall::External,
            connection,
        ));
    }
    parsed.servers.sort_by(|left, right| left.id.cmp(&right.id));
    parsed.diagnostics.sort_by(|left, right| {
        (&left.server_id, &left.message).cmp(&(&right.server_id, &right.message))
    });
    parsed
}

fn parse_vendor_connection(
    server_id: &str,
    config: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<McpParseDiagnostic>,
) -> Option<McpConnection> {
    let transport = config.get("type").and_then(Value::as_str);
    let has_command = config.contains_key("command");
    let has_url = config.contains_key("url");
    match transport {
        Some("stdio") | None if has_command => {
            parse_stdio_connection(server_id, config, diagnostics)
        }
        Some("http" | "streamable-http" | "streamableHttp" | "streamable_http") | None
            if has_url =>
        {
            parse_http_connection(server_id, config, diagnostics, false)
        }
        Some("sse") => parse_http_connection(server_id, config, diagnostics, true),
        Some(_) => {
            diagnostics.push(McpParseDiagnostic {
                server_id: server_id.to_string(),
                message: "field `type` has unsupported transport type".to_string(),
            });
            None
        }
        None => {
            diagnostics.push(McpParseDiagnostic {
                server_id: server_id.to_string(),
                message: "entry has no supported `command` or `url` field".to_string(),
            });
            None
        }
    }
}

fn parse_stdio_connection(
    server_id: &str,
    config: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<McpParseDiagnostic>,
) -> Option<McpConnection> {
    let Some(command) = config.get("command").and_then(Value::as_str) else {
        diagnostics.push(McpParseDiagnostic {
            server_id: server_id.to_string(),
            message: "field `command` has missing or invalid string type".to_string(),
        });
        return None;
    };
    if command.trim().is_empty() {
        diagnostics.push(McpParseDiagnostic {
            server_id: server_id.to_string(),
            message: "field `command` has empty string type".to_string(),
        });
        return None;
    }
    let args = parse_string_array(server_id, "args", config.get("args"), diagnostics);
    let environment = parse_persisted_values(server_id, "env", config.get("env"), diagnostics);
    Some(McpConnection::Stdio {
        command: command.to_string(),
        args,
        environment,
    })
}

fn parse_http_connection(
    server_id: &str,
    config: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<McpParseDiagnostic>,
    legacy_sse: bool,
) -> Option<McpConnection> {
    let Some(url) = config.get("url").and_then(Value::as_str) else {
        diagnostics.push(McpParseDiagnostic {
            server_id: server_id.to_string(),
            message: "field `url` has missing or invalid string type".to_string(),
        });
        return None;
    };
    if url.trim().is_empty() {
        diagnostics.push(McpParseDiagnostic {
            server_id: server_id.to_string(),
            message: "field `url` has empty string type".to_string(),
        });
        return None;
    }
    let headers = parse_persisted_values(server_id, "header", config.get("headers"), diagnostics);
    if legacy_sse {
        Some(McpConnection::LegacySse {
            url: url.to_string(),
            allow_localhost: false,
            headers,
            auth: McpAuth::None,
        })
    } else {
        Some(McpConnection::StreamableHttp {
            url: url.to_string(),
            allow_localhost: false,
            headers,
            auth: McpAuth::None,
        })
    }
}

fn parse_string_array(
    server_id: &str,
    field: &str,
    value: Option<&Value>,
    diagnostics: &mut Vec<McpParseDiagnostic>,
) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(values) = value.as_array() else {
        diagnostics.push(McpParseDiagnostic {
            server_id: server_id.to_string(),
            message: format!("field `{field}` has invalid array type"),
        });
        return Vec::new();
    };
    values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            value.as_str().map(str::to_string).or_else(|| {
                diagnostics.push(McpParseDiagnostic {
                    server_id: server_id.to_string(),
                    message: format!("field `{field}[{index}]` has invalid string type"),
                });
                None
            })
        })
        .collect()
}

fn parse_vscode_connection(
    server_id: &str,
    config: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<McpParseDiagnostic>,
) -> Option<McpConnection> {
    let transport = config
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("stdio");
    match transport {
        "http" | "streamable-http" | "streamableHttp" | "streamable_http" => {
            parse_http_connection(server_id, config, diagnostics, false)
        }
        "sse" => parse_http_connection(server_id, config, diagnostics, true),
        "stdio" => parse_stdio_connection(server_id, config, diagnostics),
        _ => {
            diagnostics.push(McpParseDiagnostic {
                server_id: server_id.to_string(),
                message: "field `type` has unsupported transport type".to_string(),
            });
            None
        }
    }
}

fn parse_persisted_values(
    server_id: &str,
    slot_kind: &str,
    value: Option<&Value>,
    diagnostics: &mut Vec<McpParseDiagnostic>,
) -> BTreeMap<String, PersistedValue> {
    let Some(value) = value else {
        return BTreeMap::new();
    };
    let Some(values) = value.as_object() else {
        diagnostics.push(McpParseDiagnostic {
            server_id: server_id.to_string(),
            message: format!("field `{slot_kind}s` has invalid object type"),
        });
        return BTreeMap::new();
    };
    values
        .iter()
        .filter_map(|(key, value)| {
            let Some(value) = value.as_str() else {
                diagnostics.push(McpParseDiagnostic {
                    server_id: server_id.to_string(),
                    message: format!("field `{slot_kind}.{key}` has invalid string type"),
                });
                return None;
            };
            let persisted = if input_placeholder(value).is_some() {
                match mcp_secret_ref(server_id, &format!("{slot_kind}.{key}")) {
                    Ok(secret_ref) => PersistedValue::Secret {
                        secret_ref: secret_ref.to_string(),
                        resolved_value: None,
                    },
                    Err(_) => {
                        diagnostics.push(McpParseDiagnostic {
                            server_id: server_id.to_string(),
                            message: format!(
                                "field `{slot_kind}.{key}` cannot form a secret reference"
                            ),
                        });
                        return None;
                    }
                }
            } else {
                PersistedValue::Literal {
                    value: value.to_string(),
                }
            };
            Some((key.clone(), persisted))
        })
        .collect()
}

fn input_placeholder(value: &str) -> Option<&str> {
    value
        .strip_prefix("${input:")
        .and_then(|value| value.strip_suffix('}'))
        .filter(|input_id| !input_id.is_empty() && !input_id.contains(['{', '}']))
}

fn strip_optional_markdown_fence(content: &str) -> Result<&str, McpImportError> {
    let trimmed = content.trim();
    let Some(after_fence) = trimmed.strip_prefix("```") else {
        return Ok(trimmed);
    };
    let Some((language, body)) = after_fence.split_once('\n') else {
        return Err(McpImportError::UnterminatedFence);
    };
    if !language.trim().is_empty() && !language.trim().eq_ignore_ascii_case("json") {
        return Err(McpImportError::InvalidJson(format!(
            "expected a JSON code fence, found `{}`",
            language.trim()
        )));
    }
    body.trim_end()
        .strip_suffix("```")
        .map(str::trim)
        .ok_or(McpImportError::UnterminatedFence)
}

pub fn parse_mcp_servers_json(content: &str) -> Option<Vec<McpServerConfig>> {
    parse_mcp_servers_json_with_diagnostics(content).map(|result| result.servers)
}

pub fn parse_mcp_servers_json_with_diagnostics(content: &str) -> Option<McpParseResult> {
    let value: Value = serde_json::from_str(content).ok()?;
    let servers_obj = value.get("mcpServers").and_then(|v| v.as_object())?;
    Some(parse_server_object(
        servers_obj,
        "mcpServers",
        "",
        ServerObjectDialect::Vendor,
    ))
}

fn parse_config_path(path: &Path, source: &str, root: &Path) -> io::Result<McpParseResult> {
    let content = std::fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&content).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} contains invalid MCP JSON: {error}", path.display()),
        )
    })?;
    parse_config_value(&value, source, &path.display().to_string(), root).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} has no supported servers object", path.display()),
        )
    })
}

fn parse_config_value(
    value: &Value,
    source: &str,
    source_path: &str,
    root: &Path,
) -> Option<McpParseResult> {
    let mut records = BTreeMap::<String, McpServerConfig>::new();
    let mut diagnostics = Vec::new();
    if let Some(servers) = value.get("mcpServers").and_then(Value::as_object) {
        let parsed = parse_server_object(servers, source, source_path, ServerObjectDialect::Vendor);
        records.extend(
            parsed
                .servers
                .into_iter()
                .map(|server| (server.id.clone(), server)),
        );
        diagnostics.extend(parsed.diagnostics);
    }
    if let Some(servers) = value.get("servers").and_then(Value::as_object) {
        let parsed =
            parse_server_object(servers, "vscode", source_path, ServerObjectDialect::VsCode);
        records.extend(
            parsed
                .servers
                .into_iter()
                .map(|server| (server.id.clone(), server)),
        );
        diagnostics.extend(parsed.diagnostics);
    }
    if source == "claude" {
        let root_key = absolute_path(root).display().to_string();
        if let Some(servers) = value
            .get("projects")
            .and_then(Value::as_object)
            .and_then(|projects| projects.get(&root_key))
            .and_then(|project| project.get("mcpServers"))
            .and_then(Value::as_object)
        {
            let parsed = parse_server_object(
                servers,
                "claude-project",
                source_path,
                ServerObjectDialect::Vendor,
            );
            records.extend(
                parsed
                    .servers
                    .into_iter()
                    .map(|server| (server.id.clone(), server)),
            );
            diagnostics.extend(parsed.diagnostics);
        }
    }
    if records.is_empty() && diagnostics.is_empty() {
        return None;
    }
    diagnostics.sort_by(|left, right| {
        (&left.server_id, &left.message).cmp(&(&right.server_id, &right.message))
    });
    Some(McpParseResult {
        servers: records.into_values().collect(),
        diagnostics,
    })
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

pub fn load_mcp_server_from_path(path: &Path, server_id: &str) -> io::Result<McpServerConfig> {
    let parsed = parse_config_path(path, source_hint_for_path(path), Path::new("."))?;
    parsed
        .servers
        .into_iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "MCP server `{server_id}` was not found in {}",
                    path.display()
                ),
            )
        })
}

fn source_hint_for_path(path: &Path) -> &str {
    let path = path.to_string_lossy();
    if path.contains(".claude") || path.contains("Claude/claude_desktop_config.json") {
        "claude"
    } else if path.contains(".cursor") {
        "cursor"
    } else if path.contains(".vscode") || path.contains("Code/User") {
        "vscode"
    } else if path.contains(".flow") {
        "openflow"
    } else {
        "mcp-json"
    }
}

pub fn hydrate_mcp_server_from_path(
    path: &Path,
    mut incoming: McpServerConfig,
) -> io::Result<McpServerConfig> {
    let discovered = load_mcp_server_from_path(path, &incoming.id)?;
    merge_discovered_env(&mut incoming, &discovered);
    Ok(incoming)
}

fn merge_discovered_env(incoming: &mut McpServerConfig, discovered: &McpServerConfig) {
    let (
        McpConnection::Stdio {
            environment: incoming,
            ..
        },
        McpConnection::Stdio {
            environment: discovered,
            ..
        },
    ) = (&mut incoming.connection, &discovered.connection)
    else {
        return;
    };
    for (key, discovered_value) in discovered {
        let incoming_value =
            incoming
                .entry(key.clone())
                .or_insert_with(|| PersistedValue::Literal {
                    value: String::new(),
                });
        if matches!(incoming_value, PersistedValue::Literal { value } if value.trim().is_empty()) {
            incoming_value.clone_from(discovered_value);
        }
    }
}

fn candidate_paths(home: &Path, root: &Path) -> Vec<(String, PathBuf)> {
    vec![
        ("cursor".into(), home.join(".cursor/mcp.json")),
        ("cursor".into(), root.join(".cursor/mcp.json")),
        ("claude".into(), home.join(".claude.json")),
        ("claude".into(), home.join(".claude/mcp.json")),
        ("claude".into(), root.join(".claude/.mcp.json")),
        ("claude".into(), root.join(".claude/mcp.json")),
        ("claude-project".into(), root.join(".mcp.json")),
        (
            "claude-desktop".into(),
            home.join("Library/Application Support/Claude/claude_desktop_config.json"),
        ),
        ("vscode".into(), root.join(".vscode/mcp.json")),
        (
            "vscode".into(),
            home.join("Library/Application Support/Code/User/mcp.json"),
        ),
        ("vscode".into(), home.join(".config/Code/User/mcp.json")),
        (
            "vscode".into(),
            home.join("AppData/Roaming/Code/User/mcp.json"),
        ),
        ("mcp-json".into(), root.join("mcp.json")),
        ("openflow".into(), root.join(".flow/mcp.json")),
    ]
}

fn scan_scanned_servers(settings: &McpSettings, root: &Path) -> BTreeMap<String, ScannedServer> {
    if !settings.discover_external {
        return BTreeMap::new();
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut by_id: BTreeMap<String, ScannedServer> = BTreeMap::new();

    for (source, path) in candidate_paths(&home, root) {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(parsed) = parse_mcp_servers_json_with_diagnostics(&content) else {
            continue;
        };
        for diagnostic in parsed.diagnostics {
            log::warn!(
                "skipping MCP server `{}` from {}: {}",
                diagnostic.server_id,
                path.display(),
                diagnostic.message
            );
        }
        for config in parsed.servers {
            let mut config = config;
            config.source = McpServerSource::Imported {
                dialect: source.clone(),
                source_path: path.display().to_string(),
            };
            by_id.insert(
                config.id.clone(),
                ScannedServer {
                    config,
                    source: source.clone(),
                    source_path: path.clone(),
                },
            );
        }
    }
    by_id
}

pub fn scan_external_mcp_for_api(
    settings: &McpSettings,
    root: &Path,
) -> Vec<crate::api::McpDiscoveryRow> {
    scan_scanned_servers(settings, root)
        .into_values()
        .map(|row| {
            let (command, args, env_keys) = match &row.config.connection {
                McpConnection::Stdio {
                    command,
                    args,
                    environment,
                } => (
                    command.clone(),
                    args.clone(),
                    environment.keys().cloned().collect(),
                ),
                McpConnection::StreamableHttp { url, headers, .. }
                | McpConnection::LegacySse { url, headers, .. } => {
                    (url.clone(), Vec::new(), headers.keys().cloned().collect())
                }
            };
            crate::api::McpDiscoveryRow {
                id: row.config.id,
                display_name: row.config.display_name,
                command,
                args,
                env_keys,
                enabled: false,
                source: row.source,
                source_path: row.source_path.display().to_string(),
            }
        })
        .collect()
}

pub fn effective_mcp_servers(settings: &McpSettings, root: &Path) -> Vec<McpServerConfig> {
    let _ = root;
    settings.servers.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stdio_parts(
        server: &McpServerConfig,
    ) -> (&str, &[String], &BTreeMap<String, PersistedValue>) {
        let McpConnection::Stdio {
            command,
            args,
            environment,
        } = &server.connection
        else {
            panic!("expected stdio server");
        };
        (command, args, environment)
    }

    #[test]
    fn parse_mcp_servers_json_parses_stdio_entries() {
        let json = r#"{"mcpServers":{"gh":{"command":"npx","args":["-y","pkg"]}}}"#;
        let servers = super::parse_mcp_servers_json(json).expect("parse");
        assert_eq!(servers[0].id, "gh");
        assert_eq!(stdio_parts(&servers[0]).0, "npx");
        assert!(!servers[0].enabled);
    }

    #[test]
    fn import_vscode_http_maps_input_placeholder_to_opaque_secret_ref() {
        let json = r#"{
            "inputs": [
                {"id": "api-token", "type": "promptString", "password": true}
            ],
            "servers": {
                "remote": {
                    "type": "streamable-http",
                    "url": "https://mcp.example.test/api",
                    "headers": {"Authorization": "${input:api-token}"}
                }
            }
        }"#;

        let parsed = import_mcp_servers_json(json).expect("import VS Code config");

        assert!(parsed.diagnostics.is_empty());
        assert_eq!(parsed.servers.len(), 1);
        assert_eq!(
            parsed.servers[0].source,
            McpServerSource::Imported {
                dialect: "vscode".to_string(),
                source_path: String::new(),
            }
        );
        assert!(!parsed.servers[0].enabled);
        assert_eq!(parsed.servers[0].trust, Default::default());
        let McpConnection::StreamableHttp {
            url, headers, auth, ..
        } = &parsed.servers[0].connection
        else {
            panic!("expected Streamable HTTP");
        };
        assert_eq!(url, "https://mcp.example.test/api");
        assert_eq!(auth, &McpAuth::None);
        assert_eq!(
            headers["Authorization"],
            PersistedValue::Secret {
                secret_ref: mcp_secret_ref("remote", "header.Authorization")
                    .unwrap()
                    .to_string(),
                resolved_value: None,
            }
        );
    }

    #[test]
    fn claude_project_map_selects_only_the_exact_absolute_root() {
        let fixture = std::env::temp_dir().join(format!(
            "openflow-claude-project-import-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let root = fixture.join("selected-project");
        let other_root = fixture.join("other-project");
        let config_path = fixture.join(".claude.json");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&other_root).unwrap();
        let json = serde_json::json!({
            "mcpServers": {
                "global": {"command": "global-server"}
            },
            "projects": {
                root.to_string_lossy(): {
                    "mcpServers": {
                        "selected": {"command": "selected-server"}
                    }
                },
                other_root.to_string_lossy(): {
                    "mcpServers": {
                        "other": {"command": "other-server"}
                    }
                }
            }
        });
        std::fs::write(&config_path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();

        let parsed = parse_config_path(&config_path, "claude", &root).expect("parse Claude file");

        assert_eq!(
            parsed
                .servers
                .iter()
                .map(|server| server.id.as_str())
                .collect::<Vec<_>>(),
            vec!["global", "selected"]
        );
        assert_eq!(
            parsed.servers[0].source,
            McpServerSource::Imported {
                dialect: "claude".to_string(),
                source_path: config_path.display().to_string(),
            }
        );
        assert_eq!(
            parsed.servers[1].source,
            McpServerSource::Imported {
                dialect: "claude-project".to_string(),
                source_path: config_path.display().to_string(),
            }
        );
        assert!(parsed.servers.iter().all(|server| !server.enabled));
        assert!(parsed
            .servers
            .iter()
            .all(|server| server.trust == Default::default()));

        std::fs::remove_dir_all(&fixture).ok();
    }

    #[test]
    fn candidate_paths_cover_existing_and_multi_dialect_locations() {
        let home = Path::new("/fixture/home");
        let root = Path::new("/fixture/project");
        let candidates = candidate_paths(home, root);

        for expected in [
            home.join(".cursor/mcp.json"),
            root.join(".cursor/mcp.json"),
            home.join(".claude.json"),
            home.join(".claude/mcp.json"),
            root.join(".claude/.mcp.json"),
            root.join(".claude/mcp.json"),
            root.join("mcp.json"),
            root.join(".flow/mcp.json"),
            root.join(".mcp.json"),
            home.join("Library/Application Support/Claude/claude_desktop_config.json"),
            root.join(".vscode/mcp.json"),
            home.join("Library/Application Support/Code/User/mcp.json"),
        ] {
            assert!(
                candidates.iter().any(|(_, path)| path == &expected),
                "missing candidate {}",
                expected.display()
            );
        }
    }

    #[test]
    fn claude_remote_http_and_sse_entries_keep_headers_and_provenance() {
        let fixture = std::env::temp_dir().join(format!(
            "openflow-claude-remote-import-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let root = fixture.join("project");
        let config_path = fixture.join("claude_desktop_config.json");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            &config_path,
            r#"{
                "mcpServers": {
                    "hosted": {
                        "url": "https://mcp.example.test/api",
                        "headers": {"X-Tenant": "team-a"}
                    },
                    "legacy": {
                        "type": "sse",
                        "url": "https://mcp.example.test/events",
                        "headers": {"X-Tenant": "team-b"}
                    }
                }
            }"#,
        )
        .unwrap();

        let parsed = parse_config_path(&config_path, "claude-desktop", &root).unwrap();

        assert_eq!(parsed.servers.len(), 2);
        let McpConnection::StreamableHttp { url, headers, .. } = &parsed.servers[0].connection
        else {
            panic!("expected hosted Streamable HTTP server");
        };
        assert_eq!(url, "https://mcp.example.test/api");
        assert_eq!(
            headers["X-Tenant"],
            PersistedValue::Literal {
                value: "team-a".to_string()
            }
        );
        assert!(matches!(
            parsed.servers[1].connection,
            McpConnection::LegacySse { .. }
        ));
        assert!(parsed.servers.iter().all(|server| {
            server.source
                == McpServerSource::Imported {
                    dialect: "claude-desktop".to_string(),
                    source_path: config_path.display().to_string(),
                }
                && !server.enabled
                && server.trust == Default::default()
        }));

        std::fs::remove_dir_all(&fixture).ok();
    }

    #[test]
    fn import_mcp_servers_json_accepts_fenced_vendor_config_and_preserves_env() {
        let json = r#"```json
        {
          "mcpServers": {
            "massive": {
              "command": "mcp_massive",
              "env": {"MASSIVE_API_KEY": "secret"}
            }
          }
        }
        ```"#;

        let parsed = super::import_mcp_servers_json(json).expect("import config");

        assert_eq!(parsed.servers.len(), 1);
        assert_eq!(parsed.servers[0].id, "massive");
        assert_eq!(stdio_parts(&parsed.servers[0]).0, "mcp_massive");
        assert_eq!(
            stdio_parts(&parsed.servers[0]).2["MASSIVE_API_KEY"],
            PersistedValue::Literal {
                value: "secret".to_string()
            }
        );
        assert!(parsed.diagnostics.is_empty());
    }

    #[test]
    fn import_openflow_export_revokes_trust_and_keeps_only_secret_refs() {
        let mut server = McpServerRecord::new(
            "massive",
            "Massive",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: "mcp-massive".to_string(),
                args: Vec::new(),
                environment: BTreeMap::from([(
                    "API_KEY".to_string(),
                    PersistedValue::Secret {
                        secret_ref: "mcp-secret:v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_string(),
                        resolved_value: Some("must-not-export".to_string()),
                    },
                )]),
            },
        );
        server.enabled = true;
        server.trust.approved_fingerprint = Some("approved".to_string());
        let exported = crate::adapters::mcp::export_canonical_mcp_json(&[server]).unwrap();

        let parsed = import_mcp_servers_json(&exported).unwrap();

        assert_eq!(parsed.servers.len(), 1);
        assert!(!parsed.servers[0].enabled);
        assert!(parsed.servers[0].trust.approved_fingerprint.is_none());
        assert!(matches!(
            parsed.servers[0].source,
            McpServerSource::Imported { ref dialect, .. } if dialect == "openflow"
        ));
        assert!(!exported.contains("must-not-export"));
    }

    #[test]
    fn parse_mcp_servers_json_skips_invalid_entries_and_keeps_supported_servers() {
        let json = r#"{
            "mcpServers": {
                "valid": {"command": "server", "args": ["--stdio"]},
                "url-only": {"url": "https://example.test/mcp"},
                "not-an-object": "invalid",
                "missing-command": {"args": []}
            }
        }"#;

        let parsed = super::parse_mcp_servers_json_with_diagnostics(json).expect("parse");

        assert_eq!(parsed.servers.len(), 2);
        assert_eq!(
            parsed
                .servers
                .iter()
                .map(|server| server.id.as_str())
                .collect::<Vec<_>>(),
            vec!["url-only", "valid"]
        );
        assert_eq!(parsed.diagnostics.len(), 2);
    }

    #[test]
    fn discovered_mcp_is_inventory_only_and_manual_record_wins_at_runtime() {
        let dir = std::env::temp_dir().join(format!("mcp-discover-test-{}", std::process::id()));
        let home = dir.join("home");
        let mcp_path = dir.join(".cursor/mcp.json");
        std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        // ponytail: isolate HOME so developer ~/.cursor/mcp.json does not affect the test.
        std::env::set_var("HOME", &home);
        std::fs::write(
            &mcp_path,
            r#"{"mcpServers":{"gh":{"command":"npx","args":["discovered"],"env":{"TOKEN":"secret"}}}}"#,
        )
        .unwrap();

        let loaded = load_mcp_server_from_path(&mcp_path, "gh").expect("load discovered server");
        assert_eq!(
            stdio_parts(&loaded).2["TOKEN"],
            PersistedValue::Literal {
                value: "secret".to_string()
            }
        );

        let settings = McpSettings {
            servers: vec![McpServerRecord {
                enabled: true,
                ..McpServerRecord::new(
                    "gh",
                    "Manual",
                    McpServerSource::Manual,
                    McpInstall::External,
                    McpConnection::Stdio {
                        command: "manual".into(),
                        args: vec!["manual".into()],
                        environment: BTreeMap::from([(
                            "TOKEN".into(),
                            PersistedValue::Literal {
                                value: String::new(),
                            },
                        )]),
                    },
                )
            }],
            discover_external: true,
            disabled_discovered_ids: vec![],
            registry_base_url: McpSettings::default().registry_base_url,
        };

        let hydrated = hydrate_mcp_server_from_path(&mcp_path, settings.servers[0].clone())
            .expect("hydrate discovered auth");
        assert_eq!(stdio_parts(&hydrated).0, "manual");
        assert_eq!(
            stdio_parts(&hydrated).2["TOKEN"],
            PersistedValue::Literal {
                value: "secret".to_string()
            }
        );

        let discovered = scan_external_mcp_for_api(&settings, &dir);
        assert_eq!(discovered[0].env_keys, vec!["TOKEN"]);

        let effective = effective_mcp_servers(&settings, &dir);
        assert_eq!(effective.len(), 1);
        assert_eq!(stdio_parts(&effective[0]).0, "manual");
        assert_eq!(
            stdio_parts(&effective[0]).2["TOKEN"],
            PersistedValue::Literal {
                value: String::new()
            },
            "runtime must not hydrate values from unreviewed external inventory"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[ignore = "depends on developer ~/.cursor/mcp.json listing playwright"]
    fn scan_finds_playwright_from_cursor_home_config() {
        let home = dirs::home_dir().expect("home dir");
        let cursor_mcp = home.join(".cursor/mcp.json");
        if !cursor_mcp.is_file() {
            return;
        }
        let settings = McpSettings::default();
        let rows = scan_external_mcp_for_api(&settings, Path::new("."));
        assert!(
            rows.iter().any(|row| row.id == "playwright"),
            "expected playwright in {:?}",
            rows
        );
    }
}

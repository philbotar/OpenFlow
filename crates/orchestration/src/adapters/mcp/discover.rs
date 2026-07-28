//! ponytail: fixed path table — extend `candidate_paths` to add providers.

use crate::settings::model::{McpServerConfig, McpSettings};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Full config + provenance. UI/API map drops `env`.
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

pub fn parse_mcp_servers_json(content: &str) -> Option<Vec<McpServerConfig>> {
    parse_mcp_servers_json_with_diagnostics(content).map(|result| result.servers)
}

pub fn parse_mcp_servers_json_with_diagnostics(content: &str) -> Option<McpParseResult> {
    let value: Value = serde_json::from_str(content).ok()?;
    let servers_obj = value.get("mcpServers").and_then(|v| v.as_object())?;
    let mut out = Vec::new();
    let mut diagnostics = Vec::new();
    for (id, cfg) in servers_obj {
        let Some(obj) = cfg.as_object() else {
            diagnostics.push(McpParseDiagnostic {
                server_id: id.clone(),
                message: "entry is not an object".to_string(),
            });
            continue;
        };
        let Some(command) = obj.get("command").and_then(|v| v.as_str()) else {
            diagnostics.push(McpParseDiagnostic {
                server_id: id.clone(),
                message: "entry has no stdio command".to_string(),
            });
            continue;
        };
        if command.trim().is_empty() {
            diagnostics.push(McpParseDiagnostic {
                server_id: id.clone(),
                message: "stdio command is empty".to_string(),
            });
            continue;
        }
        let args = obj
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let env = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        out.push(McpServerConfig {
            id: id.clone(),
            display_name: id.clone(),
            command: command.to_string(),
            args,
            env,
            enabled: obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        });
    }
    Some(McpParseResult {
        servers: out,
        diagnostics,
    })
}

fn candidate_paths(home: &Path, root: &Path) -> Vec<(String, PathBuf)> {
    vec![
        ("cursor".into(), home.join(".cursor/mcp.json")),
        ("cursor".into(), root.join(".cursor/mcp.json")),
        ("claude".into(), home.join(".claude.json")),
        ("claude".into(), home.join(".claude/mcp.json")),
        ("claude".into(), root.join(".claude/.mcp.json")),
        ("claude".into(), root.join(".claude/mcp.json")),
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
            if config.command.is_empty() {
                continue;
            }
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
            let enabled = row.config.enabled
                && !settings
                    .disabled_discovered_ids
                    .iter()
                    .any(|id| id == &row.config.id);
            crate::api::McpDiscoveryRow {
                id: row.config.id,
                display_name: row.config.display_name,
                command: row.config.command,
                args: row.config.args,
                enabled,
                source: row.source,
                source_path: row.source_path.display().to_string(),
            }
        })
        .collect()
}

pub fn effective_mcp_servers(settings: &McpSettings, root: &Path) -> Vec<McpServerConfig> {
    let mut servers: BTreeMap<String, McpServerConfig> = BTreeMap::new();

    for row in scan_scanned_servers(settings, root).into_values() {
        let enabled = row.config.enabled
            && !settings
                .disabled_discovered_ids
                .iter()
                .any(|id| id == &row.config.id);
        servers.insert(
            row.config.id.clone(),
            McpServerConfig {
                enabled,
                ..row.config
            },
        );
    }

    for manual in &settings.servers {
        servers.insert(manual.id.clone(), manual.clone());
    }

    servers.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mcp_servers_json_parses_stdio_entries() {
        let json = r#"{"mcpServers":{"gh":{"command":"npx","args":["-y","pkg"]}}}"#;
        let servers = super::parse_mcp_servers_json(json).expect("parse");
        assert_eq!(servers[0].id, "gh");
        assert_eq!(servers[0].command, "npx");
    }

    #[test]
    fn parse_mcp_servers_json_skips_invalid_entries_and_keeps_valid_servers() {
        let json = r#"{
            "mcpServers": {
                "valid": {"command": "server", "args": ["--stdio"]},
                "url-only": {"url": "https://example.test/mcp"},
                "not-an-object": "invalid",
                "missing-command": {"args": []}
            }
        }"#;

        let parsed = super::parse_mcp_servers_json_with_diagnostics(json).expect("parse");

        assert_eq!(parsed.servers.len(), 1);
        assert_eq!(parsed.servers[0].id, "valid");
        assert_eq!(parsed.diagnostics.len(), 3);
    }

    #[test]
    fn effective_mcp_servers_manual_wins_on_id_collision() {
        let dir = std::env::temp_dir().join(format!("mcp-discover-test-{}", std::process::id()));
        let home = dir.join("home");
        let mcp_path = dir.join(".cursor/mcp.json");
        std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        // ponytail: isolate HOME so developer ~/.cursor/mcp.json does not affect the test.
        std::env::set_var("HOME", &home);
        std::fs::write(
            &mcp_path,
            r#"{"mcpServers":{"gh":{"command":"npx","args":["discovered"]}}}"#,
        )
        .unwrap();

        let settings = McpSettings {
            servers: vec![McpServerConfig {
                id: "gh".into(),
                display_name: "Manual".into(),
                command: "manual".into(),
                args: vec!["manual".into()],
                env: Default::default(),
                enabled: true,
            }],
            discover_external: true,
            disabled_discovered_ids: vec![],
        };

        let effective = effective_mcp_servers(&settings, &dir);
        assert_eq!(effective.len(), 1);
        assert_eq!(effective[0].command, "manual");

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

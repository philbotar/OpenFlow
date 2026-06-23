# MCP External Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-discover stdio MCP servers from common external configs (Cursor, Claude Code, project `mcp.json`), merge with manual servers at run start, and show discovered rows in Settings.

**Architecture:** One `discover.rs` module — fixed path table, one `scan_external_mcp()` pass, stdio filter only at connect. Manual `settings.mcp.servers` wins on `id` collision. `effective_mcp_servers()` returns `Vec<McpServerConfig>` for `McpRunClients::connect`. No separate `discover_mcp_servers` IPC: `load_settings` and `bootstrap_app` return `discovered_mcp` alongside persisted settings (one scan, no duplicate UI path). No HTTP/SSE rows in UI or scan results.

**Tech Stack:** Rust (`serde_json`, `std::fs`), Tauri v2, SolidJS.

**Reference (external):** OMP `oh-my-pi/packages/coding-agent/src/discovery/{cursor,claude,mcp-json}.ts` — v1 copies only those three, not the full 8-provider table.

**Ponytail audit applied:** cut windsurf/gemini/omp/vscode paths, dual DTOs, dual scan, HTTP UI, import button, `tempfile` dep, 8-commit ceremony.

---

## Current gaps

| Layer | Today | After this plan |
| --- | --- | --- |
| `adapters/mcp/` | Manual stdio client only | + `discover.rs` scan + merge |
| `settings/model.rs` | `mcp.servers` only | + `discover_external`, `disabled_discovered_ids` |
| `run/execution/drive.rs` | Connects raw `mcp` settings | `effective_mcp_servers()` before connect |
| `backend/mod.rs` | `load_settings` → `AppSettings` only | → `SettingsLoadPayload` with `discovered_mcp` |
| `ui/settings/McpSection.tsx` | Manual add/edit only | Discovered subsection (read-only + enable toggle) |

**Out of scope (defer):**

- HTTP/SSE MCP (stdio only; url-only entries skipped silently)
- Extra providers: VS Code, Windsurf, Gemini, OMP (add paths to table when requested)
- `tools.discoveryMode` enum, BM25 discovery, MCP client pool
- Import button (use manual add form; toggle enable via `disabledDiscoveredIds`)
- Env var expansion in JSON (`${VAR}`) — v1 reads literals; expand when a real config breaks

---

## Discovery sources (v1 — 8 paths)

Last path wins on duplicate `id` during scan. Manual `settings.mcp.servers` always wins on merge.

| Source label | Path |
| --- | --- |
| `cursor` | `~/.cursor/mcp.json` |
| `cursor` | `{root}/.cursor/mcp.json` |
| `claude` | `~/.claude.json` |
| `claude` | `~/.claude/mcp.json` |
| `claude` | `{root}/.claude/.mcp.json` |
| `claude` | `{root}/.claude/mcp.json` |
| `mcp-json` | `{root}/mcp.json` |
| `openflow` | `{root}/.flow/mcp.json` |

`{root}` = active project path when provided, else `std::env::current_dir()`.

JSON shapes: `mcpServers` object (Cursor/Claude/standalone). Skip entries with `url` and no `command`.

---

## File map

| File | Responsibility |
| --- | --- |
| `crates/orchestration/src/adapters/mcp/discover.rs` | Path table, parse, `scan_external_mcp`, `effective_mcp_servers` |
| `crates/orchestration/src/adapters/mcp/mod.rs` | `mod discover;` re-exports |
| `crates/orchestration/src/settings/model.rs` | `discover_external`, `disabled_discovered_ids` |
| `crates/orchestration/src/api.rs` | `SettingsLoadPayload`, `McpDiscoveryRow` |
| `crates/orchestration/src/backend/mod.rs` | `load_settings` builds payload + scan |
| `crates/orchestration/src/run/execution/drive.rs` | `effective_mcp_servers` before connect |
| `crates/desktop/src/lib.rs` | `load_settings`, `bootstrap_app` payload shapes |
| `crates/ui/src/lib/types/`, `port.ts`, `api.ts` | Payload types + seam |
| `crates/ui/src/context/AppProvider.tsx` | `discoveredMcp` signal from bootstrap/load |
| `crates/ui/src/settings/McpSection.tsx` | Discovered UI |
| `CHANGELOG.md` | Entry |

---

## V1 behavior

- `discover_external` defaults `true`; when `false`, scan returns empty and connect uses manual servers only.
- Discovered stdio: `enabled = true` unless JSON `enabled: false` or id ∈ `disabled_discovered_ids`.
- `McpDiscoveryRow`: `id`, `displayName`, `command`, `args`, `enabled`, `source`, `sourcePath` — no `env`, no `supported`, no `imported`.
- Enable toggle on discovered row updates `disabledDiscoveredIds` in settings (no import).
- Probe + run use merged effective server list.

---

### Task 1: Schema + discovery module

**Files:**
- Modify: `crates/orchestration/src/settings/model.rs`
- Create: `crates/orchestration/src/adapters/mcp/discover.rs`
- Modify: `crates/orchestration/src/adapters/mcp/mod.rs`
- Test: inline in `discover.rs` + `model.rs`

- [ ] **Step 1: Failing settings round-trip test**

```rust
#[test]
fn mcp_discovery_settings_round_trip() {
    let settings = AppSettings {
        mcp: McpSettings {
            servers: vec![],
            discover_external: true,
            disabled_discovered_ids: vec!["playwright".into()],
        },
        ..AppSettings::default()
    };
    let json = serde_json::to_string(&settings).unwrap();
    let parsed: AppSettings = serde_json::from_str(&json).unwrap();
    assert!(parsed.mcp.discover_external);
    assert_eq!(parsed.mcp.disabled_discovered_ids, ["playwright"]);
}
```

Run: `cargo test -p orchestration mcp_discovery_settings_round_trip` — expect FAIL.

- [ ] **Step 2: Extend `McpSettings`**

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSettings {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
    #[serde(default = "default_true")]
    pub discover_external: bool,
    #[serde(default)]
    pub disabled_discovered_ids: Vec<String>,
}
```

- [ ] **Step 3: Failing parser + merge tests**

```rust
#[test]
fn parse_mcp_servers_json() {
    let json = r#"{"mcpServers":{"gh":{"command":"npx","args":["-y","pkg"]}}}"#;
    let servers = parse_mcp_servers_json(json).expect("parse");
    assert_eq!(servers[0].id, "gh");
    assert_eq!(servers[0].command, "npx");
}

#[test]
fn effective_mcp_servers_manual_wins_on_id_collision() {
    let dir = std::env::temp_dir().join(format!("mcp-discover-test-{}", std::process::id()));
    let mcp_path = dir.join(".cursor/mcp.json");
    std::fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
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
```

Run: `cargo test -p orchestration parse_mcp_servers_json effective_mcp_servers` — expect FAIL.

- [ ] **Step 4: Implement `discover.rs`**

```rust
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

pub fn parse_mcp_servers_json(content: &str) -> Option<Vec<McpServerConfig>> {
    let value: Value = serde_json::from_str(content).ok()?;
    let servers_obj = value.get("mcpServers").and_then(|v| v.as_object())?;
    let mut out = Vec::new();
    for (id, cfg) in servers_obj {
        let obj = cfg.as_object()?;
        let command = obj.get("command").and_then(|v| v.as_str())?;
        if obj.get("url").and_then(|v| v.as_str()).is_some() && command.is_empty() {
            continue;
        }
        let args = obj
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let env = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.clone())))
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
    Some(out)
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
        let content = std::fs::read_to_string(&path).ok();
        if content.is_none() {
            continue;
        }
        let parsed = parse_mcp_servers_json(&content.unwrap());
        if parsed.is_none() {
            continue;
        }
        for config in parsed.unwrap() {
            if config.command.is_empty() {
                continue;
            }
            by_id.insert(
                config.id.clone(),
                ScannedServer {
                    config,
                    source,
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
                && !settings.disabled_discovered_ids.iter().any(|id| id == &row.config.id);
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
            && !settings.disabled_discovered_ids.iter().any(|id| id == &row.config.id);
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
```

- [ ] **Step 5: Run tests — expect PASS**

Run: `cargo test -p orchestration mcp_discovery parse_mcp effective_mcp`

- [ ] **Step 6: Commit**

```bash
git add crates/orchestration/src/settings/model.rs \
  crates/orchestration/src/adapters/mcp/discover.rs \
  crates/orchestration/src/adapters/mcp/mod.rs
git commit -m "feat(mcp): scan external MCP configs and merge effective servers"
```

---

### Task 2: Backend payload + run wire

**Files:**
- Modify: `crates/orchestration/src/api.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/orchestration/src/run/execution/drive.rs`
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Add IPC types**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDiscoveryRow {
    pub id: String,
    pub display_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub source: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsLoadPayload {
    pub settings: AppSettings,
    pub discovered_mcp: Vec<McpDiscoveryRow>,
}
```

Map internal `discover::McpDiscoveryRow` → `api::McpDiscoveryRow` in backend (path to string).

- [ ] **Step 2: Change `load_settings`**

```rust
pub fn load_settings(&self, project_path: Option<&str>) -> Result<SettingsLoadPayload, BackendError> {
    let settings = self.settings.load()?;
    let root = project_path
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let discovered_mcp = crate::adapters::mcp::scan_external_mcp_for_api(&settings.mcp, &root);
    Ok(SettingsLoadPayload {
        settings: settings.redacted(),
        discovered_mcp,
    })
}
```

Add `scan_external_mcp_for_api` in `discover.rs` — thin map from internal scan to API row.

- [ ] **Step 3: Extend `bootstrap_app`**

Add `discovered_mcp: Vec<McpDiscoveryRow>` to `BootstrapPayload`; populate via same scan helper using `std::env::current_dir()`.

- [ ] **Step 4: Wire `drive.rs`**

```rust
let effective_servers =
    crate::adapters::mcp::effective_mcp_servers(&mcp, &execution_cwd);
let effective_mcp = McpSettings {
    servers: effective_servers,
    discover_external: mcp.discover_external,
    disabled_discovered_ids: mcp.disabled_discovered_ids.clone(),
};
let mcp_clients = match crate::adapters::mcp::McpRunClients::connect(&effective_mcp).await {
```

Or change `McpRunClients::connect` to accept `&[McpServerConfig]` — smaller API.

- [ ] **Step 5: Update desktop commands**

`load_settings` returns `SettingsLoadPayload`. `bootstrap_app` includes `discovered_mcp`.

Run: `cargo test -p orchestration && cargo build -p desktop`

- [ ] **Step 6: Commit**

```bash
git add crates/orchestration/src/api.rs crates/orchestration/src/backend/mod.rs \
  crates/orchestration/src/run/execution/drive.rs crates/desktop/src/lib.rs \
  crates/orchestration/src/adapters/mcp/discover.rs
git commit -m "feat(mcp): settings payload includes discovered servers; merge at run start"
```

---

### Task 3: UI + changelog

**Files:**
- Modify: `crates/ui/src/lib/types/index.ts`
- Modify: `crates/ui/src/port.ts`, `api.ts`
- Modify: `crates/ui/src/context/AppProvider.tsx`
- Modify: `crates/ui/src/settings/McpSection.tsx`
- Modify: `crates/ui/src/settings/McpSection.test.tsx`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Types + seam**

```typescript
export interface McpDiscoveryRow {
  id: string;
  displayName: string;
  command: string;
  args: string[];
  enabled: boolean;
  source: string;
  sourcePath: string;
}

export interface SettingsLoadPayload {
  settings: AppSettings;
  discoveredMcp: McpDiscoveryRow[];
}

export interface McpSettings {
  servers: McpServerConfig[];
  discoverExternal?: boolean;
  disabledDiscoveredIds?: string[];
}

// port.ts
loadSettings: (projectPath?: string) => Promise<SettingsLoadPayload>;

// BootstrapPayload adds discoveredMcp: McpDiscoveryRow[]
```

- [ ] **Step 2: AppProvider**

- Add `discoveredMcp` signal; set from `bootstrap_app.discoveredMcp`.
- When `loadSettings()` called (settings screen refresh), update `discoveredMcp` from payload.
- Expose `discoveredMcp` on context for `McpSection`.

- [ ] **Step 3: Failing `McpSection` test**

```typescript
test("renders discovered server row", () => {
  // stub context with discoveredMcp: [{ id: "linear", source: "cursor", ... }]
  expect(mountPoint.textContent).toContain("linear");
  expect(mountPoint.textContent).toContain("cursor");
});
```

Run: `npm --prefix crates/ui run test -- src/settings/McpSection.test.tsx` — expect FAIL.

- [ ] **Step 4: `McpSection` UI**

- Checkbox **Discover external MCP configs** → `discoverExternal`; on change call `loadSettings()` to refresh `discoveredMcp`.
- **Discovered servers** subsection (from context `discoveredMcp`): name, `source` + shortened `sourcePath`, enable checkbox → update `disabledDiscoveredIds`.
- Keep manual **Configured servers** section unchanged.
- No import button.

- [ ] **Step 5: Verify**

Run: `npm --prefix crates/ui run typecheck && npm --prefix crates/ui run test -- src/settings/McpSection.test.tsx`
Run: `./scripts/verify.sh`

- [ ] **Step 6: Changelog + commit**

```markdown
- **MCP external discovery:** Scans Cursor, Claude Code, and project `mcp.json` / `.flow/mcp.json`; stdio servers merge at run start; manual servers win on id collision; Settings shows discovered rows with per-server enable via `disabledDiscoveredIds`.
```

```bash
git add crates/ui/ CHANGELOG.md
git commit -m "feat(ui): discovered MCP servers in settings"
```

---

## Self-review

| Requirement | Task |
| --- | --- |
| Cursor / Claude / project mcp.json | Task 1 path table |
| Merge at run start | Task 2 `drive.rs` |
| Settings list discovered | Task 2 payload + Task 3 UI |
| Manual wins on collision | Task 1 test |
| Stdio only | Task 1 skips url-only |
| No duplicate IPC scan | `load_settings` / bootstrap only |
| Ponytail cuts applied | Architecture header |

**Disable model:** `discover_external` turns off all scanning; `disabled_discovered_ids` toggles individual discovered servers without import.

**Env on connect:** Internal scan retains full `McpServerConfig` including `env`; UI rows omit env.

---

## Execution handoff

Plan saved to `docs/superpowers/plans/2026-06-21-mcp-external-discovery.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks
2. **Inline Execution** — batch in this session with checkpoints

Which approach?

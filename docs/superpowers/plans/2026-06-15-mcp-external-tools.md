# MCP External Tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Load stdio MCP server tools as namespaced external definitions, default them to **Write** tier (prompt under `ApprovalMode::Write`), and execute through the existing `ToolRunner` / `dispatch.rs` seam.

**Architecture:** Persist MCP servers on `AppSettings`. At **run start**, spawn stdio clients for enabled servers, list tools, merge into `ToolRegistry`. Names: `mcp/{server_id}/{tool_name}`. Add one dispatch arm (`BuiltinToolKind::Mcp`) in `dispatch.rs`. Drop clients when the run ends. Settings UI edits the same settings blob; one probe IPC for the Test button. No global MCP manager, no per-node tool picker in v1.

**Tech Stack:** Rust, [`rmcp`](https://crates.io/crates/rmcp) (stdio client — do not hand-roll JSON-RPC), serde_json, Tauri v2, SolidJS.

**Reference (external):** OMP analysis — `oh-my-pi/docs/omp-tool-analysis/16-mcp-extensions.md`.

---

## Current gaps

| Layer | Today | After this plan |
| --- | --- | --- |
| `settings/model.rs` | No MCP config | `McpSettings` on `AppSettings`, round-trips in `settings.json` |
| `adapters/mcp/` | Missing | Stdio client: spawn, `list_tools`, `call_tool`, namespacing |
| `tool/registry.rs` | Builtin-only | `extend_mcp(...)` merges enabled server tools; rejects builtin id collisions |
| `engine/tools/config.rs` | `ToolTier` is `Read` \| `Write` only | `mcp/` prefix → `ToolTier::Write` (not a fictional `Exec` tier) |
| `tool/dispatch.rs` | No external arm | `BuiltinToolKind::Mcp` → MCP adapter |
| `run/execution/drive.rs` | `ToolRegistry::new()` only | Build registry from settings snapshot; run-scoped MCP handles |
| `backend/mod.rs` | — | `probe_mcp_server` delegate only (no list/get CRUD IPC) |
| `ui/settings/` | Providers + appearance only | `McpSection` + nav entry; probe via port seam |

**Out of scope (defer — do not build “for later”):**

- HTTP/SSE MCP transport
- Discovery mode (`off` / `mcp-only` / `all`) from ROADMAP
- Per-node MCP tool opt-in UI (v1: all enabled-server tools appear in `definitions_for`, same as builtins today)
- `search_tool_bm25` / resolve discovery tools
- Custom TypeScript extension loading
- Cross-run MCP client pool / daemon
- `list_mcp_servers` IPC (settings load/save already covers CRUD)

---

## File map

| File | Responsibility |
| --- | --- |
| `crates/orchestration/src/settings/model.rs` | `McpServerConfig`, `McpSettings` on `AppSettings` |
| `crates/orchestration/src/adapters/mcp/mod.rs` | `McpStdioClient`, namespacing, errors, `McpRunClients` (run-scoped `HashMap`) |
| `crates/orchestration/src/tool/registry.rs` | `extend_mcp`, `BuiltinToolKind::Mcp` |
| `crates/orchestration/src/tool/dispatch.rs` | `Mcp` match arm |
| `crates/orchestration/src/tool/runner.rs` | Hold `Option<McpRunClients>`; pass into dispatch |
| `crates/orchestration/src/run/execution/mod.rs` | `mcp: McpSettings` on `InteractiveWorkflowRunParams` |
| `crates/orchestration/src/run/execution/drive.rs` | Connect clients → registry → runner at run start; drop on exit |
| `crates/orchestration/src/run/coordinator.rs` | Pass `settings.mcp` into run params |
| `crates/orchestration/src/backend/mod.rs` | `probe_mcp_server` |
| `crates/desktop/src/lib.rs` | Register probe command |
| `crates/ui/src/port.ts`, `api.ts` | `probeMcpServer` on seam |
| `crates/ui/src/settings/McpSection.tsx` | Server list, enable toggle, add form, Test |
| `crates/ui/src/settings/types.ts`, `SettingsNav.tsx` | `"mcp"` section id |
| `crates/engine/src/tools/config.rs` | `mcp/` → `ToolTier::Write` |

---

## V1 behavior

- Stdio only: `command` + `args` + `env`.
- Per-server `enabled`; disabled servers contribute zero tools.
- Tool ids: `mcp/{server_id}/{original_name}` — `server_id` and `original_name` must not contain `/`.
- Builtins cannot be shadowed; reject merge if namespaced id equals a builtin name.
- Approval: `ToolTier::Write` → prompt when node uses default `ApprovalMode::Write`.
- Enabled MCP tools included in `definitions_for` / `definitions_for_subagent` (read-only runs still hide Write-tier tools).
- Probe IPC lists tool names from one server config without starting a workflow run.
- Run end drops subprocesses (no background MCP daemons).

---

### Task 1: Settings schema

**Files:**
- Modify: `crates/orchestration/src/settings/model.rs`
- Test: inline in `model.rs`

- [ ] **Step 1: Failing round-trip test**

```rust
#[test]
fn mcp_settings_round_trip() {
    let settings = AppSettings {
        mcp: McpSettings {
            servers: vec![McpServerConfig {
                id: "github".into(),
                display_name: "GitHub".into(),
                command: "npx".into(),
                args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
                env: Default::default(),
                enabled: true,
            }],
        },
        ..AppSettings::default()
    };
    let json = serde_json::to_string(&settings).unwrap();
    let parsed: AppSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.mcp.servers[0].id, "github");
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p orchestration mcp_settings_round_trip`

- [ ] **Step 3: Add types**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSettings {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub id: String,
    pub display_name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
```

Add `#[serde(default)] pub mcp: McpSettings` to `AppSettings`. Existing `settings_store` needs no schema migration beyond serde defaults.

- [ ] **Step 4: Run — expect PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(settings): add MCP server config schema"
```

---

### Task 2: Stdio MCP adapter (`rmcp`)

**Files:**
- Modify: workspace `Cargo.toml`, `crates/orchestration/Cargo.toml`
- Create: `crates/orchestration/src/adapters/mcp/mod.rs`
- Modify: `crates/orchestration/src/adapters/mod.rs`
- Test: `crates/orchestration/src/adapters/mcp/mod.rs`

- [ ] **Step 1: Add dependency (pick one crate — no hand-rolled stdio)**

```toml
rmcp = { version = "0.3", features = ["client", "transport-io"] }
```

- [ ] **Step 2: Namespacing unit test (no network)**

```rust
#[test]
fn namespaced_tool_name_rejects_slashes_in_segments() {
    assert_eq!(namespaced_tool_name("gh", "search"), "mcp/gh/search");
    assert!(namespaced_tool_name("bad/id", "search").is_err());
}
```

- [ ] **Step 3: Implement in one module**

```rust
pub fn namespaced_tool_name(server_id: &str, tool_name: &str) -> Result<String, McpError>;

pub struct McpStdioClient { /* rmcp handle */ }

impl McpStdioClient {
    pub async fn spawn(config: &McpServerConfig) -> Result<Self, McpError>;
    pub async fn list_tools(&self, server_id: &str) -> Result<Vec<ToolDefinition>, McpError>;
    pub async fn call_tool(&self, original_name: &str, args: Value) -> Result<String, McpError>;
}

/// Run-scoped handles — ponytail: drop on run end; pool if cold-start hurts.
pub struct McpRunClients {
    clients: HashMap<String, McpStdioClient>,
}
```

Map MCP schemas to `engine::ToolDefinition` with `tier: ToolTier::Write`, `concurrency: ToolConcurrency::Shared`.

- [ ] **Step 4: Live test (ignored)**

```rust
#[tokio::test]
#[ignore = "requires STEP_MCP_LIVE=1"]
async fn stdio_client_lists_tools() { /* npx @modelcontextprotocol/server-time */ }
```

Run: `STEP_MCP_LIVE=1 cargo test -p orchestration stdio_client_lists_tools -- --ignored --nocapture`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(mcp): stdio adapter via rmcp"
```

---

### Task 3: Registry, tier policy, dispatch

**Files:**
- Modify: `crates/orchestration/src/tool/registry.rs`
- Modify: `crates/orchestration/src/tool/dispatch.rs`
- Modify: `crates/engine/src/tools/config.rs`
- Test: registry + config inline tests

- [ ] **Step 1: Failing registry merge test**

```rust
#[test]
fn registry_extends_mcp_without_shadowing_builtins() {
    let mut registry = ToolRegistry::new();
    registry.extend_mcp(vec![RegisteredTool {
        definition: ToolDefinition {
            name: "mcp/gh/search".into(),
            description: "Search GitHub".into(),
            input_schema: json!({"type":"object","properties":{}}),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::Mcp,
    }]).unwrap();
    assert!(registry.get("read").is_ok());
    assert!(registry.get("mcp/gh/search").is_ok());
    assert!(registry.get("search").is_ok());
}
```

- [ ] **Step 2: Add `BuiltinToolKind::Mcp` + `extend_mcp`**

Return `Err` if any namespaced id equals an existing builtin key.

- [ ] **Step 3: Tier policy**

```rust
fn default_tier_for_tool_name(tool_name: &str) -> ToolTier {
    match tool_name {
        "read" | "search" | "find" | "ast_grep" => ToolTier::Read,
        name if name.starts_with("mcp/") => ToolTier::Write,
        _ => ToolTier::Write,
    }
}
```

Add unit test for `mcp/` prefix.

- [ ] **Step 4: Dispatch arm**

In `dispatch.rs`, `BuiltinToolKind::Mcp` parses `mcp/{server_id}/{tool}` and calls `McpRunClients`. Normalize result to `ToolExecutionRecord` (text + artifact spill if large).

- [ ] **Step 5: Run tests — PASS**

Run: `cargo test -p orchestration tool::registry` and `cargo test -p engine tools::config`

- [ ] **Step 6: Commit**

---

### Task 4: Run wiring

**Files:**
- Modify: `crates/orchestration/src/tool/runner.rs`
- Modify: `crates/orchestration/src/run/execution/mod.rs`
- Modify: `crates/orchestration/src/run/execution/drive.rs`
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Test: `crates/orchestration/src/tool/runner.rs` or `run/execution/tests.rs`

- [ ] **Step 1: Extend run params**

Add `pub mcp: McpSettings` to `InteractiveWorkflowRunParams`. Coordinator fills from settings snapshot at run start.

- [ ] **Step 2: Build registry in `drive.rs`**

Replace bare `ToolRegistry::new()` with:

1. `McpRunClients::connect(&params.mcp).await` (enabled servers only)
2. `registry.extend_mcp(clients.tool_definitions())`
3. `ToolRunner::with_mcp_clients(registry, clients, …)`

- [ ] **Step 3: Failing dispatch test**

Mock `McpRunClients` (or inject stub returning `"ok"`) — execute `mcp/test/echo` through `ToolRunner::execute`.

- [ ] **Step 4: Run end**

Ensure `McpRunClients` drops when the drive task finishes (struct field on runner, no global singleton).

- [ ] **Step 5: Commit**

---

### Task 5: Settings UI + probe IPC

**Files:**
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/ui/src/port.ts`, `api.ts`, `lib/types/`
- Create: `crates/ui/src/settings/McpSection.tsx`
- Modify: `crates/ui/src/settings/types.ts`, `SettingsNav.tsx`, `SettingsScreen.tsx`
- Test: `crates/ui/src/settings/McpSection.test.tsx` (minimal: renders server row)

- [ ] **Step 1: Backend probe only**

```rust
pub fn probe_mcp_server(config: McpServerConfig) -> Result<Vec<String>, BackendError> {
    // spawn ephemeral client, list tool names, drop
}
```

No `list_mcp_servers` — UI reads/writes `AppSettings.mcp` via existing load/save.

- [ ] **Step 2: Port seam**

Extend `UiDesktopOutboundPort` with `probeMcpServer(config) → Promise<string[]>`.

- [ ] **Step 3: `McpSection`**

Match `ProvidersSection` patterns: table (name, command, enabled), Add form (id, command, args comma-separated), Test → probe. Persist through `ctx.updateSettings`.

- [ ] **Step 4: Typecheck + UI test**

Run: `npm --prefix crates/ui run typecheck` and targeted vitest.

- [ ] **Step 5: Commit**

---

### Task 6: Verification

- [ ] **Step 1: Live smoke (optional)**

```bash
STEP_MCP_LIVE=1 cargo test -p orchestration stdio_client_lists_tools -- --ignored --nocapture
```

- [ ] **Step 2: Full gate**

Run: `./scripts/verify.sh`

- [ ] **Step 3: Docs**

Update `CHANGELOG.md`. Mark ROADMAP MCP items done only for shipped scope (stdio, namespacing, Write-tier prompt, settings probe UI — not discovery mode or HTTP).

---

## Self-review

| OMP capability | Task | Notes |
| --- | --- | --- |
| MCP server config | 1 | Settings blob only |
| Tool adapter + execute | 2, 3, 4 | `rmcp` + dispatch arm |
| Namespacing / no builtin shadow | 2, 3 | `mcp/{id}/{tool}` |
| Conservative approval | 3 | `ToolTier::Write`, not `Exec` |
| Discovery UI | 5 | Probe + enable toggle; no BM25 |
| BM25 search_tool | — | Out of scope |
| Custom TS tools | — | Out of scope |
| Cross-run pool | — | Out of scope |

## Ponytail audit (plan debt removed)

| Tag | Cut | Replacement |
| --- | --- | --- |
| `yagni` | `tool/external.rs`, `ExternalToolRef`, `McpClientPool` | Namespacing in `adapters/mcp/mod.rs`; `McpRunClients` HashMap |
| `yagni` | `adapters/mcp/stdio.rs` hand-rolled framing | Single `mod.rs` using `rmcp` |
| `yagni` | `backend` MCP manager + refresh on save | Run-scoped connect in `drive.rs` |
| `yagni` | `list_mcp_servers` IPC | Existing settings load/save |
| `delete` | `ToolTier::Exec` | `ToolTier::Write` (actual engine enum) |
| `delete` | Per-node MCP opt-in (v1) | Same exposure model as builtins today |
| `delete` | Discovery mode (`off`/`mcp-only`/`all`) | Defer to follow-up plan |

**net:** ~2 files, 1 abstraction layer, 1 IPC command, 1 fake enum variant avoided.

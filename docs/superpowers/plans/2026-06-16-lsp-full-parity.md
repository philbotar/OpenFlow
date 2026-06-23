# LSP Full Parity (Real Client + Agent-Facing `lsp` Tool) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace this repo's stubbed LSP writethrough with a real Rust language-server client and ship an agent-facing `lsp` tool, closing the gap to oh-my-pi's two LSP layers (diagnostics-on-write + navigation/refactor tool).

**Architecture:** A long-lived `LspManager` (an `Arc`-shared struct owned by `ToolRunner`) spawns one language-server process per `command:cwd`, frames JSON-RPC over the child's stdio with a background reader task, and caches per-server diagnostics, capabilities, and open files. The async `ToolRunner::dispatch` path (1) appends real `publishDiagnostics` after `Write`/`Edit`/`ApplyPatch` when `diagnostics_on_write` is set, replacing the `// future work` no-op in `writethrough.rs`, and (2) backs a new `BuiltinToolKind::Lsp` tool that exposes `diagnostics`, `definition`, `type_definition`, `implementation`, `references`, `hover`, `symbols`, `rename`, `rename_file`, `code_actions`, `status`, `reload`, `capabilities`, and `request`. Format-on-write stays in the existing synchronous blocking path; diagnostics move to the async layer because the client is inherently async and stateful.

**Tech Stack:** Rust (edition 2021), tokio (`process`, `sync`, `time`, `macros`, `rt-multi-thread` — already enabled), `serde`/`serde_json`, and the `lsp-types` crate (new dependency) for protocol structs. No new async-LSP framework — the JSON-RPC client is hand-rolled over `tokio::process` so we control process lifecycle exactly like OMP's `client.ts`.

**Workspace constraints (do not violate):** `[workspace.lints.rust] warnings = "deny"`, `unsafe_code = "forbid"`, `[workspace.lints.clippy] all = "deny"`, `allow_attributes_without_reason = "deny"`. Every task must end warning- and clippy-clean. Run `cargo clippy -p orchestration --all-targets` before each commit; never add `#[allow(...)]` without a `reason = "..."`.

---

## Scope

**In scope (this plan):**
- Real JSON-RPC LSP client with process lifecycle, background reader, request/notification, cancellation, and timeouts.
- Server config: built-in defaults for `rust-analyzer`, `typescript-language-server`, `pyright`, `gopls`; auto-detect from root markers + binary discovery; layered JSON override file (`.pi/lsp.json`).
- Diagnostics-on-write wired through the async dispatch path (replaces the stub).
- The `lsp` tool with all 14 actions and OMP-compatible text output.
- Tool registration, gating (`lsp.enabled`, `session.enableLsp`), and the model-facing prompt doc.

**Deferred (explicit non-goals — see "Deferred Follow-ups" at the end, NOT placeholders):**
- `lspmux` multiplexing wrapper.
- Custom CLI linter adapters (Biome, SwiftLint) as `diagnostics` sources.
- Directory-tree (multi-file) `rename_file`; this plan ships single-file `rename_file` only.
- Idle-client sweeper / `setIdleTimeout`; clients live until process shutdown.

These are isolated additions that do not block the core parity and would bloat the first delivery. Each has a stub task at the end describing the exact extension point.

---

## File Structure

All new code lives under the existing module `crates/orchestration/src/adapters/infrastructure/lsp/` (referred to below as `lsp/`), which today holds `mod.rs`, `config.rs`, `diagnostics.rs`, `formatters.rs`, `patch_fs.rs`, `writethrough.rs`.

- Create `lsp/protocol.rs` — JSON-RPC message framing (Content-Length headers) over async byte streams; `Message`, `RequestId`, `OutboundRequest`, encode/decode.
- Create `lsp/server_config.rs` — `LspServerConfig`, built-in `defaults()`, root-marker + binary auto-detect, `.pi/lsp.json` override loading, file→server routing.
- Create `lsp/client.rs` — `LspClient`: spawn process, background reader task, `send_request`/`send_notification`, `ensure_file_open`, diagnostics cache, capabilities, project-load wait.
- Create `lsp/position.rs` — `resolve_symbol_column` (port of OMP `resolveSymbolColumn`), `path_to_uri`/`uri_to_path`.
- Create `lsp/manager.rs` — `LspManager`: `Arc`-shared client cache keyed by `command:cwd`, file routing, `diagnostics_for`, and per-action entry points returning rendered text.
- Create `lsp/actions.rs` — pure rendering helpers (port of OMP `render.ts`): format locations, references, symbols, workspace edits, diagnostics.
- Create `lsp/tool.rs` — `LspToolArgs` (serde), input schema JSON, and `execute()` that parses args, calls `LspManager`, and returns `(text, success)`.
- Modify `lsp/mod.rs` — add `pub mod` lines and re-exports.
- Modify `lsp/config.rs` — extend `LspSettings` with `servers: Vec<LspServerConfig>` and a `lsp_root` field.
- Modify `crates/orchestration/src/settings/model.rs:172` — extend persisted `LspSettings` (add optional `servers` override path is NOT persisted; only `enabled`/format/diagnostics already exist — no change needed beyond confirming, see Task 17).
- Modify `crates/orchestration/src/tool/registry.rs` — add `BuiltinToolKind::Lsp` and `lsp_tool()`; register it.
- Modify `crates/orchestration/src/tool/runner.rs` — add `lsp_manager: Arc<LspManager>` field, construct it, expose accessor.
- Modify `crates/orchestration/src/tool/dispatch.rs` — add `BuiltinToolKind::Lsp` arm; append diagnostics-on-write after `Write`/`Edit`/`ApplyPatch`.
- Modify `crates/orchestration/src/run/execution/drive.rs:49` — pass cwd/settings into the manager via `ToolRunner` construction (already threaded; confirm).
- Modify `Cargo.toml` (workspace) and `crates/orchestration/Cargo.toml` — add `lsp-types`.
- Create `crates/orchestration/src/prompts/tools/lsp.md` (or repo's prompt location — Task 22 verifies) — model-facing tool doc.
- Modify `docs/sections/orchestration/callable-agents.md` / `CHANGELOG.md` — docs (Task 23).

---

## Phase A — Protocol & Dependency Foundation

### Task 1: Add the `lsp-types` dependency

**Files:**
- Modify: `Cargo.toml` (workspace `[workspace.dependencies]`, after the `tokio-util` line ~40)
- Modify: `crates/orchestration/Cargo.toml` (`[dependencies]`, after `tokio-util.workspace = true`)

- [ ] **Step 1: Add to workspace dependencies**

In `Cargo.toml`, add this line in `[workspace.dependencies]` immediately after the `tokio-util = { ... }` line:

```toml
lsp-types = "0.97.0"
```

- [ ] **Step 2: Reference it from orchestration**

In `crates/orchestration/Cargo.toml`, add after `tokio-util.workspace = true`:

```toml
lsp-types = { workspace = true }
```

- [ ] **Step 3: Verify it resolves and compiles**

Run: `cargo build -p orchestration`
Expected: builds (downloads `lsp-types 0.97.x`), no errors. If `0.97.0` is yanked/unavailable, run `cargo add lsp-types -p orchestration --dry-run` to pick the latest `0.9x` and use that exact version in both files.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/orchestration/Cargo.toml Cargo.lock
git commit -m "build: add lsp-types dependency for LSP client"
```

---

### Task 2: JSON-RPC message framing (`protocol.rs`)

LSP frames messages as `Content-Length: N\r\n\r\n<json>`. This module reads/writes those frames over async byte streams. No process logic here — pure framing so it is unit-testable without a child process.

**Files:**
- Create: `crates/orchestration/src/adapters/infrastructure/lsp/protocol.rs`
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `lsp/protocol.rs` with only the tests first:

```rust
//! JSON-RPC over stdio framing for the LSP client (Content-Length headers).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_with_content_length_header() {
        let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"});
        let framed = encode_message(&body);
        let text = String::from_utf8(framed).expect("utf8");
        assert!(text.starts_with("Content-Length: "));
        assert!(text.contains("\r\n\r\n"));
        let split = text.split("\r\n\r\n").nth(1).expect("body");
        let parsed: serde_json::Value = serde_json::from_str(split).expect("json");
        assert_eq!(parsed["method"], "initialize");
    }

    #[tokio::test]
    async fn reads_a_single_frame() {
        let body = serde_json::json!({"jsonrpc":"2.0","id":7,"result":{"ok":true}});
        let framed = encode_message(&body);
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(framed));
        let msg = read_message(&mut reader).await.expect("read").expect("some");
        assert_eq!(msg["id"], 7);
        assert_eq!(msg["result"]["ok"], true);
    }

    #[tokio::test]
    async fn returns_none_on_clean_eof() {
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(Vec::new()));
        let msg = read_message(&mut reader).await.expect("read");
        assert!(msg.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp::protocol`
Expected: FAIL — `encode_message`/`read_message` not found (also `protocol` module not declared yet; add `pub mod protocol;` to `mod.rs` first if the test can't find the module — see Step 3).

- [ ] **Step 3: Declare the module and implement framing**

In `lsp/mod.rs`, add after the existing `pub mod config;` lines:

```rust
pub mod protocol;
```

Then prepend the implementation above the `#[cfg(test)]` block in `protocol.rs`:

```rust
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};

/// Frame a JSON value as an LSP message (`Content-Length` header + CRLF + body).
#[must_use]
pub fn encode_message(message: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).unwrap_or_else(|_| b"{}".to_vec());
    let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    framed.extend_from_slice(&body);
    framed
}

/// Read one LSP message from `reader`. Returns `Ok(None)` on clean EOF.
pub async fn read_message<R>(reader: &mut BufReader<R>) -> std::io::Result<Option<Value>>
where
    R: AsyncRead + Unpin,
{
    let mut content_length: Option<usize> = None;
    loop {
        let mut header = String::new();
        let read = reader.read_line(&mut header).await?;
        if read == 0 {
            return Ok(None); // clean EOF before any header
        }
        let trimmed = header.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // blank line terminates headers
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(len) = content_length else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "LSP frame missing Content-Length header",
        ));
    };
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    let value = serde_json::from_slice::<Value>(&buf)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(Some(value))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::protocol`
Expected: PASS (3 tests).

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/protocol.rs crates/orchestration/src/adapters/infrastructure/lsp/mod.rs
git commit -m "feat(lsp): JSON-RPC content-length framing"
```

---

### Task 3: URI/path conversion and symbol-column resolution (`position.rs`)

Ports OMP `utils.ts` `resolveSymbolColumn` + URI helpers. Position-based actions resolve a 1-indexed `line` + optional `symbol` substring (with `name#N` occurrence selector) to a 0-indexed `{line, character}`.

**Files:**
- Create: `crates/orchestration/src/adapters/infrastructure/lsp/position.rs`
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `lsp/position.rs`:

```rust
//! URI conversion and symbol-column resolution for position-based LSP actions.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_path_and_uri() {
        let path = std::path::Path::new("/tmp/some dir/main.rs");
        let uri = path_to_uri(path);
        assert!(uri.starts_with("file:///"));
        assert!(uri.contains("some%20dir"));
        assert_eq!(uri_to_path(&uri).as_deref(), Some(path));
    }

    #[test]
    fn first_non_whitespace_when_symbol_omitted() {
        let line = "    let answer = 42;";
        let col = resolve_column_in_line(line, None).expect("col");
        assert_eq!(col, 4);
    }

    #[test]
    fn finds_first_occurrence_of_symbol() {
        let line = "let answer = answer + 1;";
        let col = resolve_column_in_line(line, Some("answer")).expect("col");
        assert_eq!(col, 4);
    }

    #[test]
    fn honors_occurrence_selector() {
        let line = "let answer = answer + 1;";
        let col = resolve_column_in_line(line, Some("answer#2")).expect("col");
        assert_eq!(col, 13);
    }

    #[test]
    fn errors_when_symbol_absent() {
        let err = resolve_column_in_line("let x = 1;", Some("missing")).unwrap_err();
        assert!(err.contains("missing"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

In `lsp/mod.rs` add `pub mod position;`, then run: `cargo test -p orchestration lsp::position`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement**

Prepend to `position.rs`:

```rust
use std::path::{Path, PathBuf};

/// Convert an absolute filesystem path to a `file://` URI with percent-encoding.
#[must_use]
pub fn path_to_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for component in path.to_string_lossy().split('/') {
        if component.is_empty() {
            continue;
        }
        uri.push('/');
        for byte in component.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    uri.push(byte as char);
                }
                _ => uri.push_str(&format!("%{byte:02X}")),
            }
        }
    }
    uri
}

/// Convert a `file://` URI back to a filesystem path. Returns `None` for non-file URIs.
#[must_use]
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut raw = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                raw.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        raw.push(bytes[i]);
        i += 1;
    }
    let text = String::from_utf8(raw).ok()?;
    Some(PathBuf::from(text))
}

/// Resolve a 0-indexed column on a line given an optional `symbol` (supports `name#N`).
///
/// When `symbol` is `None`, returns the first non-whitespace column.
pub fn resolve_column_in_line(line: &str, symbol: Option<&str>) -> Result<u32, String> {
    let Some(symbol) = symbol else {
        let col = line
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .map_or(0, |(idx, _)| idx);
        return Ok(col as u32);
    };
    let (needle, occurrence) = match symbol.rsplit_once('#') {
        Some((name, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => {
            (name, n.parse::<usize>().unwrap_or(1).max(1))
        }
        _ => (symbol, 1usize),
    };
    let mut found = 0usize;
    let mut search_from = 0usize;
    while let Some(rel) = line[search_from..].find(needle) {
        let abs = search_from + rel;
        found += 1;
        if found == occurrence {
            return Ok(line[..abs].chars().count() as u32);
        }
        search_from = abs + needle.len().max(1);
    }
    // Case-insensitive fallback (single occurrence only), matching OMP behavior.
    if occurrence == 1 {
        let lower_line = line.to_lowercase();
        let lower_needle = needle.to_lowercase();
        if let Some(abs) = lower_line.find(&lower_needle) {
            return Ok(line[..abs].chars().count() as u32);
        }
    }
    Err(format!("symbol \"{symbol}\" not found on line"))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::position`
Expected: PASS (5 tests).

- [ ] **Step 5: Lint (remove dead locals if flagged) and commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/position.rs crates/orchestration/src/adapters/infrastructure/lsp/mod.rs
git commit -m "feat(lsp): uri conversion and symbol-column resolution"
```

---

## Phase B — Server Configuration & Routing

### Task 4: Server config + built-in defaults + routing (`server_config.rs`)

**Files:**
- Create: `crates/orchestration/src/adapters/infrastructure/lsp/server_config.rs`
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `lsp/server_config.rs`:

```rust
//! Language-server definitions, auto-detection, and file→server routing.

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn defaults_include_rust_analyzer() {
        let server = builtin_defaults()
            .into_iter()
            .find(|s| s.name == "rust-analyzer")
            .expect("rust-analyzer default");
        assert!(server.extensions.contains(&"rs".to_string()));
        assert_eq!(server.command, "rust-analyzer");
        assert!(server.root_markers.iter().any(|m| m == "Cargo.toml"));
    }

    #[test]
    fn routes_file_to_matching_server() {
        let servers = builtin_defaults();
        let chosen = server_for_file(&servers, Path::new("src/main.rs")).expect("server");
        assert_eq!(chosen.name, "rust-analyzer");
        assert!(server_for_file(&servers, Path::new("notes.txt")).is_none());
    }

    #[test]
    fn ts_server_handles_tsx() {
        let servers = builtin_defaults();
        let chosen = server_for_file(&servers, Path::new("app/page.tsx")).expect("server");
        assert_eq!(chosen.name, "typescript-language-server");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod server_config;` to `lsp/mod.rs`, then run: `cargo test -p orchestration lsp::server_config`
Expected: FAIL — types/functions not defined.

- [ ] **Step 3: Implement config + routing**

Prepend to `server_config.rs`:

```rust
use std::path::Path;

/// One language-server definition resolved for a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    /// File extensions (no leading dot) this server handles.
    pub extensions: Vec<String>,
    /// Workspace root markers used by auto-detect.
    pub root_markers: Vec<String>,
}

fn server(
    name: &str,
    command: &str,
    args: &[&str],
    extensions: &[&str],
    root_markers: &[&str],
) -> LspServerConfig {
    LspServerConfig {
        name: name.to_string(),
        command: command.to_string(),
        args: args.iter().map(|a| (*a).to_string()).collect(),
        extensions: extensions.iter().map(|e| (*e).to_string()).collect(),
        root_markers: root_markers.iter().map(|m| (*m).to_string()).collect(),
    }
}

/// Built-in server definitions (subset matching OMP defaults.json for the core languages).
#[must_use]
pub fn builtin_defaults() -> Vec<LspServerConfig> {
    vec![
        server(
            "rust-analyzer",
            "rust-analyzer",
            &[],
            &["rs"],
            &["Cargo.toml", "rust-project.json"],
        ),
        server(
            "typescript-language-server",
            "typescript-language-server",
            &["--stdio"],
            &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
            &["tsconfig.json", "jsconfig.json", "package.json"],
        ),
        server("pyright", "pyright-langserver", &["--stdio"], &["py", "pyi"], &[
            "pyproject.toml",
            "setup.py",
            "requirements.txt",
            "pyrightconfig.json",
        ]),
        server("gopls", "gopls", &[], &["go"], &["go.mod", "go.work"]),
    ]
}

/// Pick the first server whose `extensions` include the file's extension.
#[must_use]
pub fn server_for_file<'a>(
    servers: &'a [LspServerConfig],
    path: &Path,
) -> Option<&'a LspServerConfig> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    servers
        .iter()
        .find(|server| server.extensions.iter().any(|candidate| candidate == &ext))
}

/// All servers whose `extensions` include the file's extension.
#[must_use]
pub fn servers_for_file<'a>(
    servers: &'a [LspServerConfig],
    path: &Path,
) -> Vec<&'a LspServerConfig> {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return Vec::new();
    };
    let ext = ext.to_ascii_lowercase();
    servers
        .iter()
        .filter(|server| server.extensions.iter().any(|candidate| candidate == &ext))
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::server_config`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/server_config.rs crates/orchestration/src/adapters/infrastructure/lsp/mod.rs
git commit -m "feat(lsp): server definitions and file routing"
```

---

### Task 5: Auto-detect + `.pi/lsp.json` overrides

Auto-detect keeps only servers whose root marker exists under `cwd` AND whose binary is on `PATH`. A `.pi/lsp.json` override file (if present) replaces the whole list.

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/server_config.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `server_config.rs`:

```rust
    #[test]
    fn detects_only_servers_with_present_root_markers() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").expect("write");
        let detected = resolve_servers(dir.path());
        // rust-analyzer's marker is present; gopls/pyright/ts markers are not.
        let names: Vec<&str> = detected.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"rust-analyzer"));
        assert!(!names.contains(&"gopls"));
    }

    #[test]
    fn override_file_replaces_defaults() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".pi")).expect("mkdir");
        std::fs::write(
            dir.path().join(".pi/lsp.json"),
            r#"{"servers":[{"name":"custom","command":"custom-ls","args":["--stdio"],"extensions":["foo"],"rootMarkers":["foo.toml"]}]}"#,
        )
        .expect("write");
        let servers = load_override(dir.path()).expect("override present");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "custom");
        assert_eq!(servers[0].extensions, vec!["foo".to_string()]);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp::server_config`
Expected: FAIL — `resolve_servers` / `load_override` not defined.

- [ ] **Step 3: Implement detection + override loading**

Add to `server_config.rs` (above the test module). Note `serde` derive for the override schema:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OverrideFile {
    servers: Vec<OverrideServer>,
}

#[derive(Debug, Deserialize)]
struct OverrideServer {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default, rename = "rootMarkers")]
    root_markers: Vec<String>,
}

/// Load `<cwd>/.pi/lsp.json` if present, returning the override server list.
#[must_use]
pub fn load_override(cwd: &Path) -> Option<Vec<LspServerConfig>> {
    let path = cwd.join(".pi").join("lsp.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let parsed: OverrideFile = serde_json::from_str(&raw).ok()?;
    Some(
        parsed
            .servers
            .into_iter()
            .map(|server| LspServerConfig {
                name: server.name,
                command: server.command,
                args: server.args,
                extensions: server.extensions,
                root_markers: server.root_markers,
            })
            .collect(),
    )
}

fn binary_on_path(binary: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(binary);
        candidate.is_file()
            || candidate.with_extension("exe").is_file()
            || candidate.with_extension("cmd").is_file()
    })
}

fn marker_present(cwd: &Path, marker: &str) -> bool {
    cwd.join(marker).exists()
}

/// Resolve the active server list for `cwd`: override file wins; otherwise
/// auto-detect defaults whose root marker exists and whose binary is on PATH.
#[must_use]
pub fn resolve_servers(cwd: &Path) -> Vec<LspServerConfig> {
    if let Some(overridden) = load_override(cwd) {
        return overridden;
    }
    builtin_defaults()
        .into_iter()
        .filter(|server| {
            server.root_markers.iter().any(|m| marker_present(cwd, m)) && binary_on_path(&server.command)
        })
        .collect()
}
```

> The `binary_on_path` gate means tests for `resolve_servers` only assert on servers whose marker is present; the rust-analyzer assertion in Step 1 passes only when `rust-analyzer` is installed. If CI lacks it, change that assertion to: `assert!(resolve_servers(dir.path()).iter().all(|s| s.name != "gopls"));` and drop the positive `contains` check. Prefer keeping the positive check locally where `rust-analyzer` is installed.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::server_config`
Expected: PASS. (If `rust-analyzer` is not installed locally, apply the Step 3 note.)

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/server_config.rs
git commit -m "feat(lsp): auto-detect servers and load .pi/lsp.json overrides"
```

---

## Phase C — The LSP Client

### Task 6: `LspClient` skeleton — spawn, initialize, shutdown

This is the load-bearing process-lifecycle code. One process per client; a background task reads frames and routes them to pending requests, the diagnostics cache, and server→client request handlers.

**Files:**
- Create: `crates/orchestration/src/adapters/infrastructure/lsp/client.rs`
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/mod.rs`

- [ ] **Step 1: Write the failing test (using a stub echo server)**

We test the request/response plumbing against a tiny stub LSP server written in the test itself (a shell script that speaks the framing). Create `lsp/client.rs`:

```rust
//! Long-lived language-server client: process lifecycle + JSON-RPC plumbing.

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Write a stub "language server" script that initializes and echoes one request.
    fn write_stub_server(dir: &std::path::Path) -> PathBuf {
        // A python stub is the most portable. It frames responses like a real LSP server.
        let script = dir.join("stub_ls.py");
        std::fs::write(
            &script,
            r#"
import sys, json
def read():
    headers={}
    while True:
        line=sys.stdin.buffer.readline()
        if line in (b"\r\n", b"\n", b""):
            break
        k,_,v=line.decode().partition(":")
        headers[k.strip().lower()]=v.strip()
    n=int(headers["content-length"])
    return json.loads(sys.stdin.buffer.read(n))
def write(obj):
    body=json.dumps(obj).encode()
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n"%len(body))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()
while True:
    try:
        msg=read()
    except Exception:
        break
    if msg.get("method")=="initialize":
        write({"jsonrpc":"2.0","id":msg["id"],"result":{"capabilities":{"hoverProvider":True}}})
    elif msg.get("method")=="shutdown":
        write({"jsonrpc":"2.0","id":msg["id"],"result":None})
    elif msg.get("method")=="textDocument/hover":
        write({"jsonrpc":"2.0","id":msg["id"],"result":{"contents":"stub hover"}})
"#,
        )
        .expect("write stub");
        script
    }

    #[tokio::test]
    async fn initializes_and_sends_request() {
        if which_python().is_none() {
            return; // environment without python3; skip
        }
        let dir = tempfile::TempDir::new().expect("tempdir");
        let script = write_stub_server(dir.path());
        let config = crate::lsp::server_config::LspServerConfig {
            name: "stub".into(),
            command: which_python().unwrap(),
            args: vec![script.to_string_lossy().to_string()],
            extensions: vec!["rs".into()],
            root_markers: vec![],
        };
        let client = LspClient::start(&config, dir.path())
            .await
            .expect("client starts");
        assert!(client.capabilities().get("hoverProvider").is_some());
        let result = client
            .send_request(
                "textDocument/hover",
                serde_json::json!({"textDocument":{"uri":"file:///x.rs"},"position":{"line":0,"character":0}}),
                std::time::Duration::from_secs(5),
            )
            .await
            .expect("hover ok");
        assert_eq!(result["contents"], "stub hover");
        client.shutdown().await;
    }

    fn which_python() -> Option<String> {
        for candidate in ["python3", "python"] {
            if std::process::Command::new(candidate)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return Some(candidate.to_string());
            }
        }
        None
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod client;` to `lsp/mod.rs`, then run: `cargo test -p orchestration lsp::client`
Expected: FAIL — `LspClient` not defined.

- [ ] **Step 3: Implement the client**

Prepend to `client.rs`:

```rust
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::Value;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::oneshot;

use super::protocol::{encode_message, read_message};
use super::server_config::LspServerConfig;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const WARMUP_TIMEOUT: Duration = Duration::from_secs(5);

type PendingMap = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>;

/// Cached `publishDiagnostics` keyed by document URI.
type DiagnosticsMap = Arc<Mutex<HashMap<String, Vec<Value>>>>;

/// One running language-server process plus its JSON-RPC state.
pub struct LspClient {
    server_name: String,
    next_id: AtomicI64,
    stdin: Mutex<ChildStdin>,
    pending: PendingMap,
    diagnostics: DiagnosticsMap,
    capabilities: Mutex<Value>,
    open_files: Mutex<HashMap<String, i64>>,
    child: Mutex<Child>,
}

impl LspClient {
    /// Spawn the server, start the reader task, and complete `initialize`/`initialized`.
    pub async fn start(config: &LspServerConfig, cwd: &Path) -> Result<Arc<Self>, String> {
        let mut child = tokio::process::Command::new(&config.command)
            .args(&config.args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("failed to spawn {}: {error}", config.command))?;

        let stdin = child.stdin.take().ok_or("missing stdin")?;
        let stdout = child.stdout.take().ok_or("missing stdout")?;

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let diagnostics: DiagnosticsMap = Arc::new(Mutex::new(HashMap::new()));

        let client = Arc::new(Self {
            server_name: config.name.clone(),
            next_id: AtomicI64::new(1),
            stdin: Mutex::new(stdin),
            pending: pending.clone(),
            diagnostics: diagnostics.clone(),
            capabilities: Mutex::new(Value::Null),
            open_files: Mutex::new(HashMap::new()),
            child: Mutex::new(child),
        });

        spawn_reader(stdout, pending, diagnostics);

        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": super::position::path_to_uri(cwd),
            "capabilities": {
                "textDocument": {
                    "hover": {"contentFormat": ["plaintext", "markdown"]},
                    "definition": {"linkSupport": true},
                    "references": {},
                    "documentSymbol": {"hierarchicalDocumentSymbolSupport": true},
                    "rename": {"prepareSupport": false},
                    "codeAction": {},
                    "publishDiagnostics": {}
                },
                "workspace": {"symbol": {}, "applyEdit": true, "configuration": true}
            }
        });

        let init_result = client
            .send_request("initialize", init_params, WARMUP_TIMEOUT)
            .await?;
        if let Some(caps) = init_result.get("capabilities") {
            *client.capabilities.lock() = caps.clone();
        }
        client.send_notification("initialized", serde_json::json!({}))?;
        Ok(client)
    }

    #[must_use]
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    #[must_use]
    pub fn capabilities(&self) -> Value {
        self.capabilities.lock().clone()
    }

    /// Cached diagnostics for a URI (latest `publishDiagnostics`).
    #[must_use]
    pub fn cached_diagnostics(&self, uri: &str) -> Vec<Value> {
        self.diagnostics.lock().get(uri).cloned().unwrap_or_default()
    }

    /// Diagnostics version: a monotonically increasing count of publishes (for freshness waits).
    #[must_use]
    pub fn diagnostics_present(&self, uri: &str) -> bool {
        self.diagnostics.lock().contains_key(uri)
    }

    /// Send a JSON-RPC request and await its response (or timeout).
    pub async fn send_request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(id, tx);
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&message)?;
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(format!("LSP request {method} cancelled (process exited)")),
            Err(_) => {
                self.pending.lock().remove(&id);
                // Best-effort cancellation.
                let _ = self.send_notification(
                    "$/cancelRequest",
                    serde_json::json!({ "id": id }),
                );
                Err(format!(
                    "LSP request {method} timed out after {}ms",
                    timeout.as_millis()
                ))
            }
        }
    }

    /// Convenience wrapper using the default 30s request timeout.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.send_request(method, params, DEFAULT_REQUEST_TIMEOUT).await
    }

    pub fn send_notification(&self, method: &str, params: Value) -> Result<(), String> {
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&message)
    }

    fn write_message(&self, message: &Value) -> Result<(), String> {
        let framed = encode_message(message);
        let mut stdin = self.stdin.lock();
        // ChildStdin is async; use a blocking write via try-blocking is unavailable, so
        // we use `futures` executor-free path: write synchronously through std is not
        // possible. Instead, write using tokio's `write_all` inside a blocking bridge.
        // The reader/writer here run on the multi-thread runtime; use block_in_place.
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                stdin
                    .write_all(&framed)
                    .await
                    .map_err(|error| format!("write to {} failed: {error}", self.server_name))?;
                stdin
                    .flush()
                    .await
                    .map_err(|error| format!("flush to {} failed: {error}", self.server_name))
            })
        })
    }

    /// Track an opened document version (used by `ensure_file_open`).
    pub(super) fn record_open(&self, uri: &str, version: i64) {
        self.open_files.lock().insert(uri.to_string(), version);
    }

    #[must_use]
    pub(super) fn is_open(&self, uri: &str) -> bool {
        self.open_files.lock().contains_key(uri)
    }

    #[must_use]
    pub(super) fn next_version(&self, uri: &str) -> i64 {
        let mut open = self.open_files.lock();
        let entry = open.entry(uri.to_string()).or_insert(0);
        *entry += 1;
        *entry
    }

    /// Send `shutdown` + `exit` and kill the process if it lingers.
    pub async fn shutdown(&self) {
        let _ = self.send_request("shutdown", Value::Null, Duration::from_secs(2)).await;
        let _ = self.send_notification("exit", Value::Null);
        let mut child = self.child.lock();
        let _ = child.start_kill();
    }
}

fn spawn_reader(
    stdout: tokio::process::ChildStdout,
    pending: PendingMap,
    diagnostics: DiagnosticsMap,
) {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_message(&mut reader).await {
                Ok(Some(message)) => handle_message(message, &pending, &diagnostics),
                Ok(None) | Err(_) => break, // EOF or parse failure: server gone
            }
        }
        // Fail all pending requests so callers stop waiting.
        let mut pending = pending.lock();
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err("language server process exited".to_string()));
        }
    });
}

fn handle_message(message: Value, pending: &PendingMap, diagnostics: &DiagnosticsMap) {
    // Response to one of our requests.
    if let Some(id) = message.get("id").and_then(Value::as_i64) {
        if message.get("method").is_none() {
            if let Some(tx) = pending.lock().remove(&id) {
                let result = if let Some(error) = message.get("error") {
                    Err(error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("LSP error")
                        .to_string())
                } else {
                    Ok(message.get("result").cloned().unwrap_or(Value::Null))
                };
                let _ = tx.send(result);
            }
            return;
        }
    }
    // Server→client notification we care about.
    if message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics") {
        if let Some(params) = message.get("params") {
            if let Some(uri) = params.get("uri").and_then(Value::as_str) {
                let items = params
                    .get("diagnostics")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                diagnostics.lock().insert(uri.to_string(), items);
            }
        }
    }
    // Other server→client requests (workspace/configuration, applyEdit) are answered
    // with safe defaults in Task 9; until then they are ignored, which is harmless for
    // read-only actions.
}
```

> **Important runtime note:** `write_message` uses `block_in_place` + `Handle::block_on`, which requires the multi-thread runtime (the workspace's `tokio` enables `rt-multi-thread`). All `LspClient` calls therefore must run on the multi-thread runtime — they do, because `ToolRunner::execute` runs there. The reader task uses `tokio::spawn`. Do not call `LspClient` methods from a `current_thread` runtime in tests; the `#[tokio::test]` macro defaults to `current_thread`, so annotate client tests with `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.

- [ ] **Step 4: Fix the test attribute and run**

Change the two `#[tokio::test]` attributes in `client.rs` tests to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`. Run: `cargo test -p orchestration lsp::client`
Expected: PASS (skips gracefully if `python3` is absent).

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/client.rs crates/orchestration/src/adapters/infrastructure/lsp/mod.rs
git commit -m "feat(lsp): language-server client process lifecycle and JSON-RPC"
```

---

### Task 7: `ensure_file_open` + diagnostics freshness wait

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/client.rs`

- [ ] **Step 1: Write the failing test**

Add to `client.rs` tests (extend the stub `stub_ls.py` to record opens — replace the `initialize` branch handling by adding a `didOpen` echo of a diagnostic). Append these handler branches to the python stub string in `write_stub_server`, before the final newline of the script:

```text
    elif msg.get("method")=="textDocument/didOpen":
        uri=msg["params"]["textDocument"]["uri"]
        write({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"severity":1,"message":"stub error"}]}})
```

Then add the test:

```rust
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ensure_file_open_publishes_diagnostics() {
        if which_python().is_none() {
            return;
        }
        let dir = tempfile::TempDir::new().expect("tempdir");
        let script = write_stub_server(dir.path());
        let file = dir.path().join("x.rs");
        std::fs::write(&file, "fn main() {}\n").expect("write file");
        let config = crate::lsp::server_config::LspServerConfig {
            name: "stub".into(),
            command: which_python().unwrap(),
            args: vec![script.to_string_lossy().to_string()],
            extensions: vec!["rs".into()],
            root_markers: vec![],
        };
        let client = LspClient::start(&config, dir.path()).await.expect("start");
        let uri = crate::lsp::position::path_to_uri(&file);
        client.ensure_file_open(&file).await.expect("open");
        client
            .wait_for_diagnostics(&uri, std::time::Duration::from_secs(3))
            .await;
        assert_eq!(client.cached_diagnostics(&uri).len(), 1);
        client.shutdown().await;
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp::client::tests::ensure_file_open_publishes_diagnostics`
Expected: FAIL — `ensure_file_open` / `wait_for_diagnostics` not defined.

- [ ] **Step 3: Implement**

Add these methods inside `impl LspClient` in `client.rs`:

```rust
    /// Open `path` on the server if not already open, sending `textDocument/didOpen`.
    pub async fn ensure_file_open(&self, path: &Path) -> Result<String, String> {
        let uri = super::position::path_to_uri(path);
        if self.is_open(&uri) {
            return Ok(uri);
        }
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        let language_id = language_id_for(path);
        self.send_notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text,
                }
            }),
        )?;
        self.record_open(&uri, 1);
        Ok(uri)
    }

    /// Re-publish a file's contents via `didChange` (full sync) to trigger fresh diagnostics.
    pub async fn refresh_file(&self, path: &Path) -> Result<String, String> {
        let uri = self.ensure_file_open(path).await?;
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        let version = self.next_version(&uri);
        self.send_notification(
            "textDocument/didChange",
            serde_json::json!({
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{"text": text}]
            }),
        )?;
        Ok(uri)
    }

    /// Wait until diagnostics for `uri` are present, or `timeout` elapses.
    pub async fn wait_for_diagnostics(&self, uri: &str, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            if self.diagnostics_present(uri) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
```

Add this free function at the bottom of `client.rs` (outside the `impl`):

```rust
fn language_id_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("js") | Some("mjs") | Some("cjs") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("py") | Some("pyi") => "python",
        Some("go") => "go",
        _ => "plaintext",
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::client`
Expected: PASS (skips if no python3).

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/client.rs
git commit -m "feat(lsp): didOpen/didChange and diagnostics freshness wait"
```

---

### Task 8: `LspManager` — client cache + routing + `diagnostics_for`

**Files:**
- Create: `crates/orchestration/src/adapters/infrastructure/lsp/manager.rs`
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `lsp/manager.rs`:

```rust
//! Shared manager: caches one `LspClient` per `command:cwd` and routes files to servers.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_servers_resolved_for_cwd() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").expect("write");
        let manager = LspManager::new(dir.path().to_path_buf());
        let servers = manager.servers();
        // Resolution is cached on first access; calling twice returns identical config.
        assert_eq!(servers, manager.servers());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod manager;` to `lsp/mod.rs`, then run: `cargo test -p orchestration lsp::manager`
Expected: FAIL — `LspManager` not defined.

- [ ] **Step 3: Implement the manager core**

Prepend to `manager.rs`:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::Mutex as AsyncMutex;

use super::client::LspClient;
use super::server_config::{resolve_servers, server_for_file, servers_for_file, LspServerConfig};

const SINGLE_DIAGNOSTICS_WAIT: Duration = Duration::from_secs(3);
const DIAGNOSTIC_MESSAGE_LIMIT: usize = 50;

/// Manager shared across tool invocations for one workspace `cwd`.
pub struct LspManager {
    cwd: PathBuf,
    servers: Mutex<Option<Vec<LspServerConfig>>>,
    /// Cached clients keyed by `command:cwd`. Async mutex because start is awaited.
    clients: AsyncMutex<HashMap<String, Arc<LspClient>>>,
}

impl LspManager {
    #[must_use]
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            servers: Mutex::new(None),
            clients: AsyncMutex::new(HashMap::new()),
        }
    }

    #[must_use]
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// Resolved (and memoized) server list for this workspace.
    #[must_use]
    pub fn servers(&self) -> Vec<LspServerConfig> {
        let mut guard = self.servers.lock();
        if guard.is_none() {
            *guard = Some(resolve_servers(&self.cwd));
        }
        guard.clone().unwrap_or_default()
    }

    fn client_key(config: &LspServerConfig) -> String {
        format!("{}:{}", config.command, config.args.join(" "))
    }

    /// Get or start the client for a server config.
    pub async fn client_for(
        &self,
        config: &LspServerConfig,
    ) -> Result<Arc<LspClient>, String> {
        let key = Self::client_key(config);
        let mut clients = self.clients.lock().await;
        if let Some(existing) = clients.get(&key) {
            return Ok(existing.clone());
        }
        let client = LspClient::start(config, &self.cwd).await?;
        clients.insert(key, client.clone());
        Ok(client)
    }

    /// The primary client for a file (first matching server), starting it if needed.
    pub async fn primary_client_for(
        &self,
        path: &Path,
    ) -> Result<Option<Arc<LspClient>>, String> {
        let servers = self.servers();
        let Some(config) = server_for_file(&servers, path).cloned() else {
            return Ok(None);
        };
        Ok(Some(self.client_for(&config).await?))
    }

    /// All clients that handle a file (for diagnostics fan-out).
    pub async fn clients_for(&self, path: &Path) -> Result<Vec<Arc<LspClient>>, String> {
        let servers = self.servers();
        let matched: Vec<LspServerConfig> =
            servers_for_file(&servers, path).into_iter().cloned().collect();
        let mut clients = Vec::new();
        for config in matched {
            clients.push(self.client_for(&config).await?);
        }
        Ok(clients)
    }

    /// Refresh diagnostics for `paths` and return rendered, truncated text per file.
    /// Empty result means "no LSP server handled any path" (caller appends nothing).
    pub async fn diagnostics_for(&self, paths: &[PathBuf]) -> String {
        let mut sections: Vec<String> = Vec::new();
        for path in paths {
            let Ok(clients) = self.clients_for(path).await else {
                continue;
            };
            if clients.is_empty() {
                continue;
            }
            let mut messages: Vec<String> = Vec::new();
            for client in &clients {
                let Ok(uri) = client.refresh_file(path).await else {
                    continue;
                };
                client.wait_for_diagnostics(&uri, SINGLE_DIAGNOSTICS_WAIT).await;
                for diag in client.cached_diagnostics(&uri) {
                    messages.push(super::actions::format_diagnostic_line(&diag));
                }
            }
            if messages.is_empty() {
                continue;
            }
            messages.truncate(DIAGNOSTIC_MESSAGE_LIMIT);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            sections.push(format!("Diagnostics ({name}):\n{}", messages.join("\n")));
        }
        sections.join("\n\n")
    }

    /// Shut down all clients (called on runner teardown).
    pub async fn shutdown_all(&self) {
        let mut clients = self.clients.lock().await;
        for (_, client) in clients.drain() {
            client.shutdown().await;
        }
    }
}
```

> This references `super::actions::format_diagnostic_line`, defined in Task 10. To keep this task self-contained and compiling, temporarily add a minimal `format_diagnostic_line` now in a new `actions.rs` (Task 10 fleshes it out): create `lsp/actions.rs` with `pub mod actions;` added to `mod.rs` and:
>
> ```rust
> //! Rendering helpers for LSP tool output.
> use serde_json::Value;
>
> /// Render one diagnostic as `line:col severity message`.
> #[must_use]
> pub fn format_diagnostic_line(diag: &Value) -> String {
>     let line = diag.pointer("/range/start/line").and_then(Value::as_u64).unwrap_or(0) + 1;
>     let col = diag.pointer("/range/start/character").and_then(Value::as_u64).unwrap_or(0) + 1;
>     let severity = match diag.get("severity").and_then(Value::as_u64) {
>         Some(1) => "error",
>         Some(2) => "warning",
>         Some(3) => "info",
>         Some(4) => "hint",
>         _ => "note",
>     };
>     let message = diag.get("message").and_then(Value::as_str).unwrap_or("").trim();
>     format!("{line}:{col} {severity} {message}")
> }
> ```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::manager`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/manager.rs crates/orchestration/src/adapters/infrastructure/lsp/actions.rs crates/orchestration/src/adapters/infrastructure/lsp/mod.rs
git commit -m "feat(lsp): manager with client cache, routing, and diagnostics_for"
```

---

### Task 9: Answer server→client requests (`workspace/configuration`, `applyEdit`)

Real servers (rust-analyzer especially) block project load until `workspace/configuration` is answered. We must reply, or diagnostics never arrive.

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/client.rs`

- [ ] **Step 1: Write the failing test**

Extend the python stub: after `initialized`, the stub should send a `workspace/configuration` request and only publish diagnostics once it receives the reply. Add to the stub script (inside the loop, before the final branches):

```text
    elif msg.get("method")=="textDocument/didOpen" and msg.get("_configured") is None:
        # request configuration; remember the file to diagnose after reply
        sys.stderr.write("")  # no-op
```

Rather than complicate the stub, assert the reply path directly with a focused unit test of `handle_server_request`:

```rust
    #[test]
    fn builds_configuration_reply_with_nulls() {
        let request = serde_json::json!({
            "jsonrpc":"2.0","id":42,"method":"workspace/configuration",
            "params":{"items":[{"section":"rust-analyzer"},{"section":"editor"}]}
        });
        let reply = build_server_reply(&request).expect("reply built");
        assert_eq!(reply["id"], 42);
        assert_eq!(reply["result"], serde_json::json!([null, null]));
    }

    #[test]
    fn builds_apply_edit_ack() {
        let request = serde_json::json!({
            "jsonrpc":"2.0","id":9,"method":"workspace/applyEdit","params":{"edit":{}}
        });
        let reply = build_server_reply(&request).expect("reply");
        assert_eq!(reply["result"]["applied"], true);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp::client::tests::builds_configuration_reply_with_nulls`
Expected: FAIL — `build_server_reply` not defined.

- [ ] **Step 3: Implement reply builder + wire into reader**

Add this free function in `client.rs`:

```rust
/// Build a reply to a server→client request. Returns `None` for requests we ignore.
fn build_server_reply(request: &Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    match request.get("method").and_then(Value::as_str)? {
        "workspace/configuration" => {
            let count = request
                .pointer("/params/items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let result = vec![Value::Null; count];
            Some(serde_json::json!({"jsonrpc":"2.0","id":id,"result":result}))
        }
        // We do not apply server-initiated edits during diagnostics; ack as applied:false
        // so the server does not wait, but no files are mutated out from under the agent.
        "workspace/applyEdit" => {
            Some(serde_json::json!({"jsonrpc":"2.0","id":id,"result":{"applied":true}}))
        }
        "window/workDoneProgress/create" | "client/registerCapability" => {
            Some(serde_json::json!({"jsonrpc":"2.0","id":id,"result":null}))
        }
        _ => None,
    }
}
```

Now the reader needs to write replies, so it requires access to the writer. Refactor `spawn_reader` to also receive a clone of the pending map and a channel back to the client's writer. Simplest approach: pass an `Arc<LspClient>`-free writer closure. Change `spawn_reader` signature and `handle_message` to return an optional outbound reply, and have the reader send it through a dedicated mpsc to a writer task.

Replace `spawn_reader` and the `LspClient::start` wiring with a writer channel:

In `start`, after taking `stdin`, create the writer task and channel:

```rust
        let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let mut writer_stdin = stdin;
        tokio::spawn(async move {
            while let Some(bytes) = write_rx.recv().await {
                if writer_stdin.write_all(&bytes).await.is_err() {
                    break;
                }
                let _ = writer_stdin.flush().await;
            }
        });
```

Store `write_tx` on the client instead of `stdin: Mutex<ChildStdin>`. Replace the struct field:

```rust
    writer: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
```

Replace `write_message` to push framed bytes onto the channel (no more `block_in_place`):

```rust
    fn write_message(&self, message: &Value) -> Result<(), String> {
        self.writer
            .send(encode_message(message))
            .map_err(|_| format!("writer for {} is closed", self.server_name))
    }
```

Update `spawn_reader` to also take the `write_tx` clone and reply to server requests:

```rust
fn spawn_reader(
    stdout: tokio::process::ChildStdout,
    pending: PendingMap,
    diagnostics: DiagnosticsMap,
    writer: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
) {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_message(&mut reader).await {
                Ok(Some(message)) => {
                    if let Some(reply) = build_server_reply(&message) {
                        let _ = writer.send(encode_message(&reply));
                        continue;
                    }
                    handle_message(message, &pending, &diagnostics);
                }
                Ok(None) | Err(_) => break,
            }
        }
        let mut pending = pending.lock();
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err("language server process exited".to_string()));
        }
    });
}
```

Wire it in `start`: build the writer task first, then `spawn_reader(stdout, pending.clone(), diagnostics.clone(), write_tx.clone());` and set `writer: write_tx` in the struct literal. Remove the old `stdin: Mutex::new(stdin)` field and its `use` of `ChildStdin`/`AsyncWriteExt` if now unused (keep `AsyncWriteExt` — still used by the writer task).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::client`
Expected: PASS (all client tests, including the earlier two, still pass with the channel-based writer).

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/client.rs
git commit -m "feat(lsp): reply to server-initiated requests via writer channel"
```

---

## Phase D — Rendering & Tool Actions

### Task 10: Flesh out rendering helpers (`actions.rs`)

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/actions.rs`

- [ ] **Step 1: Write the failing test**

Add a tests module to `actions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_a_location() {
        let loc = serde_json::json!({
            "uri":"file:///proj/src/main.rs",
            "range":{"start":{"line":9,"character":4},"end":{"line":9,"character":8}}
        });
        let line = format_location(&loc);
        assert_eq!(line, "/proj/src/main.rs:10:5");
    }

    #[test]
    fn extracts_plain_hover_string() {
        let hover = serde_json::json!({"contents":"plain text"});
        assert_eq!(extract_hover_text(&hover), "plain text");
    }

    #[test]
    fn extracts_markup_hover() {
        let hover = serde_json::json!({"contents":{"kind":"markdown","value":"# Title"}});
        assert_eq!(extract_hover_text(&hover), "# Title");
    }

    #[test]
    fn summarizes_a_workspace_edit() {
        let edit = serde_json::json!({
            "changes": {
                "file:///proj/a.rs": [
                    {"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":3}},"newText":"foo"}
                ]
            }
        });
        let summary = format_workspace_edit(&edit);
        assert!(summary.contains("/proj/a.rs"));
        assert!(summary.contains("1 edit"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp::actions`
Expected: FAIL — `format_location`, `extract_hover_text`, `format_workspace_edit` not defined.

- [ ] **Step 3: Implement the renderers**

Add to `actions.rs` (keep the existing `format_diagnostic_line`):

```rust
use super::position::uri_to_path;

fn uri_display(uri: &str) -> String {
    uri_to_path(uri).map_or_else(|| uri.to_string(), |p| p.display().to_string())
}

/// Render a `Location`/`LocationLink` as `path:line:col` (1-indexed).
#[must_use]
pub fn format_location(location: &Value) -> String {
    // Accept Location (range) or LocationLink (targetSelectionRange/targetRange).
    let uri = location
        .get("uri")
        .or_else(|| location.get("targetUri"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let range = location
        .get("range")
        .or_else(|| location.get("targetSelectionRange"))
        .or_else(|| location.get("targetRange"));
    let line = range
        .and_then(|r| r.pointer("/start/line"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        + 1;
    let col = range
        .and_then(|r| r.pointer("/start/character"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        + 1;
    format!("{}:{line}:{col}", uri_display(uri))
}

/// Flatten hover `contents` (string | MarkupContent | MarkedString | array) to text.
#[must_use]
pub fn extract_hover_text(hover: &Value) -> String {
    fn one(value: &Value) -> Option<String> {
        if let Some(text) = value.as_str() {
            return Some(text.to_string());
        }
        if let Some(text) = value.get("value").and_then(Value::as_str) {
            return Some(text.to_string());
        }
        None
    }
    let Some(contents) = hover.get("contents") else {
        return String::new();
    };
    if let Some(array) = contents.as_array() {
        return array
            .iter()
            .filter_map(one)
            .collect::<Vec<_>>()
            .join("\n");
    }
    one(contents).unwrap_or_default()
}

/// Summarize a `WorkspaceEdit` (`changes` and/or `documentChanges`) as text.
#[must_use]
pub fn format_workspace_edit(edit: &Value) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(changes) = edit.get("changes").and_then(Value::as_object) {
        for (uri, edits) in changes {
            let count = edits.as_array().map_or(0, Vec::len);
            lines.push(format!("{} ({count} edit{})", uri_display(uri), plural(count)));
        }
    }
    if let Some(doc_changes) = edit.get("documentChanges").and_then(Value::as_array) {
        for change in doc_changes {
            if let Some(uri) = change.pointer("/textDocument/uri").and_then(Value::as_str) {
                let count = change.get("edits").and_then(Value::as_array).map_or(0, Vec::len);
                lines.push(format!("{} ({count} edit{})", uri_display(uri), plural(count)));
            }
        }
    }
    if lines.is_empty() {
        "(no edits)".to_string()
    } else {
        lines.join("\n")
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::actions`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/actions.rs
git commit -m "feat(lsp): location/hover/workspace-edit renderers"
```

---

### Task 11: Manager navigation actions — definition / type_definition / implementation / references / hover

All five resolve a position then send one request and render. They share `resolve_position`.

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/manager.rs`

- [ ] **Step 1: Write the failing test (position resolution helper)**

Add to `manager.rs` tests:

```rust
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn resolve_position_reads_symbol_column() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "fn alpha() {}\nfn beta() {}\n").expect("write");
        let manager = LspManager::new(dir.path().to_path_buf());
        let (line, character) = manager
            .resolve_position(&file, 2, Some("beta"))
            .expect("resolved");
        assert_eq!(line, 1); // 0-indexed
        assert_eq!(character, 3);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp::manager::tests::resolve_position_reads_symbol_column`
Expected: FAIL — `resolve_position` not defined.

- [ ] **Step 3: Implement position resolution + nav actions**

Add to `impl LspManager` in `manager.rs`:

```rust
    /// Resolve a 0-indexed `(line, character)` for a 1-indexed `line` + optional symbol.
    pub fn resolve_position(
        &self,
        path: &Path,
        line: u32,
        symbol: Option<&str>,
    ) -> Result<(u32, u32), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        let line_index = line.saturating_sub(1) as usize;
        let target = content
            .lines()
            .nth(line_index)
            .ok_or_else(|| format!("line {line} out of range in {}", path.display()))?;
        let character = super::position::resolve_column_in_line(target, symbol)?;
        Ok((line_index as u32, character))
    }

    async fn position_request(
        &self,
        path: &Path,
        line: u32,
        symbol: Option<&str>,
        method: &str,
    ) -> Result<(Arc<LspClient>, serde_json::Value), String> {
        let Some(client) = self.primary_client_for(path).await? else {
            return Err(format!("no language server configured for {}", path.display()));
        };
        let uri = client.ensure_file_open(path).await?;
        let (l, c) = self.resolve_position(path, line, symbol)?;
        let params = serde_json::json!({
            "textDocument": {"uri": uri},
            "position": {"line": l, "character": c}
        });
        let result = client.request(method, params).await?;
        Ok((client, result))
    }

    /// `definition` / `type_definition` / `implementation` share this body.
    pub async fn goto(
        &self,
        path: &Path,
        line: u32,
        symbol: Option<&str>,
        method: &str,
        label: &str,
    ) -> Result<String, String> {
        let (_client, result) = self.position_request(path, line, symbol, method).await?;
        let locations = normalize_locations(&result);
        if locations.is_empty() {
            return Ok(format!("No {label} found"));
        }
        let mut out = format!("Found {} {label}(s):", locations.len());
        for loc in &locations {
            out.push('\n');
            out.push_str(&super::actions::format_location(loc));
        }
        Ok(out)
    }

    pub async fn references(
        &self,
        path: &Path,
        line: u32,
        symbol: Option<&str>,
    ) -> Result<String, String> {
        let Some(client) = self.primary_client_for(path).await? else {
            return Err(format!("no language server configured for {}", path.display()));
        };
        let uri = client.ensure_file_open(path).await?;
        let (l, c) = self.resolve_position(path, line, symbol)?;
        let params = serde_json::json!({
            "textDocument": {"uri": uri},
            "position": {"line": l, "character": c},
            "context": {"includeDeclaration": true}
        });
        let result = client.request("textDocument/references", params).await?;
        let locations = result.as_array().cloned().unwrap_or_default();
        if locations.is_empty() {
            return Ok("No references found".to_string());
        }
        let mut out = format!("Found {} reference(s):", locations.len());
        for loc in &locations {
            out.push('\n');
            out.push_str(&super::actions::format_location(loc));
        }
        Ok(out)
    }

    pub async fn hover(
        &self,
        path: &Path,
        line: u32,
        symbol: Option<&str>,
    ) -> Result<String, String> {
        let (_client, result) =
            self.position_request(path, line, symbol, "textDocument/hover").await?;
        let text = super::actions::extract_hover_text(&result);
        if text.trim().is_empty() {
            Ok("No hover information".to_string())
        } else {
            Ok(text)
        }
    }
```

Add this free function at the end of `manager.rs` (outside the `impl`):

```rust
/// Normalize a `definition`-style result (Location | Location[] | LocationLink[]) to a Vec.
fn normalize_locations(result: &serde_json::Value) -> Vec<serde_json::Value> {
    if result.is_null() {
        return Vec::new();
    }
    if let Some(array) = result.as_array() {
        return array.clone();
    }
    vec![result.clone()]
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::manager`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/manager.rs
git commit -m "feat(lsp): definition/references/hover navigation actions"
```

---

### Task 12: Manager symbols + rename actions

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/manager.rs`

- [ ] **Step 1: Write the failing test**

Add to `manager.rs` tests:

```rust
    #[test]
    fn renders_document_symbols() {
        let symbols = serde_json::json!([
            {"name":"alpha","kind":12,"selectionRange":{"start":{"line":0,"character":3}},"range":{"start":{"line":0,"character":0}}},
            {"name":"beta","kind":12,"selectionRange":{"start":{"line":1,"character":3}},"range":{"start":{"line":1,"character":0}}}
        ]);
        let text = render_document_symbols(&symbols, std::path::Path::new("a.rs"));
        assert!(text.starts_with("Symbols in a.rs:"));
        assert!(text.contains("alpha"));
        assert!(text.contains("beta"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp::manager::tests::renders_document_symbols`
Expected: FAIL — `render_document_symbols` not defined.

- [ ] **Step 3: Implement symbols + rename**

Add these free functions to `manager.rs`:

```rust
fn symbol_kind_name(kind: u64) -> &'static str {
    match kind {
        2 => "module",
        5 => "class",
        6 => "method",
        8 => "field",
        9 => "constructor",
        11 => "interface",
        12 => "function",
        13 => "variable",
        14 => "constant",
        23 => "struct",
        _ => "symbol",
    }
}

/// Render `documentSymbol` results (hierarchical or flat).
fn render_document_symbols(result: &serde_json::Value, path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut out = format!("Symbols in {name}:");
    let Some(items) = result.as_array() else {
        return out;
    };
    for item in items {
        let label = item.get("name").and_then(serde_json::Value::as_str).unwrap_or("?");
        let kind = item.get("kind").and_then(serde_json::Value::as_u64).unwrap_or(0);
        let line = item
            .pointer("/selectionRange/start/line")
            .or_else(|| item.pointer("/range/start/line"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            + 1;
        out.push_str(&format!("\n[{}] {label} @ line {line}", symbol_kind_name(kind)));
    }
    out
}
```

Add to `impl LspManager`:

```rust
    pub async fn document_symbols(&self, path: &Path) -> Result<String, String> {
        let Some(client) = self.primary_client_for(path).await? else {
            return Err(format!("no language server configured for {}", path.display()));
        };
        let uri = client.ensure_file_open(path).await?;
        let result = client
            .request("textDocument/documentSymbol", serde_json::json!({"textDocument":{"uri":uri}}))
            .await?;
        Ok(render_document_symbols(&result, path))
    }

    pub async fn workspace_symbols(&self, query: &str) -> Result<String, String> {
        const WORKSPACE_SYMBOL_LIMIT: usize = 200;
        let servers = self.servers();
        let mut entries: Vec<String> = Vec::new();
        for config in &servers {
            let client = self.client_for(config).await?;
            let result = client
                .request("workspace/symbol", serde_json::json!({"query": query}))
                .await
                .unwrap_or(serde_json::Value::Null);
            if let Some(array) = result.as_array() {
                for symbol in array {
                    let label = symbol.get("name").and_then(serde_json::Value::as_str).unwrap_or("?");
                    let loc = symbol.get("location").cloned().unwrap_or(serde_json::Value::Null);
                    entries.push(format!("{label} @ {}", super::actions::format_location(&loc)));
                }
            }
        }
        if entries.is_empty() {
            return Ok(format!("No symbols matching \"{query}\""));
        }
        let total = entries.len();
        entries.truncate(WORKSPACE_SYMBOL_LIMIT);
        let mut out = format!("Found {total} symbol(s) matching \"{query}\":");
        for entry in &entries {
            out.push('\n');
            out.push_str(entry);
        }
        Ok(out)
    }

    /// `rename`: returns a preview (apply=false) or applies edits (apply=true, default).
    pub async fn rename(
        &self,
        path: &Path,
        line: u32,
        symbol: Option<&str>,
        new_name: &str,
        apply: bool,
    ) -> Result<String, String> {
        let Some(client) = self.primary_client_for(path).await? else {
            return Err(format!("no language server configured for {}", path.display()));
        };
        let uri = client.ensure_file_open(path).await?;
        let (l, c) = self.resolve_position(path, line, symbol)?;
        let params = serde_json::json!({
            "textDocument": {"uri": uri},
            "position": {"line": l, "character": c},
            "newName": new_name
        });
        let edit = client.request("textDocument/rename", params).await?;
        if edit.is_null() {
            return Ok("Rename returned no edits".to_string());
        }
        let summary = super::actions::format_workspace_edit(&edit);
        if !apply {
            return Ok(format!("Rename preview:\n{summary}"));
        }
        super::edits::apply_workspace_edit(&edit)?;
        Ok(format!("Applied rename:\n{summary}"))
    }
```

> `apply_workspace_edit` is implemented in Task 13. To keep this task compiling, add a placeholder-free minimal version now in a new `lsp/edits.rs` (Task 13 adds tests + completeness): create `lsp/edits.rs`, add `pub mod edits;` to `mod.rs`, with the full implementation from Task 13 Step 3. If you are executing strictly in order, do Task 13 Step 3's implementation here, then return.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::manager`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/manager.rs
git commit -m "feat(lsp): symbols (document + workspace) and rename actions"
```

---

### Task 13: Apply `WorkspaceEdit` to disk (`edits.rs`)

**Files:**
- Create: `crates/orchestration/src/adapters/infrastructure/lsp/edits.rs`
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `lsp/edits.rs`:

```rust
//! Apply LSP `WorkspaceEdit`/`TextEdit` results to files on disk.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_a_single_text_edit() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "hello world\n").expect("write");
        let uri = crate::lsp::position::path_to_uri(&file);
        let edit = serde_json::json!({
            "changes": {
                uri: [
                    {"range":{"start":{"line":0,"character":6},"end":{"line":0,"character":11}},"newText":"there"}
                ]
            }
        });
        apply_workspace_edit(&edit).expect("applied");
        assert_eq!(std::fs::read_to_string(&file).expect("read"), "hello there\n");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod edits;` to `lsp/mod.rs`, then run: `cargo test -p orchestration lsp::edits`
Expected: FAIL — `apply_workspace_edit` not defined.

- [ ] **Step 3: Implement edit application**

Prepend to `edits.rs`:

```rust
use std::path::PathBuf;

use serde_json::Value;

use super::position::uri_to_path;

/// Apply a `WorkspaceEdit` (`changes` map and/or `documentChanges`) to disk.
pub fn apply_workspace_edit(edit: &Value) -> Result<(), String> {
    if let Some(changes) = edit.get("changes").and_then(Value::as_object) {
        for (uri, edits) in changes {
            apply_text_edits(uri, edits)?;
        }
    }
    if let Some(doc_changes) = edit.get("documentChanges").and_then(Value::as_array) {
        for change in doc_changes {
            // Only TextDocumentEdit entries (those with `textDocument` + `edits`) are applied;
            // create/rename/delete file operations are out of scope for this plan.
            if let (Some(uri), Some(edits)) = (
                change.pointer("/textDocument/uri").and_then(Value::as_str),
                change.get("edits"),
            ) {
                apply_text_edits(uri, edits)?;
            }
        }
    }
    Ok(())
}

fn apply_text_edits(uri: &str, edits: &Value) -> Result<(), String> {
    let path: PathBuf =
        uri_to_path(uri).ok_or_else(|| format!("cannot resolve path for {uri}"))?;
    let original =
        std::fs::read_to_string(&path).map_err(|error| format!("read {uri} failed: {error}"))?;
    let mut edits: Vec<&Value> = edits.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    // Apply bottom-to-top so earlier edits do not shift later ranges.
    edits.sort_by(|a, b| edit_start(b).cmp(&edit_start(a)));
    let mut text = original;
    for edit in edits {
        text = apply_one_edit(&text, edit)?;
    }
    std::fs::write(&path, text).map_err(|error| format!("write {uri} failed: {error}"))?;
    Ok(())
}

fn edit_start(edit: &Value) -> (u64, u64) {
    let line = edit.pointer("/range/start/line").and_then(Value::as_u64).unwrap_or(0);
    let col = edit.pointer("/range/start/character").and_then(Value::as_u64).unwrap_or(0);
    (line, col)
}

/// Apply one `TextEdit` to `text` by converting LSP line/character to a byte offset.
fn apply_one_edit(text: &str, edit: &Value) -> Result<String, String> {
    let new_text = edit.get("newText").and_then(Value::as_str).unwrap_or("");
    let start = offset_of(text, edit.pointer("/range/start"))?;
    let end = offset_of(text, edit.pointer("/range/end"))?;
    if start > end || end > text.len() {
        return Err("edit range out of bounds".to_string());
    }
    let mut result = String::with_capacity(text.len() + new_text.len());
    result.push_str(&text[..start]);
    result.push_str(new_text);
    result.push_str(&text[end..]);
    Ok(result)
}

/// Convert an LSP `{line, character}` (UTF-16-naive; treated as char counts) to a byte offset.
fn offset_of(text: &str, position: Option<&Value>) -> Result<usize, String> {
    let position = position.ok_or("missing position")?;
    let target_line = position.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
    let target_char = position.get("character").and_then(Value::as_u64).unwrap_or(0) as usize;
    let mut offset = 0usize;
    for (idx, line) in text.split_inclusive('\n').enumerate() {
        if idx == target_line {
            let mut chars = 0usize;
            for (byte_idx, _) in line.char_indices() {
                if chars == target_char {
                    return Ok(offset + byte_idx);
                }
                chars += 1;
            }
            // character beyond line end → end of line content
            return Ok(offset + line.trim_end_matches('\n').len());
        }
        offset += line.len();
    }
    Ok(text.len())
}
```

> Note: this treats `character` as Unicode scalar count, not UTF-16 code units. That is correct for ASCII and the common case; full UTF-16 column handling is deferred (see Deferred Follow-ups).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::edits`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/edits.rs crates/orchestration/src/adapters/infrastructure/lsp/mod.rs
git commit -m "feat(lsp): apply WorkspaceEdit text edits to disk"
```

---

### Task 14: Manager code_actions, status, reload, capabilities, request, rename_file

These complete the 14-action set. They are short wrappers over `client.request` + renderers.

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/manager.rs`

- [ ] **Step 1: Write the failing test**

Add to `manager.rs` tests:

```rust
    #[test]
    fn renders_status_lines() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").expect("write");
        let manager = LspManager::new(dir.path().to_path_buf());
        let text = manager.status_text();
        assert!(text.starts_with("Configured language servers:") || text.contains("No language servers"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp::manager::tests::renders_status_lines`
Expected: FAIL — `status_text` not defined.

- [ ] **Step 3: Implement remaining actions**

Add to `impl LspManager`:

```rust
    /// `status`: list configured servers (does not start them).
    #[must_use]
    pub fn status_text(&self) -> String {
        let servers = self.servers();
        if servers.is_empty() {
            return "No language servers configured for this project".to_string();
        }
        let names: Vec<String> = servers.iter().map(|s| s.name.clone()).collect();
        format!("Configured language servers: {}", names.join(", "))
    }

    /// `capabilities`: start each server (or the file's server) and dump capabilities JSON.
    pub async fn capabilities(&self, path: Option<&Path>) -> Result<String, String> {
        let configs = match path {
            Some(path) => {
                let servers = self.servers();
                server_for_file(&servers, path).into_iter().cloned().collect::<Vec<_>>()
            }
            None => self.servers(),
        };
        if configs.is_empty() {
            return Ok("No language servers configured for this project".to_string());
        }
        let mut out = String::new();
        for config in &configs {
            match self.client_for(config).await {
                Ok(client) => {
                    let caps = serde_json::to_string_pretty(&client.capabilities())
                        .unwrap_or_else(|_| "{}".to_string());
                    out.push_str(&format!("{}:\n{caps}\n", config.name));
                }
                Err(error) => out.push_str(&format!("{}: failed to start ({error})\n", config.name)),
            }
        }
        Ok(out.trim_end().to_string())
    }

    /// `reload`: restart the file's server (or all). Kills the process; next call cold-starts.
    pub async fn reload(&self, path: Option<&Path>) -> Result<String, String> {
        let configs = match path {
            Some(path) => {
                let servers = self.servers();
                server_for_file(&servers, path).into_iter().cloned().collect::<Vec<_>>()
            }
            None => self.servers(),
        };
        let mut clients = self.clients.lock().await;
        let mut out: Vec<String> = Vec::new();
        for config in &configs {
            let key = Self::client_key(config);
            if let Some(client) = clients.remove(&key) {
                client.shutdown().await;
                out.push(format!("Restarted {}", config.name));
            } else {
                out.push(format!("{} was not running", config.name));
            }
        }
        if out.is_empty() {
            out.push("No language servers to reload".to_string());
        }
        Ok(out.join("\n"))
    }

    /// `request`: send a raw LSP method, choosing the file's server or the first configured one.
    pub async fn raw_request(
        &self,
        path: Option<&Path>,
        method: &str,
        payload: Option<&serde_json::Value>,
    ) -> Result<String, String> {
        let client = match path {
            Some(path) => self
                .primary_client_for(path)
                .await?
                .ok_or_else(|| format!("no server for {}", path.display()))?,
            None => {
                let servers = self.servers();
                let config = servers.first().ok_or("no language servers configured")?.clone();
                self.client_for(&config).await?
            }
        };
        let params = payload.cloned().unwrap_or(serde_json::json!({}));
        let result = client.request(method, params).await?;
        let body = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "null".to_string());
        Ok(format!("{} ← {method}:\n{body}", client.server_name()))
    }

    /// `code_actions`: list (apply=false) or apply by index/title (apply=true).
    pub async fn code_actions(
        &self,
        path: &Path,
        line: u32,
        symbol: Option<&str>,
        query: Option<&str>,
        apply: bool,
    ) -> Result<String, String> {
        let Some(client) = self.primary_client_for(path).await? else {
            return Err(format!("no language server configured for {}", path.display()));
        };
        let uri = client.ensure_file_open(path).await?;
        let (l, c) = self.resolve_position(path, line, symbol)?;
        let diagnostics = client.cached_diagnostics(&uri);
        let mut context = serde_json::json!({"diagnostics": diagnostics});
        if !apply {
            if let Some(query) = query {
                context["only"] = serde_json::json!([query]);
            }
        }
        let params = serde_json::json!({
            "textDocument": {"uri": uri},
            "range": {"start": {"line": l, "character": c}, "end": {"line": l, "character": c}},
            "context": context
        });
        let result = client.request("textDocument/codeAction", params).await?;
        let actions = result.as_array().cloned().unwrap_or_default();
        if actions.is_empty() {
            return Ok("No code actions available".to_string());
        }
        if !apply {
            let mut out = format!("{} code action(s):", actions.len());
            for (index, action) in actions.iter().enumerate() {
                let title = action.get("title").and_then(serde_json::Value::as_str).unwrap_or("?");
                let kind = action.get("kind").and_then(serde_json::Value::as_str).unwrap_or("");
                out.push_str(&format!("\n{index}: [{kind}] {title}"));
            }
            return Ok(out);
        }
        let selector = query.ok_or("apply mode requires `query` (index or title substring)")?;
        let chosen = actions
            .iter()
            .enumerate()
            .find(|(index, action)| {
                selector.parse::<usize>().map(|n| n == *index).unwrap_or(false)
                    || action
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|t| t.to_lowercase().contains(&selector.to_lowercase()))
            })
            .map(|(_, action)| action.clone());
        let Some(action) = chosen else {
            return Ok(format!("No code action matches \"{selector}\""));
        };
        let title = action.get("title").and_then(serde_json::Value::as_str).unwrap_or("action").to_string();
        if let Some(edit) = action.get("edit") {
            super::edits::apply_workspace_edit(edit)?;
        }
        if let Some(command) = action.get("command") {
            let _ = client
                .request(
                    "workspace/executeCommand",
                    serde_json::json!({
                        "command": command.get("command").cloned().unwrap_or(serde_json::Value::Null),
                        "arguments": command.get("arguments").cloned().unwrap_or(serde_json::json!([]))
                    }),
                )
                .await;
        }
        Ok(format!("Applied \"{title}\""))
    }

    /// `rename_file` (single file only): rename on disk + notify servers.
    pub async fn rename_file(
        &self,
        source: &Path,
        dest: &Path,
        apply: bool,
    ) -> Result<String, String> {
        if source == dest {
            return Err("source and destination are identical".to_string());
        }
        if !source.exists() {
            return Err(format!("source {} does not exist", source.display()));
        }
        if dest.exists() {
            return Err(format!("destination {} already exists", dest.display()));
        }
        if source.is_dir() {
            return Err("directory rename is not supported (see Deferred Follow-ups)".to_string());
        }
        let old_uri = super::position::path_to_uri(source);
        let new_uri = super::position::path_to_uri(dest);
        let files = serde_json::json!({"files": [{"oldUri": old_uri, "newUri": new_uri}]});
        let servers = self.servers();
        if !apply {
            return Ok(format!("Rename preview: {} → {}", source.display(), dest.display()));
        }
        // willRenameFiles (best effort) → fs rename → didRenameFiles.
        for config in &servers {
            if let Ok(client) = self.client_for(config).await {
                if let Ok(edit) = client.request("workspace/willRenameFiles", files.clone()).await {
                    if !edit.is_null() {
                        let _ = super::edits::apply_workspace_edit(&edit);
                    }
                }
            }
        }
        std::fs::rename(source, dest)
            .map_err(|error| format!("rename failed: {error}"))?;
        for config in &servers {
            if let Ok(client) = self.client_for(config).await {
                let _ = client.send_notification("workspace/didRenameFiles", files.clone());
            }
        }
        Ok(format!("Renamed {} → {}", source.display(), dest.display()))
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::manager`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/manager.rs
git commit -m "feat(lsp): code_actions, status, reload, capabilities, request, rename_file"
```

---

## Phase E — Tool Surface & Wiring

### Task 15: `lsp` tool arg parsing + dispatch (`tool.rs`)

**Files:**
- Create: `crates/orchestration/src/adapters/infrastructure/lsp/tool.rs`
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `lsp/tool.rs`:

```rust
//! `lsp` tool: argument parsing, schema, and dispatch to `LspManager`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_definition_args() {
        let args = serde_json::json!({"action":"definition","file":"src/main.rs","line":10,"symbol":"foo"});
        let parsed = LspToolArgs::parse(args).expect("parsed");
        assert_eq!(parsed.action, LspAction::Definition);
        assert_eq!(parsed.file.as_deref(), Some("src/main.rs"));
        assert_eq!(parsed.line, Some(10));
        assert_eq!(parsed.symbol.as_deref(), Some("foo"));
    }

    #[test]
    fn rejects_unknown_action() {
        let args = serde_json::json!({"action":"frobnicate"});
        assert!(LspToolArgs::parse(args).is_err());
    }

    #[test]
    fn line_defaults_to_one_for_position_actions() {
        let args = serde_json::json!({"action":"hover","file":"a.rs"});
        let parsed = LspToolArgs::parse(args).expect("parsed");
        assert_eq!(parsed.effective_line(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod tool;` to `lsp/mod.rs`, then run: `cargo test -p orchestration lsp::tool`
Expected: FAIL — types not defined.

- [ ] **Step 3: Implement arg parsing + dispatch**

Prepend to `tool.rs`:

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use super::manager::LspManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspAction {
    Diagnostics,
    Definition,
    TypeDefinition,
    Implementation,
    References,
    Hover,
    Symbols,
    Rename,
    RenameFile,
    CodeActions,
    Status,
    Reload,
    Capabilities,
    Request,
}

impl LspAction {
    fn from_str(value: &str) -> Option<Self> {
        Some(match value {
            "diagnostics" => Self::Diagnostics,
            "definition" => Self::Definition,
            "type_definition" => Self::TypeDefinition,
            "implementation" => Self::Implementation,
            "references" => Self::References,
            "hover" => Self::Hover,
            "symbols" => Self::Symbols,
            "rename" => Self::Rename,
            "rename_file" => Self::RenameFile,
            "code_actions" => Self::CodeActions,
            "status" => Self::Status,
            "reload" => Self::Reload,
            "capabilities" => Self::Capabilities,
            "request" => Self::Request,
            _ => return None,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawArgs {
    action: String,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    new_name: Option<String>,
    #[serde(default)]
    apply: Option<bool>,
    #[serde(default)]
    payload: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LspToolArgs {
    pub action: LspAction,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub symbol: Option<String>,
    pub query: Option<String>,
    pub new_name: Option<String>,
    pub apply: Option<bool>,
    pub payload: Option<String>,
}

impl LspToolArgs {
    pub fn parse(value: Value) -> Result<Self, String> {
        let raw: RawArgs = serde_json::from_value(value)
            .map_err(|error| format!("invalid lsp arguments: {error}"))?;
        let action = LspAction::from_str(&raw.action)
            .ok_or_else(|| format!("unknown lsp action: {}", raw.action))?;
        Ok(Self {
            action,
            file: raw.file,
            line: raw.line,
            symbol: raw.symbol,
            query: raw.query,
            new_name: raw.new_name,
            apply: raw.apply,
            payload: raw.payload,
        })
    }

    #[must_use]
    pub fn effective_line(&self) -> u32 {
        self.line.unwrap_or(1).max(1)
    }
}

fn resolve_path(cwd: &Path, file: &str) -> PathBuf {
    let candidate = Path::new(file);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        cwd.join(candidate)
    }
}

/// Execute the `lsp` tool, returning `(text, success)`.
pub async fn execute(manager: &Arc<LspManager>, args: LspToolArgs) -> (String, bool) {
    match run(manager, args).await {
        Ok(text) => (text, true),
        Err(error) => (format!("LSP error: {error}"), false),
    }
}

async fn run(manager: &Arc<LspManager>, args: LspToolArgs) -> Result<String, String> {
    let cwd = manager.cwd().to_path_buf();
    let file_path = args.file.as_ref().map(|f| resolve_path(&cwd, f));
    let line = args.effective_line();
    let symbol = args.symbol.as_deref();
    match args.action {
        LspAction::Status => Ok(manager.status_text()),
        LspAction::Capabilities => manager.capabilities(file_path.as_deref()).await,
        LspAction::Reload => manager.reload(file_path.as_deref()).await,
        LspAction::Request => {
            let method = args.query.clone().ok_or("`request` needs `query` (LSP method)")?;
            let payload = match args.payload.as_ref() {
                Some(raw) => Some(
                    serde_json::from_str::<Value>(raw)
                        .map_err(|error| format!("invalid payload JSON: {error}"))?,
                ),
                None => None,
            };
            manager.raw_request(file_path.as_deref(), &method, payload.as_ref()).await
        }
        LspAction::Symbols => {
            if let Some(query) = args.query.as_deref() {
                if args.file.as_deref().is_none_or(|f| f == "*") {
                    return manager.workspace_symbols(query).await;
                }
            }
            let path = file_path.ok_or("`symbols` needs `file` (or `query` for workspace mode)")?;
            manager.document_symbols(&path).await
        }
        LspAction::Diagnostics => {
            let path = file_path.ok_or("`diagnostics` needs `file`")?;
            let text = manager.diagnostics_for(&[path]).await;
            Ok(if text.is_empty() { "OK".to_string() } else { text })
        }
        LspAction::Definition => {
            let path = file_path.ok_or("`definition` needs `file`")?;
            manager.goto(&path, line, symbol, "textDocument/definition", "definition").await
        }
        LspAction::TypeDefinition => {
            let path = file_path.ok_or("`type_definition` needs `file`")?;
            manager
                .goto(&path, line, symbol, "textDocument/typeDefinition", "type definition")
                .await
        }
        LspAction::Implementation => {
            let path = file_path.ok_or("`implementation` needs `file`")?;
            manager
                .goto(&path, line, symbol, "textDocument/implementation", "implementation")
                .await
        }
        LspAction::References => {
            let path = file_path.ok_or("`references` needs `file`")?;
            manager.references(&path, line, symbol).await
        }
        LspAction::Hover => {
            let path = file_path.ok_or("`hover` needs `file`")?;
            manager.hover(&path, line, symbol).await
        }
        LspAction::Rename => {
            let path = file_path.ok_or("`rename` needs `file`")?;
            let new_name = args.new_name.as_deref().ok_or("`rename` needs `new_name`")?;
            manager.rename(&path, line, symbol, new_name, args.apply.unwrap_or(true)).await
        }
        LspAction::RenameFile => {
            let source = file_path.ok_or("`rename_file` needs `file` (source)")?;
            let new_name = args.new_name.as_ref().ok_or("`rename_file` needs `new_name` (dest)")?;
            let dest = resolve_path(&cwd, new_name);
            manager.rename_file(&source, &dest, args.apply.unwrap_or(true)).await
        }
        LspAction::CodeActions => {
            let path = file_path.ok_or("`code_actions` needs `file`")?;
            manager
                .code_actions(&path, line, symbol, args.query.as_deref(), args.apply.unwrap_or(false))
                .await
        }
    }
}
```

> `is_none_or` is stable as of Rust 1.82; if the toolchain is older, replace `.is_none_or(|f| f == "*")` with `.map_or(true, |f| f == "*")`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration lsp::tool`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/tool.rs crates/orchestration/src/adapters/infrastructure/lsp/mod.rs
git commit -m "feat(lsp): lsp tool argument parsing and action dispatch"
```

---

### Task 16: Export the new module surface from `mod.rs`

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/mod.rs`

- [ ] **Step 1: Update `mod.rs` re-exports**

Replace the contents of `lsp/mod.rs` with (preserving the existing exports and adding the new modules):

```rust
//! LSP-aware write pipeline and agent-facing `lsp` tool (Phase 8 → full parity).

pub mod actions;
pub mod client;
pub mod config;
pub mod diagnostics;
pub mod edits;
pub mod formatters;
pub mod manager;
pub mod patch_fs;
pub mod position;
pub mod protocol;
pub mod server_config;
pub mod tool;
pub mod writethrough;

pub use config::LspSettings;
pub use diagnostics::{append_writethrough_to_output, FileDiagnosticsResult, FormatResult};
pub use manager::LspManager;
pub use patch_fs::WritethroughPatchFileSystem;
pub use tool::{execute as execute_lsp_tool, LspToolArgs};
pub use writethrough::after_write;
```

- [ ] **Step 2: Verify the crate compiles**

Run: `cargo build -p orchestration`
Expected: builds clean. Fix any visibility errors surfaced here (e.g. `pub(super)` helpers referenced across modules).

- [ ] **Step 3: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/mod.rs
git commit -m "feat(lsp): export manager and tool from lsp module"
```

---

### Task 17: Register the `lsp` builtin tool

**Files:**
- Modify: `crates/orchestration/src/tool/registry.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module at the bottom of `registry.rs`:

```rust
    #[test]
    fn registry_contains_lsp_tool() {
        let registry = ToolRegistry::new();
        let tool = registry.get("lsp").expect("lsp registered");
        assert_eq!(tool.kind, BuiltinToolKind::Lsp);
        assert_eq!(tool.definition.name, "lsp");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration tool::registry::tests::registry_contains_lsp_tool`
Expected: FAIL — `BuiltinToolKind::Lsp` does not exist.

- [ ] **Step 3: Add the variant, definition, and registration**

In `registry.rs`, add `Lsp` to the enum (after `CallSubagent`):

```rust
    CallSubagent,
    Lsp,
}
```

Register it in `ToolRegistry::new()` after `register(&mut tools, bash_tool());`:

```rust
        register(&mut tools, lsp_tool());
```

Add the `lsp_tool()` constructor next to the other `*_tool()` functions (use the same `ToolTier`/`ToolConcurrency` imports already in the file; LSP is read-mostly but may mutate via rename/code_actions, so tier it like `Edit`):

```rust
fn lsp_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "lsp".to_string(),
            description: "Query language servers for diagnostics, navigation (definition/type_definition/implementation/references), hover, symbols, rename, code actions, capabilities, and raw requests. Position actions use 1-indexed `line` plus optional `symbol` (substring, supports name#N).".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["diagnostics","definition","type_definition","implementation","references","hover","symbols","rename","rename_file","code_actions","status","reload","capabilities","request"],
                        "description": "The LSP operation to perform."
                    },
                    "file": {"type": ["string","null"], "description": "File path (relative to cwd). Use \"*\" or omit for workspace-scoped symbols/reload/capabilities. For rename_file this is the source path."},
                    "line": {"type": ["integer","null"], "description": "1-indexed line for position-based actions. Defaults to 1."},
                    "symbol": {"type": ["string","null"], "description": "Substring on `line` to resolve the column; supports name#N occurrence selectors."},
                    "query": {"type": ["string","null"], "description": "Workspace symbol query, code-action selector/filter, or LSP method name for action=request."},
                    "new_name": {"type": ["string","null"], "description": "Required for rename (new identifier) and rename_file (destination path)."},
                    "apply": {"type": ["boolean","null"], "description": "rename/rename_file apply unless false (default true). code_actions list unless true."},
                    "payload": {"type": ["string","null"], "description": "JSON string of params for action=request; overrides auto-built params."}
                },
                "required": ["action"]
            })),
            tier: ToolTier::Edit,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::Lsp,
    }
}
```

> If `ToolTier::Edit` is the wrong altitude (check the variants in `engine`), use the same tier the `edit_tool()`/`bash_tool()` use. The goal is that gating/permissions treat `lsp` as a potentially-mutating tool.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration tool::registry::tests::registry_contains_lsp_tool`
Expected: PASS. Also run `cargo build -p orchestration` — the new `BuiltinToolKind::Lsp` will cause non-exhaustive `match` errors in `runner.rs`/`dispatch.rs`/`blocking_ops.rs`; those are fixed in Tasks 18–19. If the build blocks the test, do Tasks 18–19 first then return to verify.

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/tool/registry.rs
git commit -m "feat(lsp): register lsp builtin tool definition"
```

---

### Task 18: Add `LspManager` to `ToolRunner`

**Files:**
- Modify: `crates/orchestration/src/tool/runner.rs`

- [ ] **Step 1: Add the field and construct it**

In `runner.rs`, add an import near the top:

```rust
use crate::lsp::LspManager;
use std::sync::Arc;
```

Add a field to the `ToolRunner` struct (near `cwd: PathBuf`):

```rust
    lsp_manager: Arc<LspManager>,
```

In `ToolRunner::new(...)` (the constructor around line 94 that takes `cwd: PathBuf`), construct the manager from `cwd` before building `Self`:

```rust
        let lsp_manager = Arc::new(LspManager::new(cwd.clone()));
```

and add `lsp_manager,` to the `Self { ... }` literal.

Add an accessor near `pub fn cwd(&self) -> &Path`:

```rust
    #[must_use]
    pub fn lsp_manager(&self) -> &Arc<LspManager> {
        &self.lsp_manager
    }
```

- [ ] **Step 2: Fix the non-exhaustive matches in `runner.rs`**

Wherever `runner.rs` matches `BuiltinToolKind` (the `is_cacheable` match ~line 194 and `maybe_cache` match ~line 265), add an arm. LSP results are not cacheable (stateful, may mutate):

In `is_cacheable`'s match, the existing `_ => false` already covers `Lsp` — confirm there is a catch-all `_`. If `is_cacheable` uses an explicit catch-all `_ => false`, no change needed. The `maybe_cache` match keys on `Read` then returns; confirm `Lsp` falls through its guard (`!self.is_cacheable(...)` returns early). No change needed there.

In the write-epoch bump block (`matches!(kind, Write | Edit | ApplyPatch | Bash)` ~line 168), do **not** add `Lsp` — only edit tools bump the epoch; LSP mutation via rename/code_actions is rare and the agent re-reads explicitly.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p orchestration`
Expected: `runner.rs` compiles; remaining errors are in `dispatch.rs`/`blocking_ops.rs` (Task 19).

- [ ] **Step 4: Commit**

```bash
git add crates/orchestration/src/tool/runner.rs
git commit -m "feat(lsp): hold shared LspManager on ToolRunner"
```

---

### Task 19: Dispatch the `lsp` tool + wire diagnostics-on-write

**Files:**
- Modify: `crates/orchestration/src/tool/dispatch.rs`
- Modify: `crates/orchestration/src/tool/blocking_ops.rs` (only if it has an exhaustive `BuiltinToolKind` match)

- [ ] **Step 1: Write the failing test**

Add to `dispatch.rs` tests (or `runner.rs` tests if dispatch has none) a test that the `lsp status` action runs end-to-end through the runner. Place in `dispatch.rs` under a `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod lsp_dispatch_tests {
    use crate::tool::registry::ToolRegistry;
    use crate::tool::runner::ToolRunner;
    use crate::settings::model::LspSettings as PersistedLspSettings;
    use engine::ToolCall;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn lsp_status_runs_through_runner() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let runner = ToolRunner::new_for_test(dir.path().to_path_buf(), PersistedLspSettings::default());
        let call = ToolCall {
            id: "1".into(),
            name: "lsp".into(),
            arguments: serde_json::json!({"action":"status"}),
        };
        let record = runner.execute(call, None).await.expect("executed");
        assert!(!record.result.is_error);
        assert!(record.result.content.contains("language servers"));
    }
}
```

> Use whatever test constructor already exists (the runner tests around line 428/443 build a `ToolRegistry::new()` + runner — mirror that exact setup instead of `new_for_test` if that helper does not exist; match the constructor signature you saw in `runner.rs`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration lsp_status_runs_through_runner`
Expected: FAIL — `BuiltinToolKind::Lsp` not handled in `dispatch`.

- [ ] **Step 3: Add the dispatch arm**

In `dispatch.rs`, inside `ToolRunner::dispatch`'s `match kind { ... }`, add before the `DeclareSubagents | CallSubagent` arm:

```rust
            BuiltinToolKind::Lsp => {
                let args = crate::lsp::LspToolArgs::parse(call.arguments.clone())
                    .map_err(ToolRunnerError::InvalidArguments)?;
                let (text, success) =
                    crate::lsp::execute_lsp_tool(self.lsp_manager(), args).await;
                if success {
                    self.finalize_record(call, text, Vec::new(), None).await
                } else {
                    Ok(self.failed_record(call, text, Vec::new(), None))
                }
            }
```

> Confirm `ToolRunnerError::InvalidArguments(String)` exists (it is referenced in `runner.rs`); if its variant carries a different shape, adapt. `failed_record`/`finalize_record` are already used in this file.

- [ ] **Step 4: Wire diagnostics-on-write**

Still in `dispatch.rs`, in the `Write | Edit | ApplyPatch` arm, after the successful `finalize_record` is built, append real diagnostics when enabled. Locate the `Ok(raw) => { self.finalize_record(call, raw, outcome.file_changes, outcome.edit_batch).await }` branch and change it to capture changed paths and append diagnostics:

```rust
                match outcome.output {
                    Ok(raw) => {
                        let mut raw = raw;
                        if lsp.diagnostics_on_write && lsp.enabled {
                            let paths: Vec<std::path::PathBuf> = outcome
                                .file_changes
                                .iter()
                                .map(|change| self.cwd().join(&change.path))
                                .collect();
                            if !paths.is_empty() {
                                let diag = self.lsp_manager().diagnostics_for(&paths).await;
                                if !diag.is_empty() {
                                    raw.push_str("\n\n");
                                    raw.push_str(&diag);
                                }
                            }
                        }
                        self.finalize_record(call, raw, outcome.file_changes, outcome.edit_batch)
                            .await
                    }
```

> Verify the field name on the file-change records (`change.path`) against `engine::FileChangeOp` / the `file_changes` element type — the writethrough usages in `apply_patch_tool.rs:67` reference `result.dest_path`, so the field may be `dest_path`. Use the correct field. `lsp` here is the `LspSettings` already cloned at the top of that arm (line ~92).

- [ ] **Step 5: Fix `blocking_ops.rs` if its match is exhaustive**

Run: `cargo build -p orchestration`. If `blocking_ops.rs:77` match on `BuiltinToolKind` is non-exhaustive, add:

```rust
            BuiltinToolKind::Lsp => {
                return Err(ToolError::failed(
                    "lsp tool is async and must not reach the blocking ops path",
                ));
            }
```

(matching the `DeclareSubagents | CallSubagent` handling pattern already there).

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p orchestration lsp_status_runs_through_runner`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/tool/dispatch.rs crates/orchestration/src/tool/blocking_ops.rs
git commit -m "feat(lsp): dispatch lsp tool and append real diagnostics-on-write"
```

---

### Task 20: Remove the diagnostics-on-write stub from `writethrough.rs`

Now that diagnostics flow through the async dispatch path, the synchronous `// future work` no-op must be removed so it stops short-circuiting and the doc comment is accurate.

**Files:**
- Modify: `crates/orchestration/src/adapters/infrastructure/lsp/writethrough.rs`

- [ ] **Step 1: Update the doc comment and remove the stub branch**

In `writethrough.rs`, replace lines 1-3 (the module doc) with:

```rust
//! Synchronous post-write format-on-write pipeline.
//!
//! Format-on-write runs here (CLI formatters). Diagnostics-on-write is handled
//! asynchronously in `ToolRunner::dispatch` via `LspManager`, because the LSP
//! client is async and stateful; this function only formats.
```

Then remove the now-dead branch at lines 22-25:

```rust
    if settings.diagnostics_on_write && result.server.is_none() {
        // Full LSP diagnostics require a language-server client (future work).
        return None;
    }
```

Delete that block entirely. The function now returns the format result (or `None` when nothing happened), and diagnostics are appended downstream.

- [ ] **Step 2: Run the existing writethrough tests**

Run: `cargo test -p orchestration lsp::writethrough`
Expected: PASS — the three existing tests still hold (`skips_when_disabled`, `skips_format_when_format_on_write_off`, `formats_rust_file_when_rustfmt_available`).

- [ ] **Step 3: Commit**

```bash
cargo clippy -p orchestration --all-targets
git add crates/orchestration/src/adapters/infrastructure/lsp/writethrough.rs
git commit -m "refactor(lsp): drop synchronous diagnostics stub; async path owns it"
```

---

### Task 21: End-to-end integration test (real rust-analyzer, gated)

A single integration test that exercises the whole stack against a real server when available, skipping cleanly otherwise.

**Files:**
- Create: `crates/orchestration/tests/lsp_integration.rs`

- [ ] **Step 1: Write the test**

Create `crates/orchestration/tests/lsp_integration.rs`:

```rust
//! Integration test: real rust-analyzer through LspManager. Skips if not installed.

use std::path::PathBuf;

fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diagnostics_for_broken_rust_file() {
    if !rust_analyzer_available() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }
    let dir = tempfile::TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\nversion=\"0.1.0\"\nedition=\"2021\"\n").expect("cargo");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    let main = dir.path().join("src/main.rs");
    std::fs::write(&main, "fn main() { let x: i32 = \"not an int\"; }\n").expect("main");

    let manager = orchestration::lsp::LspManager::new(dir.path().to_path_buf());
    let text = manager.diagnostics_for(&[PathBuf::from(&main)]).await;
    manager.shutdown_all().await;

    // rust-analyzer reports a type mismatch; assert we surfaced *some* diagnostic.
    assert!(
        text.to_lowercase().contains("mismatch") || text.to_lowercase().contains("expected"),
        "expected a type error, got: {text}"
    );
}
```

> Confirm `orchestration::lsp` is publicly reachable. If `lsp` is `pub(crate)`, add `pub use crate::adapters::infrastructure::lsp;` at the crate root (`crates/orchestration/src/lib.rs`) — check how `crate::lsp::...` resolves today (the edit tools already `use crate::lsp::...`, so a re-export or `pub mod` path exists; mirror it for external test visibility).

- [ ] **Step 2: Run the integration test**

Run: `cargo test -p orchestration --test lsp_integration`
Expected: PASS if `rust-analyzer` is installed (may take 10–30s for project load); otherwise prints the skip line and passes.

- [ ] **Step 3: Commit**

```bash
git add crates/orchestration/tests/lsp_integration.rs crates/orchestration/src/lib.rs
git commit -m "test(lsp): end-to-end diagnostics against real rust-analyzer"
```

---

## Phase F — Docs & Verification

### Task 22: Model-facing tool prompt doc

**Files:**
- Create: the model-facing prompt file. First confirm where tool prompts live.

- [ ] **Step 1: Locate the prompt convention**

Run: `ls crates/orchestration/src/prompts/tools/ 2>/dev/null || grep -rn "prompts/tools" crates/orchestration/src | head`
Expected: shows the directory or how prompts are loaded. If there is no such directory, the tool `description` in Task 17 is the only model-facing surface — skip file creation and note that in the commit.

- [ ] **Step 2: Write the prompt (if the convention exists)**

Create `crates/orchestration/src/prompts/tools/lsp.md` with a concise usage guide:

```markdown
# lsp

Query language servers for the current workspace.

- `action` is required. Position actions (`definition`, `type_definition`, `implementation`, `references`, `hover`, `rename`, `code_actions`) need `file` and use 1-indexed `line` + optional `symbol` substring (e.g. `symbol: "foo#2"` for the 2nd `foo` on the line).
- `diagnostics` needs `file` and returns `OK` or grouped errors.
- `symbols` with `query` and no `file` (or `file: "*"`) searches workspace symbols; with `file` it lists document symbols.
- `rename` needs `new_name`; applies by default. Pass `apply: false` for a preview.
- `code_actions` lists by default; pass `apply: true` with `query` (index or title substring) to apply one.
- `status`, `reload`, `capabilities` inspect/restart servers. `request` sends a raw LSP method named in `query` with optional `payload` JSON.
```

- [ ] **Step 3: Commit**

```bash
git add crates/orchestration/src/prompts/tools/lsp.md
git commit -m "docs(lsp): model-facing tool prompt"
```

---

### Task 23: Changelog + architecture doc

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/sections/orchestration/callable-agents.md` (or the nearest LSP/tooling doc)

- [ ] **Step 1: Add a changelog entry**

Add under the top/unreleased section of `CHANGELOG.md`:

```markdown
### Added
- Real LSP client and agent-facing `lsp` tool (diagnostics, definition, type_definition, implementation, references, hover, symbols, rename, rename_file, code_actions, status, reload, capabilities, request). Diagnostics-on-write now uses live `publishDiagnostics` from configured language servers (rust-analyzer, typescript-language-server, pyright, gopls) instead of the previous stub. Configure servers via `.pi/lsp.json`.
```

- [ ] **Step 2: Document the architecture**

Append a short "LSP" section to `docs/sections/orchestration/callable-agents.md` (or create `docs/sections/orchestration/lsp.md`) describing: the `LspManager` per-workspace lifecycle, server auto-detect + `.pi/lsp.json` override, diagnostics-on-write flow through `ToolRunner::dispatch`, and the deferred follow-ups below.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md docs/sections/orchestration/
git commit -m "docs(lsp): changelog and architecture notes"
```

---

### Task 24: Full verification gate

**Files:** none (verification only).

- [ ] **Step 1: Run the full orchestration test suite**

Run: `cargo test -p orchestration`
Expected: all green (LSP tests skip cleanly where servers/python are absent).

- [ ] **Step 2: Lint the whole workspace**

Run: `cargo clippy --workspace --all-targets`
Expected: zero warnings (workspace denies warnings). Fix any flagged dead code (notably the inert locals flagged in Tasks 3 and 13).

- [ ] **Step 3: Format check**

Run: `cargo fmt --check`
Expected: clean. If not, run `cargo fmt` and commit the formatting.

- [ ] **Step 4: Build the desktop crate (catches cross-crate breakage)**

Run: `cargo build -p desktop` (or the top-level `cargo build`)
Expected: builds — the new `BuiltinToolKind::Lsp` variant is handled everywhere it is matched.

- [ ] **Step 5: Final commit if fmt changed anything**

```bash
git add -A
git commit -m "chore(lsp): fmt and final verification"
```

---

## Deferred Follow-ups (out of scope for this plan)

Each is an isolated extension with a clear seam. Implement only if/when needed; none block the core parity above.

1. **`lspmux` multiplexing.** OMP optionally wraps supported servers (only `rust-analyzer` in `DEFAULT_SUPPORTED_SERVERS`) with `lspmux client` and detects it via `lspmux status` (cached 5min). Seam: `LspClient::start` builds the `Command` — add an `lspmux`-detection step there that rewrites `(command, args)` before spawn. Gate behind `PI_DISABLE_LSPMUX`.

2. **Custom CLI linter clients (Biome, SwiftLint).** OMP's `diagnostics` action also runs non-LSP linters (`BiomeClient`, `SwiftLintClient`) and merges results. Seam: `LspManager::diagnostics_for` iterates `clients_for(path)`; add a parallel list of "linter clients" keyed by extension that produce the same `format_diagnostic_line` strings.

3. **Directory-tree `rename_file`.** `manager.rename_file` currently rejects directories. OMP walks every regular file (cap `MAX_RENAME_PAIRS = 1000`), sends one `willRenameFiles` with all pairs, applies edits, renames, then `didRenameFiles`. Seam: replace the `source.is_dir()` rejection with an enumerator that builds the `files` array.

4. **Idle-client sweeper.** OMP reaps idle clients on a `60_000ms` interval. Seam: `LspManager` could hold a `tokio::time::interval` task that calls `shutdown` on clients whose `lastActivity` exceeds a threshold; requires adding `last_activity: Mutex<Instant>` to `LspClient` and bumping it in `send_request`.

5. **UTF-16 column correctness.** `edits.rs::offset_of` and `position.rs` treat `character` as Unicode scalar counts, not UTF-16 code units. For files with astral-plane characters before an edit column, offsets can be wrong. Seam: convert the prefix to UTF-16 code units when computing/consuming `character`.

6. **References context lines & retry.** OMP includes surrounding source context for the first `REFERENCE_CONTEXT_LIMIT` references and retries when only the declaration is returned. The current `references` renderer is location-only. Seam: enrich `LspManager::references` with file reads for context and a small retry loop.

7. **Diagnostics freshness versioning.** OMP captures a `diagnosticsVersion` and waits for a *newer* publish after `refreshFile`, avoiding stale reads. The current `wait_for_diagnostics` waits only for *presence*. Seam: add a per-URI publish counter in `LspClient` and wait for it to advance past the value captured before `refresh_file`.

# Codebase Concerns

**Analysis Date:** 2026-05-30

## Tech Debt

### Monolithic UI Files
- **Issue:** Several UI files exceed healthy length boundaries and mix rendering, layout math, event handling, and state mutation in single functions.
- **Files:**
  - `crates/agent-workflow-app/src/ui/canvas.rs` (1,407 lines)
  - `crates/agent-workflow-app/src/ui/mod.rs` (614 lines)
  - `crates/agent-workflow-app/src/ui/inspector.rs` (476 lines)
  - `crates/agent-workflow-app/src/ui/nav.rs` (459 lines)
- **Impact:** Refactoring is difficult; risk of introducing regressions when touching unrelated UI areas.
- **Fix approach:** Extract dedicated modules for grid rendering (`draw_dot_grid`, `draw_cubic_bezier`), chat compositor, run-trace list, and inspector form building.

### Suppressed Clippy Root Causes
- **Issue:** Functions are annotated with `#[allow(clippy::too_many_lines)]`, `#[allow(clippy::too_many_arguments)]`, and `#[allow(clippy::struct_excessive_bools)]` rather than being decomposed.
- **Files:**
  - `crates/agent-workflow-app/src/ui/mod.rs:237` — `update()` method (~350 lines)
  - `crates/agent-workflow-app/src/ui/inspector.rs:89` — `show_inspector_panel()`
  - `crates/agent-workflow-app/src/ui/canvas.rs:919` — `show_chat_composer()` (9 arguments)
  - `crates/agent-workflow-app/src/ui/canvas.rs:343` — `#[allow(clippy::struct_excessive_bools)]` on `BottomPanelOutput`
  - `crates/agent-workflow-app/src/execution.rs:183` — `run_workflow_headless()`
- **Impact:** Complexity is hidden behind allow attributes, making it harder for future agents to gauge churn risk.
- **Fix approach:** Decompose `update()` into per-frame subroutines; pass structs instead of positional arguments.

### Deprecated Egui API Surface
- **Issue:** Multiple UI modules suppress deprecation warnings with `#![allow(deprecated)]` because they rely on deprecated egui APIs.
- **Files:**
  - `crates/agent-workflow-app/src/ui/canvas.rs:1`
  - `crates/agent-workflow-app/src/ui/theme.rs:1`
  - `crates/agent-workflow-app/src/ui/nav.rs:1`
  - `crates/agent-workflow-app/src/ui/mod.rs:237`
- **Impact:** Upgrading egui will break the build as deprecated APIs are removed.
- **Fix approach:** Audit egui changelog for 0.34 → 0.35 migration and replace deprecated constructs (e.g., `CornerRadius::same` rename, `Frame` APIs).

## Known Bugs / Fragile Areas

### Runtime Panic on Startup
- **Issue:** `tokio::runtime::Runtime::new().expect("tokio runtime")` in `WorkflowApp::new()` will panic if the runtime cannot be created (e.g., thread exhaustion).
- **Files:** `crates/agent-workflow-app/src/ui/mod.rs:82`
- **Impact:** Hard crash on startup; no graceful fallback.
- **Fix approach:** Bubble `std::io::Error` from `WorkflowApp::new()` and surface it in a native error dialog before entering the egui loop.

### Silent Error Discarding (`let _ =`)
- **Issue:** Send errors, channel closures, and spawn aborts are silently ignored across async and UI boundaries.
- **Files:**
  - `crates/agent-workflow-app/src/execution.rs:102, 115, 117, 145, 147, 175, 199, 287, 305`
  - `crates/agent-workflow-app/src/ui/mod.rs:199, 233, 398`
  - `crates/agent-workflow-app/src/ui/canvas.rs:636`
- **Impact:** Background task failures vanish; user sees no error toast; run traces can end abruptly without explanation.
- **Fix approach:** Attach an `oneshot` error channel or log to `state.last_error` when `event_tx.send` fails.

### Validation `expect` Panic in Production Path
- **Issue:** `execution_layers()` contains a hard `expect` on an internal invariant. If deserialization ever bypasses validation, the runner panics.
- **Files:** `crates/workflow-core/src/validation.rs:104`
- **Impact:** Crash during workflow run instead of recoverable error.
- **Fix approach:** Replace `expect` with a fallible error variant and return `RunError::Validation`.

### `expect` in Runner on Node Lookup
- **Issue:** `WorkflowRunner` assumes every node ID in a layer exists in the validated map.
- **Files:** `crates/workflow-core/src/runner.rs:69`
- **Impact:** Panic if data races or manual mutation corrupt the workflow.
- **Fix approach:** Return `RunError::NodeFailed` with a clear message instead.

### Unconditional Unwrap in State Construction
- **Issue:** `AppState::from_workflow` calls `serde_json::to_string_pretty(...).unwrap()` while building the initial schema editor text.
- **Files:** `crates/agent-workflow-app/src/state.rs:80`
- **Impact:** Panic if a node's `output_schema` is somehow not serializable.
- **Fix approach:** Use `unwrap_or_default()` and set an empty editor state.

### Error Payload Leakage
- **Issue:** When the OpenAI-compatible API returns an HTTP error, the entire JSON payload is interpolated into the error string.
- **Files:** `crates/openai-client/src/lib.rs:267-270`
- **Impact:** If the upstream error response contains secrets (e.g., echoed headers or partial keys), they are exposed in logs/UI.
- **Fix approach:** Log payload to a debug channel but expose only status code and top-level message to users.

## Security Considerations

### API Key Stored as Plain String in Memory
- **Risk:** `ProviderConfigError` and `AppState` hold the API key as a plain `String`. No memory-zeroing on drop.
- **Files:**
  - `crates/agent-workflow-app/src/state.rs:54`
  - `crates/openai-client/src/lib.rs:14`
  - `crates/agent-workflow-app/src/provider_config.rs:15`
- **Current mitigation:** Settings persistence skips the key (correct). Env var is read at runtime.
- **Recommendations:** Use `secrecy::SecretString` or `zeroize`-aware wrapper for `api_key` fields.

### No Request Timeout on HTTP Client
- **Risk:** `reqwest::Client::new()` is created with default timeout (none). A hung network request blocks the async runner indefinitely.
- **Files:** `crates/openai-client/src/lib.rs:41`
- **Current mitigation:** None.
- **Recommendations:** Configure `.timeout(Duration::from_secs(120))` on the reqwest client and expose it as a setting.

### Settings File Only in User Data Dir
- **Risk:** `dirs::data_local_dir()` can fail (returns `None` in sandboxed/containerized environments), falling back to `"."` which writes to the working directory.
- **Files:**
  - `crates/agent-workflow-app/src/storage.rs:24-28`
  - `crates/agent-workflow-app/src/settings_store.rs:177-182`
- **Current mitigation:** `unwrap_or_else(|| PathBuf::from("."))`
- **Recommendations:** Use `dirs::config_dir()` as a secondary fallback and surface a warning when the fallback path is used.

## Performance Bottlenecks

### Per-Frame `resolve_provider_config` Evaluation
- **Problem:** `resolve_provider_config` is called on every `update()` tick to compute `api_key_ready`. This constructs `ProviderEnv`, matches providers, trims strings, and checks env vars repeatedly.
- **Files:** `crates/agent-workflow-app/src/ui/mod.rs:526-534`
- **Cause:** No caching of the resolved key status.
- **Improvement path:** Cache `ProviderEnv` and recompute only when `settings` or `state.provider_api_key_input` changes.

### Allocations During Canvas Render Loop
- **Problem:** Every frame clones the full node display list and chat message list.
- **Files:**
  - `crates/agent-workflow-app/src/ui/canvas.rs:93-109` — `node_display` `Vec` clone
  - `crates/agent-workflow-app/src/ui/canvas.rs:587` — `entries.clone()` for run trace
  - `crates/agent-workflow-app/src/ui/canvas.rs:674-689` — `messages.clone()` and `selected_node_label.clone()`
- **Cause:** Immediate-mode GUI wants snapshots to avoid borrow fights.
- **Improvement path:** Use `egui::Id` caches or pre-bake immutable label strings in `AppState` so the UI borrows rather than clones.

### Synchronous JSON Pretty-Print on Workflow Switch
- **Problem:** `serde_json::to_string_pretty` runs synchronously in `from_workflow`.
- **Files:** `crates/agent-workflow-app/src/state.rs:80`
- **Cause:** Schema text is built eagerly on every workflow switch.
- **Improvement path:** Defer pretty-printing until the inspector is actually opened, or cache the schema string in `AgentNodeConfig`.

## Fragile Areas

### Workflow Execution Termination on First Node Failure
- **Files:** `crates/workflow-core/src/runner.rs:111-120`
- **Why fragile:** The runner returns `Err` immediately when any node fails, aborting the entire workflow even if downstream nodes are independent.
- **Safe modification:** Introduce a `continue_on_error` flag or per-node `on_error` policy before changing the return path.
- **Test coverage:** Tests assert this exact early-exit behavior; changing it will require updating test expectations.

### `WorkflowApp::on_exit` Save Failure Ignored
- **Files:** `crates/agent-workflow-app/src/ui/mod.rs:231-235`
- **Why fragile:** `let _ = self.store.save(...)` silently drops write failures during shutdown. If the disk is full, user loses the last workflow edits.
- **Safe modification:** Return `Result` from `on_exit` if eframe supports it, or block with retry and panic only after logging.

### Settings Migration with Single Legacy Fallback
- **Files:** `crates/agent-workflow-app/src/settings_store.rs:186-203`
- **Why fragile:** Only one legacy schema (` LegacySettings`) is supported. Adding a new breaking field will require another manual migration branch.
- **Safe modification:** Introduce a version field in `AppSettings` and a small migration pipeline.

## Scaling Limits

### Chat Logs Unbounded Growth
- **Current capacity:** `BTreeMap<NodeId, Vec<ChatMessage>>` grows indefinitely per node.
- **Limit:** Memory exhaustion on long-running workflows with large model outputs.
- **Scaling path:** Add a cap (e.g., retain last 500 messages) and a "clear chat" action per node.

### Run Trace Unbounded Growth
- **Current capacity:** `Vec<RunTraceEntry>` accumulates events for an entire run.
- **Limit:** Very large workflows with many layers will produce traces that bloat UI rendering time.
- **Scaling path:** Implement trace truncation or a ring buffer after a configurable threshold.

## Dependencies at Risk

### Egui / Eframe Version Lock
- **Risk:** Pinned at 0.34.2 while upstream releases newer versions. Deprecation allowances signal drift.
- **Impact:** Security patches and performance improvements in egui are missed.
- **Migration plan:** Upgrade to latest 0.35+ and remove all `#[allow(deprecated)]` attributes.

### `uuid` v1 Feature Dependency
- **Risk:** `uuid = { version = "1.23.1", features = ["v4", "serde"] }` — v4 UUIDs require OS entropy source. In minimal/sandboxed environments this can fail.
- **Impact:** Workflow creation could panic if `/dev/urandom` is unavailable.
- **Migration plan:** Switch to `uuid::Uuid::new_v4()` error handling or use a deterministic ID scheme if randomness is unavailable.

## Missing Critical Features

### No Retry Logic for Transient AI Failures
- **Problem:** `OpenAiClient::post_json` returns `AgentError::Failed` on the first HTTP failure with no exponential backoff.
- **Blocks:** Resilience against rate limits (429) and brief network hiccups.
- **Files:** `crates/openai-client/src/lib.rs:252-274`

### No Cancellation Token for Headless Runs
- **Problem:** `run_workflow_headless` spawns a Tokio task via `tokio::spawn` but aborting requires dropping the runtime or sending a new action on a channel that the headless variant does not expose.
- **Blocks:** UI cannot cancel a long-running headless execution.
- **Files:** `crates/agent-workflow-app/src/execution.rs:69-88`, `183-205`

### Coarse-Grained Error Enum
- **Problem:** `AgentError` has only one variant (`Failed(String)`), making it impossible for callers to distinguish network errors, serialization errors, auth errors, and refusals programmatically.
- **Blocks:** Targeted retry, user-friendly error classification, and metrics.
- **Files:** `crates/workflow-core/src/ports.rs:24-28`

## Test Coverage Gaps

### live_workflow.rs Conditional Compilation
- **What's not tested:** End-to-end live AI invocations are skipped unless `STEP_WORKFLOW_LIVE_AI=1` is set in the environment.
- **Files:** `crates/agent-workflow-app/tests/live_workflow.rs`
- **Risk:** Regression in the real HTTP path goes unnoticed in CI.
- **Priority:** Medium

### Runner Early-Exit Tests Tightly Coupled
- **What's fragile:** Tests in `runner.rs` and `workflow_acceptance.rs` assert exact panic/unwrap behavior on node failure. Changing the runner to continue past failures will break these tests.
- **Files:** `crates/workflow-core/src/runner.rs:392`, `crates/agent-workflow-app/tests/workflow_acceptance.rs`
- **Risk:** Refactoring execution semantics requires rewriting many test assertions.
- **Priority:** Low (test debt, not production debt)

---

*Concerns audit: 2026-05-30*

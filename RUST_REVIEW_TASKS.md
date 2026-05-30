# Remaining Rust Best Practices Tasks

This document tracks the open items from the comprehensive codebase review. Items are ordered by impact: **CRITICAL → HIGH → MEDIUM → LOW**.

---

## CRITICAL

### 1. Convert `NodeId`/`EdgeId`/`WorkflowId` from type aliases to newtypes

- **Rule:** `type-newtype-ids`, `own-borrow-over-clone`
- **Location:** `crates/workflow-core/src/model.rs:5-7`
- **Issue:** `pub type NodeId = String;` provides zero type safety. A `WorkflowId` can be passed where a `NodeId` is expected, and the compiler allows it.
- **Impact:** Every DAG traversal causes heap allocations (`node.id.clone()`, `edge.to.clone()`, etc.).
- **Fix:**
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
  pub struct NodeId(pub String);
  #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
  pub struct EdgeId(pub String);
  #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
  pub struct WorkflowId(pub String);
  ```
- **Files affected:** `model.rs`, `validation.rs`, `runner.rs`, `interactive.rs`, `ports.rs`, `openai-client/src/lib.rs`, `agent-workflow-app/src/**/*.rs`.

---

### 2. Structure `AgentError` into variants instead of a single `Failed(String)`

- **Rule:** `err-custom-type`, `anti-stringly-typed`
- **Location:** `crates/workflow-core/src/ports.rs:24-28`
- **Issue:** One catch-all `Failed(String)` destroys structured error matching downstream. HTTP status codes, JSON parse errors, refusals, and missing fields are all flattened.
- **Impact:** Downstream code must parse error strings to branch; source chains are lost.
- **Fix:**
  ```rust
  #[derive(Debug, Error)]
  pub enum AgentError {
      #[error("HTTP {status}: {body}")]
      Http { status: u16, body: String },
      #[error("invalid JSON: {0}")]
      Json(#[from] serde_json::Error),
      #[error("refusal: {0}")]
      Refusal(String),
      #[error("missing field: {0}")]
      MissingField(&'static str),
  }
  ```
- **Files affected:** `ports.rs`, `openai-client/src/lib.rs`.

---

## HIGH

### 3. Replace `unbounded_channel` with a bounded channel

- **Rule:** `async-bounded-channel`
- **Location:** `crates/agent-workflow-app/src/execution.rs:82-83`
- **Issue:** `tokio::sync::mpsc::unbounded_channel()` grows memory without limit if the UI thread blocks.
- **Fix:**
  ```rust
  let (event_tx, event_rx) = tokio::sync::mpsc::channel(256);
  ```
  Producer side: `event_tx.send(...).await` (natural backpressure). Consumer side in UI remains `rx.try_recv()`.
- **Files affected:** `execution.rs`.

---

### 4. Replace `Result<(), String>` in internal API with a structured error

- **Rule:** `err-result-over-panic`, `type-result-fallible`
- **Location:** `crates/workflow-core/src/interactive.rs` (`on_human_input`)
- **Issue:** `Result<(), String>` prevents `.context()` enrichment and makes matching fragile.
- **Fix:**
  ```rust
  #[derive(Debug, Error)]
  pub enum InputError {
      #[error("no node awaiting input")]
      NotAwaiting,
      #[error("expected input for {expected}, got {actual}")]
      WrongNode { expected: NodeId, actual: NodeId },
  }
  ```
- **Files affected:** `interactive.rs` and all callers in `agent-workflow-app`.

---

### 5. Store `tokio::runtime::Handle` instead of `Runtime` in app struct

- **Rule:** `async-tokio-runtime`
- **Location:** `crates/agent-workflow-app/src/ui/mod.rs:49`
- **Issue:** Storing the full `Runtime` ties the app struct to a concrete instance and prevents cheap cloning.
- **Fix:** Store `tokio::runtime::Handle` (which is `Clone` + `Send`). Spawn tasks via `self.runtime_handle.spawn(...)` or `tokio::spawn`.
- **Files affected:** `ui/mod.rs`.

---

## MEDIUM

### 6. Add `#![warn(missing_docs)]` and document all public items

- **Rule:** `doc-all-public`, `doc-module-inner`, `lint-missing-docs`
- **Location:** All crates
- **Issue:** `openai-client/src/lib.rs` has **zero** `///` or `//!` docs. `model.rs`, `ports.rs`, `settings_store.rs` are similarly undocumented.
- **Fix:**
  1. Add to root manifests or lib roots:
     ```rust
     #![warn(missing_docs)]
     ```
  2. Add `///` docs to every `pub` struct, enum, trait, and fn.
  3. Add `//!` crate-level docs in each `lib.rs`.
- **Priority crates:** `openai-client`, `workflow-core/model.rs`, `workflow-core/ports.rs`, `agent-workflow-app/src/*.rs`.

---

### 7. Extract integration tests to `tests/` directories

- **Rule:** `test-integration-dir`
- **Location:** `crates/workflow-core`, `crates/openai-client`
- **Issue:** All tests are inline unit tests (`#[cfg(test)] mod tests`). No `tests/` directory integration tests verify the public API boundary.
- **Fix:** Move wiremock HTTP tests from `openai-client/src/lib.rs` to `crates/openai-client/tests/responses_roundtrip.rs`. Create `crates/workflow-core/tests/validation_integration.rs`.
- **Files affected:** `workflow-core`, `openai-client`.

---

### 8. Add `#[non_exhaustive]` to `workflow-core` enums

- **Rule:** `api-non-exhaustive`
- **Location:** `crates/workflow-core/src/model.rs`
- **Issue:** Adding a variant to `NodeKind`, `RunEventKind`, or `ChatRole` is a breaking change for downstream `match` statements.
- **Fix:**
  ```rust
  #[non_exhaustive]
  pub enum NodeKind { Agent }
  ```
- **Files affected:** `model.rs`.

---

### 9. Document `# Panics` on `runner.rs` methods

- **Rule:** `doc-panics-section`
- **Location:** `crates/workflow-core/src/runner.rs`
- **Issue:** `run` and `run_with_entrypoint` contain `.expect("layer contains validated node id")` but the panics are not documented.
- **Fix:** Add `# Panics` doc sections mirroring the existing docs in `validation.rs`.
- **Files affected:** `runner.rs`.

---

### 10. Replace `format!` in `build_user_content` with `write!` (or document)

- **Rule:** `mem-avoid-format`, `anti-format-hot-path`
- **Location:** `crates/openai-client/src/lib.rs`
- **Issue:** `format!` allocates a new `String` on every AI invocation. While acceptable for a desktop app, it sets a poor example.
- **Fix:**
  ```rust
  let mut buf = String::with_capacity(estimated_len);
  write!(
      buf,
      "Node: {}\nTask:\n{}\n\nUpstream input JSON:\n{}",
      request.node_label, request.task_prompt, request.input
  ).unwrap();
  buf
  ```
- **Files affected:** `openai-client/src/lib.rs`.

### 11. Pre-size vectors in aggregation loops

- **Rule:** `mem-with-capacity`
- **Location:** `crates/workflow-core/src/runner.rs` and `interactive.rs`
- **Issue:** `Vec::new()` is used when the final size is known ahead of time.
- **Fix:**
  ```rust
  let mut events = Vec::with_capacity(workflow.nodes.len());
  ```
- **Files affected:** `runner.rs`, `interactive.rs`.

---

## LOW

### 12. Fix placeholder repository URL in workspace root

- **Location:** `Cargo.toml:14`
- **Issue:** `repository = "https://example.invalid/..."` may trigger warnings on `cargo publish`.
- **Fix:** Replace with real repo URL or remove key.

---

### 13. Normalize `async-trait` dependency classification

- **Location:** `Cargo.toml` workspace deps, `agent-workflow-app/Cargo.toml`
- **Issue:** `async-trait` is a normal dep in `workflow-core` and `openai-client`, but a **dev-dependency** in `agent-workflow-app`. Verify whether it is truly test-only or needed at runtime.
- **Fix:** Move to `[dependencies]` if used in non-test code.

---

### 14. Centralize `egui-phosphor` version in workspace deps

- **Location:** `crates/agent-workflow-app/Cargo.toml`
- **Issue:** `egui-phosphor = "0.12.0"` is pinned directly instead of in `[workspace.dependencies]`.
- **Fix:** Add to root `[workspace.dependencies]` and use `{ workspace = true }` in the app crate.

---

### 15. Use `version.workspace = true` in crate manifests

- **Location:** All `crates/*/Cargo.toml`
- **Issue:** Each crate hardcodes `version = "0.1.0"` rather than inheriting from workspace.
- **Fix:** Add `version = "0.1.0"` to `[workspace.package]` and switch each crate to `version.workspace = true`.

---

## REFERENCE (Anti-patterns already checked — no action needed)

- ✅ `anti-unwrap-abuse` — Safe (`unwrap` only in tests)
- ✅ `anti-expect-lazy` — `expect` only for programmer invariants
- ✅ `anti-lock-across-await` — No locks held over await points
- ✅ `anti-index-over-iter` — Iterators preferred
- ✅ `anti-panic-expected` — No panic on recoverable I/O errors
- ✅ `anti-empty-catch` — No empty error arms
- ✅ `anti-over-abstraction` — No trait tower of generics
- ✅ `anti-premature-optimize` — Performance appropriate for scope
- ✅ `anti-type-erasure` — `AiPort` trait object only at boundary
- ✅ `anti-collect-intermediate` — No unnecessary intermediate collects

---

## Resolved ✅

| # | Task | Commit / Result |
|---|---|---|
| 1 | Add `[profile.release]` to root `Cargo.toml` | Added `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true` |
| 2 | Fix pre-existing `cargo fmt` drift | Ran `cargo fmt --all` |
| 3 | Fix single Clippy lint (`unnecessary_mut`) | Removed `mut` from `&mut self.settings` in `ui/mod.rs:554` |

---

*Last updated: after Issue #1 review pass*

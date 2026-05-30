# Coding Conventions

**Analysis Date:** 2026-05-30

## Project Context

This is a Rust workspace (edition 2021) with three crates: `workflow-core`, `openai-client`, `agent-workflow-app`. Shared standards are enforced via workspace-level lints in `Cargo.toml`.

---

## Naming Patterns

**Files:**
- Modules use `snake_case.rs` (e.g. `settings_store.rs`, `provider_config.rs`)
- Test files use `snake_case.rs` (e.g. `workflow_acceptance.rs`, `live_workflow.rs`)

**Functions:**
- Use `snake_case` for all functions and methods
- Boolean/predicate helpers use `is_` or `has_` prefixes where appropriate
- Constructors are `new()` or `default()` via `Default` impl
- Setters returning `Self` are rare; prefer direct mutation

**Variables:**
- Use `snake_case` for bindings
- Avoid single-letter names except in loops and math

**Types:**
- Structs and enums use `PascalCase`
- Newtype ID wrappers use `PascalCase` and end in `Id` (e.g. `NodeId`, `EdgeId`, `WorkflowId`)
- Error enums use `PascalCase` and end in `Error` (e.g. `WorkflowValidationError`, `ProviderConfigError`)
- Constants use `SCREAMING_SNAKE_CASE` (e.g. `NODE_WIDTH`, `INSPECTOR_GAP`, `TS_LABEL`)

**Traits:**
- `PascalCase`, nouns or adjective-noun pairs (e.g. `AiPort`)

---

## Code Style

**Formatting:**
- Tool: `rustfmt` (default config, no `rustfmt.toml` present)
- CI enforces: `cargo fmt --all --check`
- Line length is implicitly ~100 characters (rustfmt default)

**Linting:**
- Baseline CI: `cargo clippy --workspace --all-targets`
- Max strictness: `cargo clippy-max` alias defined in `.cargo/config.toml`:
  ```toml
  clippy-max = "clippy --workspace --all-targets -- -D warnings -D clippy::pedantic -D clippy::nursery -D clippy::cargo"
  ```
- Workspace lints in root `Cargo.toml`:
  ```toml
  [workspace.lints.rust]
  warnings = "deny"
  unsafe_code = "forbid"
  [workspace.lints.clippy]
  all = "deny"
  ```
- Each crate inherits workspace lints via `[lints] workspace = true`

**Suppression Patterns:**
- `#[allow(clippy::float_cmp)]` in tests asserting constant values (e.g. `assert_eq!(COLLAPSED_NAV_ICON_SIZE, 34.0)`)
- `#[allow(clippy::too_many_lines)]` on large UI render/update functions (`agent-workflow-app/src/ui/mod.rs:237`, `execution.rs:183`)
- `#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]` for index-to-float conversions
- `#[allow(clippy::wildcard_imports)]` in UI modules importing theme tokens (`ui/mod.rs:17`, `ui/inspector.rs:1`, `ui/canvas.rs:1`)
- `#[allow(clippy::struct_excessive_bools)]` on UI structs with many boolean fields
- `#[allow(clippy::needless_pass_by_value)]` on event handler signatures in `canvas.rs`
- `#[allow(clippy::while_float)]` in canvas math
- `#[allow(deprecated)]` at top of `theme.rs` and `nav.rs` for `egui::CornerRadius` usage

---

## Import Organization

**Order:**
1. `use crate::...` (internal modules)
2. `use std::...` (standard library)
3. `use <third_party>` (external crates)

**Path Aliases:**
- No custom path aliases via `[dependencies]` rename
- Use direct crate names: `workflow_core`, `openai_client`, `agent_workflow_app`

**Wildcard Imports:**
- Allowed in UI modules for theme tokens: `use theme::*;`
- Disallowed in domain logic; prefer explicit imports

---

## Error Handling

**Strategy:** Map external/system errors into local domain language at crate boundaries.

**Patterns:**
- Use `thiserror` derive macros for error enums:
  ```rust
  #[derive(Debug, Clone, Error, PartialEq, Eq)]
  pub enum WorkflowValidationError {
      #[error("workflow must contain at least one node")]
      EmptyWorkflow,
      #[error("node {node_id} failed: {message}")]
      NodeFailed { node_id: NodeId, message: String },
  }
  ```
- Return `Result<_, _>` from operations that can fail; avoid panics outside tests
- Use `expect(...)` only for invariants guaranteed by validated flow:
  ```rust
  let count = incoming
      .get_mut(child_id)
      .expect("child id was validated before layer build");
  ```
- `#[from]` for automatic error composition:
  ```rust
  #[derive(Debug, Clone, Error)]
  pub enum RunError {
      #[error(transparent)]
      Validation(#[from] WorkflowValidationError),
  }
  ```
- IO errors in storage modules map to `io::Error` with custom messages:
  ```rust
  io::Error::new(
      io::ErrorKind::InvalidData,
      format!("workflow store JSON invalid: {error}"),
  )
  ```

---

## Constants

**Placement:** At top of file, named by intent.

**Examples:**
- `crates/agent-workflow-app/src/canvas_math.rs`: `NODE_WIDTH`, `NODE_HEIGHT`
- `crates/agent-workflow-app/src/ui/theme.rs`: `TS_LABEL`, `SURFACE_0`
- `crates/agent-workflow-app/src/ui/mod.rs`: `COLLAPSED_NAV_ICON_SIZE`
- `crates/agent-workflow-app/src/ui/nav.rs`: `NAV_ROW_HEIGHT`, `NAV_PILL_INSET_X`

---

## Visibility

**Patterns:**
- Default to `pub(super)` in UI modules to keep surface area narrow
- `pub(crate)` for intra-crate sharing (e.g. `build_node_input` in `runner.rs`)
- `pub` only for cross-crate API surface
- `pub use` re-exports in `lib.rs` for ergonomic access

---

## Documentation

**Required doc comments:**
- `/// # Errors` block on fallible functions explaining error conditions
- `/// # Panics` block on functions with potential panics
- Example:
  ```rust
  /// # Errors
  /// Returns an error if the workflow is invalid.
  pub fn validate_workflow(workflow: &Workflow) -> Result<(), WorkflowValidationError> { ... }
  ```

**Inline comments:**
- Use `// ── Section Name ──` style for UI update function sections
- Brief inline comments for algorithm steps

---

## Function Design

**Size:**
- Large UI update functions accepted with `#[allow(clippy::too_many_lines)]`
- Domain functions are short and focused (e.g. `upstream_map`, `build_node_input`)

**Parameters:**
- Accept generic `impl Into<String>` or `impl Into<PathBuf>` for constructors
- Use `&str` for lookup keys after ID validation

**Return Values:**
- `#[must_use]` on constructors and pure functions (e.g. `WorkflowApp::new`, `AppState::new`, `clamp_node_position`)
- `const fn` where possible for simple constructors

---

## Module Design

**Exports:**
- `lib.rs` uses `pub mod` for public modules and `pub use` for re-exports
- Barrels not used; explicit re-exports in `lib.rs`

**Internal modules:**
- `mod` without `pub` for UI submodules (`canvas`, `inspector`, `nav`, `settings`, `theme`, `widgets`)
- `pub mod` for crate public API (`execution`, `state`, `storage`, etc.)

---

## Async Patterns

- Use `async-trait` crate for trait async methods (e.g. `AiPort::invoke`)
- Tokio runtime created in `WorkflowApp::new` for desktop app
- `#[tokio::test]` for async test functions
- `futures::future::join_all` for parallel layer execution in runner

---

## Unsafe Code

- **Forbidden** at workspace level via `unsafe_code = "forbid"`
- No unsafe blocks anywhere in the codebase

---

## Dependency Rules

- Add workspace deps in root `Cargo.toml` first, then consume in crate manifests via `.workspace = true`
- Keep crate dependencies minimal and role-specific:
  - `workflow-core`: model/validation/runner only (`serde`, `uuid`, `thiserror`, `async-trait`, `futures`)
  - `openai-client`: HTTP + payload parsing (`reqwest`)
  - `agent-workflow-app`: UI/runtime/persistence (`eframe`, `egui`, `egui-phosphor`, `tokio`)

---

*Convention analysis: 2026-05-30*

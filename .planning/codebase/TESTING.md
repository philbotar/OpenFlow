# Testing Patterns

**Analysis Date:** 2026-05-30

## Test Framework

**Runner:** Built-in Rust test harness (`cargo test`)
- No external test runner (no `nextest`, `cargo-testify`, etc.)
- Uses `tokio` async runtime for async tests via `#[tokio::test]`

**Assertion Library:** Built-in `assert!`, `assert_eq!`, `assert_ne!`
- No external assertion crate (no `pretty_assertions`, `claim`, etc.)

**Run Commands:**
```bash
# Run all tests across workspace
cargo test --workspace

# Run unit tests only (including inline tests)
cargo test -p workflow-core
cargo test -p openai-client
cargo test -p agent-workflow-app

# Run integration / acceptance tests
cargo test -p agent-workflow-app --test workflow_acceptance -- --nocapture
cargo test -p agent-workflow-app --test live_workflow -- --ignored --nocapture

# Watch mode (not configured; use manual re-runs or entr/fd)
```

---

## Test File Organization

**Location:** Co-located `#[cfg(test)] mod tests` in source files + separate `tests/` directories for integration tests.

**Naming:**
- Inline test modules: `mod tests` inside same file as behavior
- Integration tests: `tests/workflow_acceptance.rs`, `tests/live_workflow.rs`
- Test functions use `snake_case` and descriptive names

**Structure by crate:**

| Crate | Inline Tests | Integration Tests |
|-------|-------------|-------------------|
| `workflow-core` | `model.rs`, `validation.rs`, `runner.rs`, `interactive.rs` | None |
| `openai-client` | `lib.rs` | None |
| `agent-workflow-app` | `state.rs`, `storage.rs`, `settings_store.rs`, `provider_config.rs`, `canvas_math.rs`, `ui/theme.rs`, `ui/mod.rs`, `ui/canvas.rs`, `ui/inspector.rs`, `ui/nav.rs` | `tests/workflow_acceptance.rs`, `tests/live_workflow.rs` |

---

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // additional imports as needed

    #[test]
    fn descriptive_test_name_says_what_it_proves() {
        // arrange
        let state = AppState::new();
        // act
        let result = state.some_operation();
        // assert
        assert_eq!(result, expected_value);
    }

    #[tokio::test]
    async fn async_behavior_completes_correctly() {
        let runner = WorkflowRunner::new(RecordingAi::default());
        let report = runner.run(&workflow).await.unwrap();
        assert_eq!(report.outputs.len(), 2);
    }
}
```

**Setup pattern:**
- Helper functions for test data creation, NOT fixtures libraries:
  ```rust
  fn node(id: &str) -> Node {
      let mut node = Node::agent(id, 0.0, 0.0);
      node.id = NodeId(id.to_string());
      node
  }

  fn agent(id: &str, label: &str) -> Node {
      let mut node = Node::agent(label, 0.0, 0.0);
      node.id = NodeId(id.to_string());
      node
  }
  ```
- Helper functions for parameterized workflows exist in `validation.rs` (`workflow_with_nodes`) and acceptance tests (`branch_join_workflow`)

**Teardown pattern:**
- No explicit teardown; tests are stateless
- Filesystem tests use `tempfile::tempdir` for isolation

---

## Mocking

**Framework:** Manual test doubles (no mocking framework)

**Patterns:**
```rust
// Recording stub for asserting request shape
#[derive(Default)]
struct RecordingAi {
    requests: Arc<Mutex<Vec<AgentRequest>>>,
}

#[async_trait]
impl AiPort for RecordingAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(AgentResponse {
            output: json!({"summary": "ok"}),
            raw_text: "...".to_string(),
        })
    }
}

// Failing stub for error path testing
struct FailingAi;

#[async_trait]
impl AiPort for FailingAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        Err(AgentError::Failed(format!("synthetic failure for {}", request.node_id)))
    }
}
```

**What to Mock:**
- AI port implementations (`AiPort`) to control execution without real HTTP calls
- HTTP responses using `wiremock` in `openai-client` tests

**What NOT to Mock:**
- Internal domain types (`Workflow`, `Node`, `Edge`)
- Serialization/deserialization (test with real serde round-trips)
- File I/O (use `tempfile::tempdir` for real FS isolation)

---

## Fixtures and Factories

**Test Data Location:**
- Inline helper functions in `#[cfg(test)] mod tests`
- No external fixture files or factory crates

**HTTP Response Fixtures:**
- Inline JSON literals in `wiremock` tests:
  ```rust
  Mock::given(method("POST"))
      .and(path("/v1/responses"))
      .respond_with(ResponseTemplate::new(200).set_body_json(json!({
          "output": [{
              "type": "message",
              "content": [{
                  "type": "output_text",
                  "text": "{\"summary\":\"done\"}"
              }]
          }]
      })))
      .mount(&server)
      .await;
  ```

**Filesystem isolation:**
```rust
use tempfile::tempdir;

#[test]
fn saves_and_loads_workflows() {
    let dir = tempdir().unwrap();
    let store = FileWorkflowStore::new(dir.path().join("nested").join("workflows.json"));
    // ...
}
```

---

## Coverage

**Requirements:** No explicit coverage target enforced
- Total Rust LOC: ~7,500 across workspace
- Test LOC: ~2,500 (inline + integration tests)
- Estimated coverage: high for domain logic; UI tests focus on token contracts

**Coverage gaps:**
- No automated coverage tool (no `tarpaulin`, `grcov` configured)
- Manual verification runs: acceptance and live tests are opt-in

---

## Test Types

**Unit Tests:**
- Scope: Single function or small module behavior
- Examples:
  - DAG layer ordering (`validation.rs`)
  - Input shape construction (`runner.rs`)
  - State mutations (`state.rs`)
  - Layout math (`inspector.rs`, `canvas.rs`, `canvas_math.rs`)

**Integration Tests:**
- Location: `crates/agent-workflow-app/tests/`
- Files: `workflow_acceptance.rs` (202 lines), `live_workflow.rs` (209 lines)

**Deterministic Acceptance Tests (`workflow_acceptance.rs`):**
- Sends a branch/join workflow through headless execution
- Proves:
  1. Root nodes receive `entrypoint.text`
  2. Downstream nodes receive upstream outputs in deterministic order
  3. Branch/join workflows complete with all expected node outputs
  4. Manual nodes pause, receive scripted human input, pass it downstream
  5. Run trace entries expose queued, running, paused, completed, failed states
  6. Chat logs capture system, thinking, user, assistant messages

**Live AI Smoke Tests (`live_workflow.rs`):**
- Marked with `#[ignore]` requiring env vars
- Avoid exact prose assertions; assert contracts:
  - Run completes
  - Every expected node has output
  - Output is valid JSON satisfying schema
  - Required fields non-empty
  - Sentinel value (`ORCHID-91`) preserved across nodes

**UI Layout Contract Tests:**
- Purpose: prevent token drift in spacing/sizing constants
- Examples:
  ```rust
  #[test]
  fn floating_width_uses_desktop_width_when_space_allows() {
      assert_eq!(floating_inspector_width(1280.0), 340.0);
  }
  ```
- Found in: `ui/theme.rs`, `ui/inspector.rs`, `ui/mod.rs`, `ui/canvas.rs`

---

## Async Testing

**Pattern:**
```rust
#[tokio::test]
async fn waits_for_upstream_outputs_before_downstream_calls() {
    let mut workflow = Workflow::new("runner");
    // ...
    let report = runner.run(&workflow).await.unwrap();
    assert_eq!(report.outputs.len(), 2);
}
```

- `tokio` runtime provided by `#[tokio::test]` macro
- No manual runtime setup in tests
- `dev-dependencies` includes `tokio.workspace = true` in each crate manifest

---

## Error Testing

**Pattern:**
```rust
#[tokio::test]
async fn rejects_invalid_workflow_before_openai_call() {
    let workflow = Workflow::new("empty");
    let runner = WorkflowRunner::new(RecordingAi::default());
    let error = runner.run(&workflow).await.unwrap_err();
    assert!(matches!(
        error,
        RunError::Validation(WorkflowValidationError::EmptyWorkflow)
    ));
}

#[test]
fn missing_key_reports_selected_provider_and_env_var() {
    let error = resolve_provider_config(...).unwrap_err();
    assert_eq!(
        error,
        ProviderConfigError::MissingApiKey { provider: "...", env_var: "..." }
    );
}
```

**Key patterns:**
- Use `unwrap_err()` and match/partial_eq against expected error variants
- For HTTP errors, assert message contains key substrings rather than exact strings

---

## CI Configuration

**GitHub Actions** (`.github/workflows/ci.yml`):
- Blocking job: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`
- Non-blocking job: `cargo clippy-max` (pedantic + nursery + cargo lints)

**Verification Commands (pre-commit):**
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo clippy-max
cargo test --workspace
```

---

## Test Philosophy Docs

**Reference documents:**
- `agent-reference-docs/testing-workflows.md` — acceptance and live-AI rules
- `agent-reference-docs/coding-patterns.md` — test strategy (test externally visible behavior, not private implementation)

---

*Testing analysis: 2026-05-30*

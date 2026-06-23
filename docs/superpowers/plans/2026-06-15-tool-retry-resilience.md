# Tool Retry & Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retry transient tool-runner failures per workflow `retry_policy` before surfacing `is_error` results to the model, and guarantee failed tools resume the agent loop (`CallAi`) without aborting the run.

**Architecture:** T19 (`ToolError::is_retryable`) and the `ToolHooks` seam around `ToolRunner::execute` are **already done**. Add a small `tool/retry.rs` helper in orchestration that wraps single-tool execution with exponential backoff (reusing `engine::RetryPolicy`). Wire it from `tool_port.rs` (`execute_tool_or_cancel` and the parallel-tool spawn path) — this is the real execution host; `drive.rs` only calls `engine.run()`, which delegates to `ToolPort`. For T21, harden `InteractiveEngine::on_tool_results` to synthesize error results for any missing tool calls in a batch (cancel/interrupt/partial batches), and add headless + unit tests proving a permanent tool failure still completes the node.

**Tech Stack:** Rust (`engine`, `orchestration`), `tokio` backoff + `CancellationToken`, `./scripts/verify.sh`

**Prerequisites (done — no tasks):**
- `crates/orchestration/src/tool/errors.rs` — `ToolError::is_retryable()`
- `crates/orchestration/src/tool/hooks.rs` — before/after hook seam on `ToolRunner::execute`
- `crates/engine/src/graph/workflow.rs` — `RetryPolicy` (shared with AI retry; default 3 attempts, 1000ms backoff)

**Related docs:** [ROADMAP.md — Tool invocation retry and resilience](../ROADMAP.md#tool-invocation-retry-and-resilience), [Domain hardening T20–T21](../ROADMAP.md#phase-2--functional-gaps), D5 (retry then feed error to model; never abort run for one tool call)

---

## Current gaps

| Layer | Today | After this plan |
| --- | --- | --- |
| `tool_port.rs` | One `ToolRunner::execute` attempt; `Err` → `denied()` `is_error` result | Retry retryable errors per `workflow.settings.retry_policy` with backoff |
| `tool_port.rs` | Cancel/interrupt mid-batch returns partial `Vec<ToolResult>` | Engine fills missing calls with error results; run continues |
| `interactive_engine/mod.rs` | `on_tool_results` error → `EngineRunResult::Failed` → drive emits `Error` and exits | Partial batches tolerated; one `is_error` result per pending call |
| Telemetry | No visibility into in-flight tool retries | `RunTelemetry::ToolRetrying` projected to trace |
| Tests | Engine unit test for denied tools; no orchestration retry/resilience tests | Unit tests for retry helper + headless acceptance for permanent tool failure |

**Out of scope:** Tool-specific retry overrides on `ToolRef`, UI retry badges, new hook implementations, MCP tools.

---

## File map

| File | Responsibility |
| --- | --- |
| `crates/orchestration/src/tool/retry.rs` | `execute_with_retry` — backoff loop, cancel-aware sleep, retryability gate |
| `crates/orchestration/src/tool/mod.rs` | `pub mod retry;` |
| `crates/orchestration/src/tool/errors.rs` | `ToolRunnerError::is_retryable()` delegating to `ToolError` |
| `crates/orchestration/src/run/execution/tool_port.rs` | Call `execute_with_retry`; emit `ToolRetrying`; pass policy from `self.workflow` |
| `crates/engine/src/execution/telemetry.rs` | Add `ToolRetrying` variant |
| `crates/engine/src/execution/interactive_engine/tools.rs` | Fill missing batch results in `on_tool_results` |
| `crates/orchestration/src/run/execution/events.rs` | Reducer for `ToolRetrying` (trace row) |
| `crates/orchestration/src/run/execution/tests.rs` | Headless permanent-tool-failure + retry unit coverage |
| `crates/orchestration/tests/workflow_acceptance.rs` | Acceptance: failed `read` → model recovers → node completes |
| `docs/ROADMAP.md`, `CHANGELOG.md` | Mark T20–T21 done under item #4 |

---

### Task 1: `ToolRunnerError::is_retryable`

**Files:**
- Modify: `crates/orchestration/src/tool/errors.rs` (append to existing `ToolRunnerError` impl block in `runner.rs` — prefer new `impl ToolRunnerError` in `errors.rs` or `runner.rs` bottom)
- Test: `crates/orchestration/src/tool/errors.rs` (`#[cfg(test)]` block)

- [ ] **Step 1: Write the failing test**

Add to `crates/orchestration/src/tool/errors.rs` `mod tests`:

```rust
use super::*;
use crate::tool::runner::ToolRunnerError;

#[test]
fn runner_registry_error_is_not_retryable() {
    let err = ToolRunnerError::Registry(ToolRegistryError::Missing("nope".into()));
    assert!(!err.is_retryable());
}

#[test]
fn runner_tool_timeout_is_retryable() {
    let err = ToolRunnerError::Tool(ToolError::Timeout {
        tool: "bash".into(),
        after_secs: 1,
        hint: "retry".into(),
        partial_output: None,
    });
    assert!(err.is_retryable());
}

#[test]
fn runner_invalid_arguments_is_not_retryable() {
    let err = ToolRunnerError::InvalidArguments("bad json".into());
    assert!(!err.is_retryable());
}
```

Add `use crate::tool::registry::ToolRegistryError;` at top of test module.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration runner_tool_timeout_is_retryable -- --nocapture`

Expected: FAIL — `is_retryable` not found on `ToolRunnerError`

- [ ] **Step 3: Implement `is_retryable`**

Create `crates/orchestration/src/tool/runner_error.rs` **or** add to `runner.rs` after `ToolRunnerError` enum:

```rust
impl ToolRunnerError {
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Tool(error) => error.is_retryable(),
            Self::Registry(_) | Self::InvalidArguments(_) | Self::BlockingTask(_) => false,
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p orchestration is_retryable -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/tool/runner.rs crates/orchestration/src/tool/errors.rs
git commit -m "feat(orchestration): classify ToolRunnerError retryability (T19 wiring)"
```

---

### Task 2: `execute_with_retry` helper

**Files:**
- Create: `crates/orchestration/src/tool/retry.rs`
- Modify: `crates/orchestration/src/tool/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/orchestration/src/tool/retry.rs`:

```rust
use crate::tool::errors::ToolError;
use crate::tool::runner::ToolRunnerError;
use engine::RetryPolicy;
use std::future::Future;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

/// Execute `run_attempt` up to `policy.max_attempts` retries after the first failure.
/// Only retries when the error is retryable and the cancel token is not set.
pub async fn execute_with_retry<T, F, Fut>(
    policy: &RetryPolicy,
    cancel: &CancellationToken,
    mut on_retry: impl FnMut(u8, std::time::Duration),
    mut run_attempt: F,
) -> Result<T, ToolRunnerError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ToolRunnerError>>,
{
    let mut retry_count: u8 = 0;
    loop {
        if cancel.is_cancelled() {
            return Err(ToolRunnerError::Tool(ToolError::Cancelled {
                tool: "tool".to_string(),
            }));
        }
        match run_attempt().await {
            Ok(value) => return Ok(value),
            Err(error) if error.is_retryable() && retry_count < policy.max_attempts => {
                retry_count += 1;
                let delay = policy.delay_for_attempt(retry_count);
                on_retry(retry_count, delay);
                tokio::select! {
                    biased;
                    () = cancel.cancelled() => {
                        return Err(ToolRunnerError::Tool(ToolError::Cancelled {
                            tool: "tool".to_string(),
                        }));
                    }
                    () = tokio::time::sleep(delay) => {}
                }
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn retries_transient_errors_until_success() {
        let policy = RetryPolicy {
            max_attempts: 2,
            backoff_ms: 1,
        };
        let attempts = Arc::new(AtomicU8::new(0));
        let attempts_cloned = Arc::clone(&attempts);
        let cancel = CancellationToken::new();
        let mut retry_events = 0u8;

        let result = execute_with_retry(
            &policy,
            &cancel,
            |_, _| retry_events += 1,
            || {
                let attempts = Arc::clone(&attempts_cloned);
                async move {
                    let n = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if n < 3 {
                        Err(ToolRunnerError::Tool(ToolError::transient("connection reset")))
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await
        .expect("should succeed on third attempt");

        assert_eq!(result, 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(retry_events, 2);
    }

    #[tokio::test]
    async fn permanent_error_does_not_retry() {
        let policy = RetryPolicy::default();
        let attempts = Arc::new(AtomicU8::new(0));
        let cancel = CancellationToken::new();

        let error = execute_with_retry(
            &policy,
            &cancel,
            |_, _| {},
            || {
                let attempts = Arc::clone(&attempts);
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(ToolRunnerError::Tool(ToolError::NotFound {
                        what: "missing".into(),
                        hint: "use find".into(),
                    }))
                }
            },
        )
        .await
        .expect_err("permanent");

        assert!(error.to_string().contains("[not_found]"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
```

Add to `crates/orchestration/src/tool/mod.rs`:

```rust
pub mod retry;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration retries_transient_errors_until_success -- --nocapture`

Expected: FAIL — module not found or compile error until `mod.rs` export added

- [ ] **Step 3: Fix any compile issues and ensure helper is complete**

Remove unused `Pin` import if the compiler warns. Keep `on_retry` callback — `tool_port` uses it to emit telemetry.

- [ ] **Step 4: Run tests**

Run: `cargo test -p orchestration retries_transient permanent_error_does_not_retry -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/tool/retry.rs crates/orchestration/src/tool/mod.rs
git commit -m "feat(orchestration): add execute_with_retry helper for tool backoff (T20)"
```

---

### Task 3: Wire retry into `execute_tool_or_cancel`

**Files:**
- Modify: `crates/orchestration/src/run/execution/tool_port.rs:467-538`

- [ ] **Step 1: Confirm Task 2 unit tests pass**

Run: `cargo test -p orchestration retries_transient_errors_until_success -- --nocapture`

Expected: PASS — retry helper is the unit-tested core; wiring is verified by compile + Task 7–8 integration tests.

- [ ] **Step 2: Implement wiring in `execute_tool_or_cancel`**

At top of `tool_port.rs`, add:

```rust
use crate::tool::retry::execute_with_retry;
use engine::RetryPolicy;
```

Replace the `tool_runner.execute(tool_call, Some(ctx))` arm inside `execute_tool_or_cancel` with:

```rust
let policy = self.workflow.settings.retry_policy.clone();
let tool_runner = Arc::clone(&tool_runner);
let cancel_for_retry = self.cancel_token.clone();
let event_tx = self.event_tx.clone();
let retry_node_id = node_id.clone();
let retry_tool_call_id = tool_call.id.clone();
let retry_tool_name = tool_call.name.clone();
let ctx_for_attempt = ctx.clone();
let call_for_attempt = tool_call.clone();

let run = execute_with_retry(
    &policy,
    &cancel_for_retry,
    |attempt, delay| {
        let _ = event_tx.send(ExecutionEvent::ToolRetrying {
            node_id: retry_node_id.clone(),
            tool_call_id: retry_tool_call_id.clone(),
            tool_name: retry_tool_name.clone(),
            attempt,
            backoff_ms: delay.as_millis() as u64,
        });
    },
    || {
        let tool_runner = Arc::clone(&tool_runner);
        let call = call_for_attempt.clone();
        let ctx = ctx_for_attempt.clone();
        async move { tool_runner.execute(call, Some(ctx)).await }
    },
);

// use `run` in both tokio::select! branches instead of `tool_runner.execute(...)`
```

Apply the same pattern in **both** `select!` branches (with and without `node_token`).

- [ ] **Step 3: Compile**

Run: `cargo check -p orchestration`

Expected: FAIL until Task 4 adds `ToolRetrying` to `RunTelemetry` — proceed to Task 4 before expecting green.

- [ ] **Step 4: Commit after Task 4 reducer lands**

```bash
git add crates/orchestration/src/run/execution/tool_port.rs
git commit -m "feat(orchestration): retry transient tool failures in tool_port (T20)"
```

---

### Task 4: `ToolRetrying` telemetry

**Files:**
- Modify: `crates/engine/src/execution/telemetry.rs`
- Modify: `crates/orchestration/src/run/execution/events.rs`

- [ ] **Step 1: Add variant to `RunTelemetry`**

In `crates/engine/src/execution/telemetry.rs`, after `ToolStarted`:

```rust
ToolRetrying {
    node_id: NodeId,
    tool_call_id: String,
    tool_name: String,
    attempt: u8,
    backoff_ms: u64,
},
```

Add test in same file:

```rust
#[test]
fn tool_retrying_debug() {
    let event = RunTelemetry::ToolRetrying {
        node_id: NodeId("n1".to_string()),
        tool_call_id: "tc-1".to_string(),
        tool_name: "bash".to_string(),
        attempt: 2,
        backoff_ms: 2000,
    };
    let debug = format!("{event:?}");
    assert!(debug.contains("ToolRetrying"));
    assert!(debug.contains("attempt: 2"));
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p engine tool_retrying_debug -- --nocapture`

Expected: PASS

- [ ] **Step 3: Handle in events reducer**

In `crates/orchestration/src/run/execution/events.rs`, find the `match event` arm block and add:

```rust
ExecutionEvent::ToolRetrying {
    node_id,
    tool_name,
    attempt,
    backoff_ms,
    ..
} => {
    state.push_trace(RunTraceEntry {
        node_id: node_id.to_string(),
        status: TraceStatus::Running,
        summary: format!(
            "Retrying {tool_name} (attempt {attempt}, backoff {backoff_ms}ms)"
        ),
        output: None,
        started_at_ms: now_ms(),
        finished_at_ms: None,
    });
}
```

Import/use existing `now_ms()` helper used by sibling arms.

- [ ] **Step 4: Compile orchestration**

Run: `cargo check -p orchestration`

Expected: PASS (exhaustive match may require updating other `match` sites — fix any compile errors in `incident/from_event.rs` with `_ => None` if needed)

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/execution/telemetry.rs crates/orchestration/src/run/execution/events.rs crates/orchestration/src/incident/from_event.rs
git commit -m "feat: emit ToolRetrying telemetry during tool backoff (T20)"
```

---

### Task 5: Wire retry into parallel shared tools

**Files:**
- Modify: `crates/orchestration/src/run/execution/tool_port.rs:228-252`

- [ ] **Step 1: Update parallel spawn closure**

Inside `run_parallel_regular_tools`, the spawned task currently calls `tool_runner.execute(call, Some(ctx))` directly. Replace with `execute_with_retry` using `workflow.settings.retry_policy` captured before the spawn loop:

```rust
let retry_policy = self.workflow.settings.retry_policy.clone();
let cancel_token = self.cancel_token.clone();
// Note: parallel path has no event_tx in the task — pass None for on_retry
// OR clone event_tx + node_id + call metadata for ToolRetrying emission (preferred)

join_handles.push(tokio::spawn(async move {
    let _permit = exclusive_permit;
    let conversation_id = node_id_for_task.0.clone();
    let ctx = ToolExecutionContext { /* unchanged */ };
    execute_with_retry(
        &retry_policy,
        &cancel_token,
        |_, _| {}, // emit ToolRetrying in a follow-up if event_tx cloned into task
        || {
            let tool_runner = Arc::clone(&tool_runner);
            let call = call.clone();
            let ctx = ctx.clone();
            async move { tool_runner.execute(call, Some(ctx)).await }
        },
    ).await
}));
```

**Preferred:** clone `event_tx`, `node_id`, `tool_call.id`, `tool_call.name` into the task and emit `ToolRetrying` in `on_retry` (same as Task 3).

- [ ] **Step 2: Run existing parallel tool tests**

Run: `cargo test -p orchestration run_parallel -- --nocapture` (if none, run full orchestration tests)

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/orchestration/src/run/execution/tool_port.rs
git commit -m "feat(orchestration): retry parallel shared tools with same policy (T20)"
```

---

### Task 6: Resilient `on_tool_results` (T21)

**Files:**
- Modify: `crates/engine/src/execution/interactive_engine/tools.rs`
- Test: `crates/engine/src/execution/interactive_engine/tests.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/engine/src/execution/interactive_engine/tests.rs`:

```rust
#[test]
fn partial_tool_results_fill_missing_calls_with_errors() {
    let mut workflow = Workflow::new("partial");
    let mut idea = node("idea");
    idea.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: None,
            tool_calls: vec![
                ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "a.md"}),
                },
                ToolCall {
                    id: "call-2".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "b.md"}),
                },
            ],
        })),
    );
    assert!(matches!(engine.poll(), EnginePollResult::RunTools { .. }));

    engine
        .on_tool_results(
            &NodeId::from("idea"),
            vec![ToolResult {
                tool_call_id: "call-1".to_string(),
                tool_name: "read".to_string(),
                content: "ok".to_string(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            }],
        )
        .expect("partial batch should not error");

    let resumed = engine.poll();
    assert!(
        matches!(resumed, EnginePollResult::CallAi { .. }),
        "expected CallAi after partial tool batch filled"
    );
    let request = match resumed {
        EnginePollResult::CallAi { request, .. } => request,
        _ => panic!("unreachable"),
    };
    let error_results = request
        .transcript
        .iter()
        .filter_map(|item| match item {
            AgentTranscriptItem::ToolResult { result } if result.is_error => Some(&result.tool_call_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(error_results.contains(&&"call-2".to_string()));
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p engine partial_tool_results_fill -- --nocapture`

Expected: FAIL — `on_tool_results` returns `Ok` but second call missing from transcript OR returns `Err`

- [ ] **Step 3: Implement fill-missing logic**

Replace `on_tool_results` body in `tools.rs`:

```rust
pub fn on_tool_results(
    &mut self,
    node_id: &NodeId,
    results: Vec<ToolResult>,
) -> Result<(), EngineInputError> {
    let approval_id = self
        .find_pending_tool_batch(node_id, false)
        .ok_or(EngineInputError::NoPendingTools)?;
    let pending_calls = self
        .pending_tool_batches
        .get(&approval_id)
        .map(|batch| batch.tool_calls.clone())
        .ok_or(EngineInputError::NoPendingTools)?;
    let mut by_id: std::collections::HashMap<String, ToolResult> = results
        .into_iter()
        .map(|result| (result.tool_call_id.clone(), result))
        .collect();
    let transcript = self.transcripts.entry(node_id.clone()).or_default();
    for call in &pending_calls {
        let result = by_id.remove(&call.id).unwrap_or_else(|| {
            crate::execution::tool_results::error_tool_result(
                call,
                "tool execution did not complete (interrupted or cancelled)",
            )
        });
        transcript.push(AgentTranscriptItem::ToolResult { result });
    }
    self.pending_tool_batches.remove(&approval_id);
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p engine partial_tool_results_fill denied_tool_call_resumes -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/execution/interactive_engine/tools.rs crates/engine/src/execution/interactive_engine/tests.rs
git commit -m "fix(engine): fill missing tool results so agent loop resumes (T21)"
```

---

### Task 7: Headless — permanent tool failure does not kill run

**Files:**
- Test: `crates/orchestration/src/run/execution/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn headless_run_survives_permanent_tool_failure_and_completes() {
    #[derive(Clone, Default)]
    struct ToolThenDoneAi {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl AiPort for ToolThenDoneAi {
        async fn invoke(
            &self,
            request: AgentRequest,
        ) -> Result<AgentTurnOutcome, engine::AgentError> {
            let mut calls = self.calls.lock();
            *calls += 1;
            if *calls == 1 {
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: None,
                    tool_calls: vec![ToolCall {
                        id: "call-missing".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "definitely-missing-file-orch-test.txt"}),
                    }],
                }));
            }
            let saw_error = request.transcript.iter().any(|item| {
                matches!(
                    item,
                    engine::AgentTranscriptItem::ToolResult { result }
                        if result.is_error && result.content.contains("[not_found]")
                )
            });
            assert!(saw_error, "model should see not_found tool error");
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "recovered"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    let temp = TempDir::new().expect("tempdir");
    let mut workflow = workflow();
    workflow.nodes[0].agent.tools.catalog.tools = vec![ToolRef {
        name: "read".to_string(),
        tier: Some(ToolTier::Read),
    }];
    workflow.nodes[0].agent.tools.approval_mode = Some(ApprovalMode::Yolo);

    let snapshot = run_workflow_headless(
        workflow,
        None,
        ToolThenDoneAi::default(),
        Vec::new(),
        Vec::new(),
        BTreeMap::new(),
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("run should complete after tool failure");

    assert_eq!(
        snapshot.outputs[&workflow.nodes[0].id.clone()],
        json!({"summary": "recovered"})
    );
    let tool_rows = &snapshot.tool_calls_by_node[&workflow.nodes[0].id];
    assert!(
        tool_rows.iter().any(|row| row.status == ToolCallStatus::Failed),
        "trace should record failed tool call"
    );
}
```

Fix any type mismatches (`ToolCallStatus`, `run_workflow_headless` cwd arg) against the real signatures in `tests.rs`.

- [ ] **Step 2: Run test**

Run: `cargo test -p orchestration headless_run_survives_permanent_tool_failure -- --nocapture`

Expected: PASS once Tasks 1–6 land (may already pass before T20; confirms T21)

- [ ] **Step 3: Commit**

```bash
git add crates/orchestration/src/run/execution/tests.rs
git commit -m "test(orchestration): headless run survives permanent tool failure (T21)"
```

---

### Task 8: Acceptance test in `workflow_acceptance.rs`

**Files:**
- Modify: `crates/orchestration/tests/workflow_acceptance.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn failed_read_tool_feeds_error_and_node_completes() {
    #[derive(Clone, Default)]
    struct RecoverAi {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl AiPort for RecoverAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            let n = {
                let mut calls = self.calls.lock();
                *calls += 1;
                *calls
            };
            if n == 1 {
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: None,
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "missing-acceptance-file.txt"}),
                    }],
                }));
            }
            assert!(request.transcript.iter().any(|item| matches!(
                item,
                engine::AgentTranscriptItem::ToolResult { result } if result.is_error
            )));
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok after tool error"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let mut workflow = Workflow::new("tool resilience");
    let mut node = agent("worker", "Worker");
    node.agent.tools.catalog.tools = vec![ToolRef {
        name: "read".to_string(),
        tier: Some(ToolTier::Read),
    }];
    node.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    workflow.nodes = vec![node];

    let snapshot = run_workflow_headless(
        workflow,
        None,
        RecoverAi::default(),
        vec![],
        vec![],
        BTreeMap::new(),
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("acceptance run completes");

    assert_eq!(
        snapshot.report.outputs.len(),
        1,
        "node should complete after tool error"
    );
}
```

- [ ] **Step 2: Run acceptance test**

Run: `cargo test -p orchestration --test workflow_acceptance failed_read_tool -- --nocapture`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/orchestration/tests/workflow_acceptance.rs
git commit -m "test: acceptance for resilient tool failure path (T21)"
```

---

### Task 9: Docs, roadmap, changelog, verify

**Files:**
- Modify: `docs/ROADMAP.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Update ROADMAP item #4**

In `docs/ROADMAP.md` queue table row #4, set status **Done** and note T20–T21 complete; hooks already done.

In [Tool invocation retry and resilience](../ROADMAP.md#tool-invocation-retry-and-resilience) section:
- Mark bullets 2–4 **Done**
- Update layer table: `tool_port.rs` retry loop **Done**; `interactive_engine` partial-batch fill **Done**

- [ ] **Step 2: Update CHANGELOG**

Add under `## Unreleased`:

```markdown
- **Tool retry & resilience (T20–T21):** transient tool failures retry per workflow `retry_policy` before surfacing `is_error` results; `ToolRetrying` telemetry; engine fills missing tool-batch results so cancelled/interrupted tools resume `CallAi` instead of aborting the run.
```

- [ ] **Step 3: Run verification**

Run: `./scripts/verify.sh`

Expected: all steps PASS

- [ ] **Step 4: Commit**

```bash
git add docs/ROADMAP.md CHANGELOG.md
git commit -m "docs: mark tool retry and resilient failure (T20–T21) done"
```

---

## Self-review

| Requirement | Task |
| --- | --- |
| T20 retry loop honoring `retry_policy` | Tasks 2–5 |
| T21 failed tools → transcript → `CallAi`, no run abort | Tasks 6–8 |
| T19 `is_retryable` wiring for `ToolRunnerError` | Task 1 |
| Hooks seam (already done) | Prerequisite — no task |
| Cancel respects user stop | `execute_with_retry` checks `CancellationToken` |
| `ToolStarted` once, not per retry | Retry wraps execute inside `execute_tool_or_cancel` after `emit_tool_started` in `run_regular_tool` |
| Subagent tool path | Uses same `execute_tool_or_cancel` — gets retry automatically |

**Placeholder scan:** none.

**Type consistency:** `RetryPolicy`, `ToolRunnerError`, `ToolRetrying`, `ExecutionEvent` = `RunTelemetry` — aligned across tasks.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-15-tool-retry-resilience.md`. Two execution options:

**1. Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?

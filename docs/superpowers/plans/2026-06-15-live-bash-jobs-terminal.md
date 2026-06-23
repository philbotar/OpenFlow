# Live Bash, Jobs, and Terminal Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add OMP-style background bash (`async` flag + `job` tool) and a user-facing terminal tab in the bottom dock, sharing cwd resolution with run execution.

**Architecture:** Own background job state in `orchestration/src/job/` (per-run `JobManager` on `ToolRunner`). Extend `bash` with optional `async: true` that registers a job and returns immediately. Add `job` builtin for poll/cancel/list. Terminal tab is user shell state in `orchestration/src/terminal/` with xterm.js in UI — independent from agent bash rows. Reuse existing `ToolUpdated` sink for live job output.

**Tech Stack:** Rust, Tokio, `portable-pty`, Tauri v2, SolidJS, xterm.js, Vitest.

**Related plan:** Terminal UI wiring details also live in `docs/superpowers/plans/2026-06-14-project-terminal-tab.md`. Execute Tasks 1–3 here first; then Tasks 4–6 (terminal tab) from that plan or the condensed steps below.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/orchestration/src/job/mod.rs` | `JobManager`, `BackgroundJob`, status snapshots, cancel/watch/ack |
| `crates/orchestration/src/job/bash_job.rs` | Spawn async bash, stream output into job record + `ToolUpdated` |
| `crates/orchestration/src/tool/registry.rs` | Add `job` schema; extend `bash` with `async` |
| `crates/orchestration/src/tool/runner.rs` | Own `JobManager` per run; route `job` dispatch |
| `crates/orchestration/src/adapters/tool_impl/bash.rs` | `async` early-return path |
| `crates/orchestration/src/adapters/tool_impl/job.rs` | `job` poll/cancel/list implementation |
| `crates/orchestration/src/terminal/mod.rs` | PTY terminal manager (user tab) |
| `crates/desktop/src/lib.rs` | Terminal + job-free IPC (terminal only) |
| `crates/ui/src/panels/TerminalPanel.tsx` | xterm host |
| `crates/ui/src/panels/DockPanel.tsx` | Terminal tab |
| `crates/engine/src/execution/node_invocation.rs` | Preamble guidance for `job` + `bash async` |

## V1 Scope

- `bash` with `async: true` returns `job_id` immediately; output streams via `ToolUpdated` until complete.
- `job` supports `list`, `poll`, `cancel` (OMP-aligned semantics).
- One interactive terminal session per app window (no chat injection in v1).
- Jobs are scoped to the active run; discarded when run ends.
- No PTY on agent `bash` (OMP `pty` deferred).

## Out of Scope

- Auto-background long bash without `async: true`.
- Shared state between terminal tab and agent bash.
- "Run in terminal" from chat tool rows.

---

### Task 1: Job Manager Core

**Files:**
- Create: `crates/orchestration/src/job/mod.rs`
- Modify: `crates/orchestration/src/lib.rs`
- Test: `crates/orchestration/src/job/mod.rs`

- [ ] **Step 1: Write failing job manager tests**

Create `crates/orchestration/src/job/mod.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct JobSnapshot {
    pub id: String,
    pub label: String,
    pub status: JobStatus,
    pub result_text: Option<String>,
    pub error_text: Option<String>,
}

pub struct JobManager {
    inner: Arc<Mutex<HashMap<String, JobRecord>>>,
}

struct JobRecord {
    snapshot: JobSnapshot,
    cancel: tokio_util::sync::CancellationToken,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, label: impl Into<String>) -> (String, tokio_util::sync::CancellationToken) {
        let id = Uuid::new_v4().to_string();
        let cancel = tokio_util::sync::CancellationToken::new();
        let snapshot = JobSnapshot {
            id: id.clone(),
            label: label.into(),
            status: JobStatus::Running,
            result_text: None,
            error_text: None,
        };
        self.inner.lock().insert(
            id.clone(),
            JobRecord {
                snapshot,
                cancel: cancel.clone(),
            },
        );
        (id, cancel)
    }

    pub fn complete(&self, id: &str, result_text: String) {
        if let Some(record) = self.inner.lock().get_mut(id) {
            record.snapshot.status = JobStatus::Completed;
            record.snapshot.result_text = Some(result_text);
        }
    }

    pub fn fail(&self, id: &str, error_text: String) {
        if let Some(record) = self.inner.lock().get_mut(id) {
            record.snapshot.status = JobStatus::Failed;
            record.snapshot.error_text = Some(error_text);
        }
    }

    pub fn cancel(&self, id: &str) -> bool {
        let mut guard = self.inner.lock();
        let Some(record) = guard.get_mut(id) else {
            return false;
        };
        if record.snapshot.status != JobStatus::Running {
            return false;
        }
        record.cancel.cancel();
        record.snapshot.status = JobStatus::Cancelled;
        true
    }

    pub fn snapshots(&self) -> Vec<JobSnapshot> {
        self.inner
            .lock()
            .values()
            .map(|record| record.snapshot.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{JobManager, JobStatus};

    #[test]
    fn register_and_complete_job() {
        let manager = JobManager::new();
        let (id, _cancel) = manager.register("cargo test");
        assert_eq!(manager.snapshots()[0].status, JobStatus::Running);
        manager.complete(&id, "ok".to_string());
        assert_eq!(manager.snapshots()[0].status, JobStatus::Completed);
    }

    #[test]
    fn cancel_running_job() {
        let manager = JobManager::new();
        let (id, cancel) = manager.register("sleep 60");
        assert!(manager.cancel(&id));
        assert!(cancel.is_cancelled());
        assert_eq!(manager.snapshots()[0].status, JobStatus::Cancelled);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p orchestration job::`

Expected: FAIL — `job` module not exported from `lib.rs`.

- [ ] **Step 3: Export module**

Add to `crates/orchestration/src/lib.rs`:

```rust
pub mod job;
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p orchestration job::`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/job/mod.rs crates/orchestration/src/lib.rs
git commit -m "feat(orchestration): add per-run JobManager for background bash"
```

---

### Task 2: Bash Async Mode

**Files:**
- Create: `crates/orchestration/src/job/bash_job.rs`
- Modify: `crates/orchestration/src/adapters/tool_impl/bash.rs`
- Modify: `crates/orchestration/src/tool/registry.rs`
- Modify: `crates/orchestration/src/tool/runner.rs`
- Test: `crates/orchestration/src/adapters/tool_impl/bash.rs`

- [ ] **Step 1: Write failing async bash test**

Add to `crates/orchestration/src/adapters/tool_impl/bash.rs` `#[cfg(test)]`:

```rust
#[tokio::test]
async fn bash_async_returns_job_handle_without_waiting() {
    use crate::job::JobManager;
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let jobs = Arc::new(JobManager::new());
    let args = serde_json::json!({
        "command": "echo hello-from-async",
        "async": true
    });
    let outcome = execute_bash_async(
        dir.path(),
        args,
        &CancellationToken::new(),
        jobs.clone(),
        None,
    )
    .await
    .expect("async bash starts");
    assert!(outcome.job_id.is_some());
    assert!(outcome.output.contains("job_id"));
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let done = jobs
        .snapshots()
        .into_iter()
        .find(|job| job.status == crate::job::JobStatus::Completed)
        .expect("job completed");
    assert!(done.result_text.unwrap_or_default().contains("hello-from-async"));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p orchestration bash_async_returns_job_handle_without_waiting`

Expected: FAIL — `execute_bash_async` not defined; `async` not in schema.

- [ ] **Step 3: Extend bash schema**

In `crates/orchestration/src/tool/registry.rs` `bash_tool()` properties, add:

```json
"async": {
    "type": "boolean",
    "description": "Run in background; returns a job id immediately. Poll with the job tool."
}
```

In `bash.rs` `BashArgs`:

```rust
#[serde(default)]
async_mode: Option<bool>,
```

Map with `#[serde(rename = "async")]`.

- [ ] **Step 4: Implement `execute_bash_async`**

In `crates/orchestration/src/job/bash_job.rs`, spawn `execute_bash` on a Tokio task keyed by `JobManager::register`. Return immediately:

```rust
pub struct BashAsyncOutcome {
    pub output: String,
    pub job_id: Option<String>,
}

pub async fn execute_bash_async(
    execution_cwd: &Path,
    args: Value,
    cancel_token: &CancellationToken,
    jobs: Arc<JobManager>,
    update_tx: Option<UnboundedSender<ToolExecutionUpdate>>,
) -> Result<BashAsyncOutcome, ToolError> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let (job_id, job_cancel) = jobs.register(truncate_label(&command, 80));
    let cwd = execution_cwd.to_path_buf();
    let child_cancel = cancel_token.child_token();
    tokio::spawn(async move {
        let merged_cancel = child_cancel;
        merged_cancel.run_until_cancelled(async {
            match execute_bash(&cwd, args, &job_cancel, update_tx).await {
                Ok(outcome) if outcome.is_error => {
                    jobs.fail(&job_id, outcome.output);
                }
                Ok(outcome) => jobs.complete(&job_id, outcome.output),
                Err(error) => jobs.fail(&job_id, error.to_string()),
            }
        })
        .await;
    });
    Ok(BashAsyncOutcome {
        output: format!("Started background job {job_id}"),
        job_id: Some(job_id),
    })
}
```

Wire `execute_bash` to branch on `async_mode == Some(true)`.

- [ ] **Step 5: Attach `JobManager` to `ToolRunner`**

Add `jobs: JobManager` field; construct in `ToolRunner::new`; pass `Arc::new(jobs)` into bash dispatch.

- [ ] **Step 6: Run test to verify pass**

Run: `cargo test -p orchestration bash_async_returns_job_handle_without_waiting`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/orchestration/src/job/bash_job.rs crates/orchestration/src/adapters/tool_impl/bash.rs crates/orchestration/src/tool/registry.rs crates/orchestration/src/tool/runner.rs
git commit -m "feat(tools): bash async mode with background JobManager"
```

---

### Task 3: Job Tool

**Files:**
- Create: `crates/orchestration/src/adapters/tool_impl/job.rs`
- Modify: `crates/orchestration/src/tool/registry.rs`
- Modify: `crates/orchestration/src/adapters/tool_impl/mod.rs`
- Modify: `crates/orchestration/src/tool/runner.rs`
- Test: `crates/orchestration/src/adapters/tool_impl/job.rs`

- [ ] **Step 1: Write failing job tool tests**

```rust
#[tokio::test]
async fn job_list_returns_snapshots() {
    let jobs = Arc::new(JobManager::new());
    let (id, _) = jobs.register("echo test");
    jobs.complete(&id, "test".to_string());
    let text = execute_job(
        &jobs,
        serde_json::json!({ "list": true }),
        &CancellationToken::new(),
    )
    .await
    .expect("job list");
    assert!(text.contains(&id));
    assert!(text.contains("Completed"));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p orchestration job_list_returns_snapshots`

Expected: FAIL

- [ ] **Step 3: Register `job` builtin**

Add `BuiltinToolKind::Job`, `job_tool()` in registry:

```rust
description: "Poll, cancel, or list background bash jobs started with bash async=true.",
input_schema: with_intent_field(serde_json::json!({
    "type": "object",
    "additionalProperties": false,
    "properties": {
        "list": { "type": "boolean" },
        "poll": { "type": "array", "items": { "type": "string" } },
        "cancel": { "type": "array", "items": { "type": "string" } }
    }
})),
tier: ToolTier::Read,
concurrency: ToolConcurrency::Shared,
```

- [ ] **Step 4: Implement `execute_job`**

OMP-aligned rules:
- `list: true` → immediate markdown snapshot, no wait.
- `cancel` only → cancel ids, return immediately.
- `poll` or no args → wait up to 30s (configurable later) for watched jobs to finish; emit `ToolUpdated` every 500ms while waiting.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test -p orchestration job_`

```bash
git commit -m "feat(tools): add job builtin for background bash polling"
```

---

### Task 4: Terminal Tab (Orchestration)

**Files:**
- Modify: `Cargo.toml`, `crates/orchestration/Cargo.toml`
- Create: `crates/orchestration/src/terminal/mod.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`

Follow Tasks 1–3 in `docs/superpowers/plans/2026-06-14-project-terminal-tab.md` verbatim for PTY manager, `resolve_terminal_cwd`, and `TerminalManager` with `start`/`write`/`resize`/`stop`.

- [ ] **Step 1–5:** Execute terminal plan Task 1 (orchestration manager).

---

### Task 5: Terminal Tab (Desktop + UI)

**Files:**
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/ui/src/api.ts`, `crates/ui/src/port.ts`
- Create: `crates/ui/src/panels/TerminalPanel.tsx`
- Modify: `crates/ui/src/panels/DockPanel.tsx`

- [ ] **Step 1–6:** Execute terminal plan Tasks 2–4 (IPC, xterm panel, dock tab).

Run: `npm --prefix crates/ui run typecheck && npm --prefix crates/ui run test`

---

### Task 6: Docs and Verification

**Files:**
- Modify: `docs/ROADMAP.md`
- Modify: `CHANGELOG.md`
- Modify: `crates/engine/src/execution/node_invocation.rs`

- [ ] **Step 1: Update node preamble**

Add to `NODE_RUNTIME_PREAMBLE`:

```text
- bash: pass `"async": true` for long commands; poll with `job` (`list`, `poll`, `cancel`).
- The Terminal dock tab is for the user only; agent commands use the bash tool.
```

- [ ] **Step 2: Run verify**

Run: `./scripts/verify.sh`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add docs/ROADMAP.md CHANGELOG.md crates/engine/src/execution/node_invocation.rs
git commit -m "docs: mark bash jobs and terminal tab v1 complete"
```

---

## Self-Review

| Requirement | Task |
| --- | --- |
| `bash async` returns immediately | Task 2 |
| `job` poll/cancel/list | Task 3 |
| Live output via `ToolUpdated` | Task 2 (`update_tx` passed to spawned bash) |
| User terminal tab | Tasks 4–5 |
| Per-run job scope | Task 2 (`JobManager` on `ToolRunner`) |

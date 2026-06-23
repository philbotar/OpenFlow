# Orchestration Refactor Smells Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce structural smells in `crates/orchestration` — split catch-all errors, decouple workflow/project domains, inject incident persistence, extract run edit helpers, modularize event projection, consolidate `ToolPortImpl` mutex state, and split oversized test modules — without changing engine execution semantics.

**Architecture:** Six independently mergeable waves. Each wave ships working software and passes `./scripts/verify.sh`. Domain folders stay adapter-free; engine construction stays in `run/execution/`. Prefer extracting modules over behavior changes.

**Tech Stack:** Rust workspace (`engine`, `orchestration`, `desktop`), `./scripts/verify.sh`, `cargo test -p orchestration`

**Related docs:** [orchestration AGENTS.md](../../crates/orchestration/AGENTS.md), [architecture/contract.md](../../docs/architecture/contract.md), [testing-workflows.md](../../docs/contributing/testing-workflows.md)

---

## Wave overview

| Wave | Delivers | Risk |
| --- | --- | --- |
| 1 | Typed `BackendError` variants + `EditRevertFailed` | Low — mechanical migration |
| 2 | `WorkflowCatalog` decoupled from `ProjectRegistry` | Low |
| 3 | `IncidentStore` injected via `AppBackendDeps` | Low |
| 4 | `run/edit_session.rs` extracted from `RunCoordinator` | Medium — run lifecycle touch |
| 5 | `events/` split + `ToolPortRuntimeState` | Medium — event projection |
| 6 | Split `backend/tests.rs` and `run/execution/tests.rs` | Low — test-only |

**Out of scope:** Edit-tool subsystem (`adapters/tool_impl/edit/patch.rs`) extraction, crate-level clippy cleanup, `ProviderProfile.new_model_input` UI split, `ScheduleService` port extraction.

---

## File map (all waves)

| File | Responsibility |
| --- | --- |
| `crates/orchestration/src/error.rs` | Add `ProjectValidation`, `FileReference`, `Terminal`, `EditRevertFailed`; remove `ProjectOperation` |
| `crates/orchestration/src/error_tests.rs` | Error variant display + incident code mapping smoke tests |
| `crates/orchestration/src/incident/recorder.rs` | Map new `BackendError` variants to incident codes |
| `crates/orchestration/src/project/registry.rs` | Use `ProjectValidation` |
| `crates/orchestration/src/project/file_refs.rs` | Use `FileReference` |
| `crates/orchestration/src/backend/mod.rs` | Terminal errors, incident injection, catalog call-site updates |
| `crates/desktop/src/lib.rs` | Map incident IPC `io::Error` to `BackendError::Io` (not `ProjectOperation`) |
| `crates/orchestration/src/workflow/catalog.rs` | Accept `&[Project]` instead of `&ProjectRegistry` |
| `crates/orchestration/src/run/edit_session.rs` | Preview, git diff, revert helpers (new) |
| `crates/orchestration/src/run/coordinator.rs` | Delegate edit/git to `edit_session`; shrink |
| `crates/orchestration/src/run/mod.rs` | `pub(crate) mod edit_session;` |
| `crates/orchestration/src/run/execution/events/mod.rs` | Dispatcher + re-exports |
| `crates/orchestration/src/run/execution/events/node.rs` | Node lifecycle handlers |
| `crates/orchestration/src/run/execution/events/tool.rs` | Tool call handlers |
| `crates/orchestration/src/run/execution/events/chat.rs` | Chat delta handlers |
| `crates/orchestration/src/run/execution/events/subagent.rs` | Subagent handlers |
| `crates/orchestration/src/run/execution/events/lifecycle.rs` | Finished / Aborted / Error |
| `crates/orchestration/src/run/execution/tool_port_state.rs` | `ToolPortRuntimeState` (new) |
| `crates/orchestration/src/run/execution/tool_port.rs` | Single mutex over `ToolPortRuntimeState` |
| `crates/orchestration/src/backend/run_ipc_tests.rs` | Extracted run IPC tests from `backend/tests.rs` |
| `crates/orchestration/src/backend/catalog_ipc_tests.rs` | Extracted catalog tests |
| `crates/orchestration/src/run/execution/event_projection_tests.rs` | Extracted from `tests.rs` |
| `CHANGELOG.md` | Entry per wave |

---

## Wave 1 — Split `BackendError`

### Task 1: Add typed error variants

**Files:**
- Modify: `crates/orchestration/src/error.rs`
- Create: `crates/orchestration/src/error_tests.rs`
- Modify: `crates/orchestration/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/orchestration/src/error_tests.rs`:

```rust
use crate::error::BackendError;

#[test]
fn project_validation_has_distinct_message() {
    let err = BackendError::ProjectValidation("path already registered".to_string());
    assert!(err.to_string().contains("path already registered"));
}

#[test]
fn file_reference_has_distinct_message() {
    let err = BackendError::FileReference("read src/foo.rs: not found".to_string());
    assert!(err.to_string().contains("read src/foo.rs"));
}

#[test]
fn terminal_has_distinct_message() {
    let err = BackendError::Terminal("session not found".to_string());
    assert!(err.to_string().contains("session not found"));
}

#[test]
fn edit_revert_failed_is_not_git_failed() {
    let err = BackendError::EditRevertFailed("disk full".to_string());
    assert!(err.to_string().contains("edit revert failed"));
    assert!(!err.to_string().contains("git"));
}
```

Add to `crates/orchestration/src/lib.rs`:

```rust
#[cfg(test)]
mod error_tests;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration error_tests -- --nocapture`

Expected: FAIL — variants `ProjectValidation`, `FileReference`, `Terminal`, `EditRevertFailed` not found

- [ ] **Step 3: Add variants to `error.rs`**

Replace `ProjectOperation` and add `EditRevertFailed` in `crates/orchestration/src/error.rs`:

```rust
    #[error("{0}")]
    ProjectValidation(String),
    #[error("{0}")]
    FileReference(String),
    #[error("{0}")]
    Terminal(String),
    #[error("edit revert failed: {0}")]
    EditRevertFailed(String),
```

Remove:

```rust
    #[error("{0}")]
    ProjectOperation(String),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration error_tests -- --nocapture`

Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/error.rs crates/orchestration/src/error_tests.rs crates/orchestration/src/lib.rs
git commit -m "refactor(orchestration): add typed BackendError variants"
```

---

### Task 2: Migrate orchestration call sites

**Files:**
- Modify: `crates/orchestration/src/project/registry.rs`
- Modify: `crates/orchestration/src/project/file_refs.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/orchestration/src/incident/recorder.rs`
- Modify: `crates/orchestration/src/backend/tests.rs`
- Modify: `crates/orchestration/src/run/coordinator.rs`

- [ ] **Step 1: Write the failing test**

In `crates/orchestration/src/backend/tests.rs`, update the project duplicate-path test (around line 232):

```rust
    assert!(matches!(error, BackendError::ProjectValidation(_)));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration backend::tests::create_project_rejects_duplicate_path -- --nocapture`

Expected: FAIL — still returns `ProjectOperation` or compile error

- [ ] **Step 3: Migrate call sites**

`project/registry.rs` — replace every `BackendError::ProjectOperation` with `BackendError::ProjectValidation`.

`project/file_refs.rs` — replace every `BackendError::ProjectOperation` with `BackendError::FileReference`.

`backend/mod.rs` — terminal methods (`write_terminal`, `resize_terminal`, `stop_terminal`, `start_terminal` error path ~538):

```rust
.map_err(BackendError::Terminal)
```

`incident/recorder.rs` — in `backend_error_code`, replace:

```rust
        BackendError::ProjectOperation(_) => "backend.project_operation",
```

with:

```rust
        BackendError::ProjectValidation(_) => "backend.project_validation",
        BackendError::FileReference(_) => "backend.file_reference",
        BackendError::Terminal(_) => "backend.terminal",
        BackendError::EditRevertFailed(_) => "backend.edit_revert_failed",
```

`run/coordinator.rs` — fix revert + join error mapping in `revert_edit_batch`:

```rust
        self.runtime_handle
            .spawn_blocking(move || {
                crate::tools::edit::batch::revert_edit_batch(&cwd, &batch_for_revert)
            })
            .await
            .map_err(|error| BackendError::EditRevertFailed(error.to_string()))?
            .map_err(BackendError::EditRevertFailed)?;
```

Leave `git_diff_file` on `BackendError::GitFailed` — that path calls `crate::git::diff_file`.

- [ ] **Step 4: Run full orchestration tests**

Run: `cargo test -p orchestration -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/project/registry.rs crates/orchestration/src/project/file_refs.rs \
  crates/orchestration/src/backend/mod.rs crates/orchestration/src/incident/recorder.rs \
  crates/orchestration/src/backend/tests.rs crates/orchestration/src/run/coordinator.rs
git commit -m "refactor(orchestration): migrate BackendError call sites"
```

---

### Task 3: Fix desktop incident IPC error mapping

**Files:**
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/orchestration/src/backend/tests.rs`:

```rust
#[test]
fn list_incidents_io_error_is_not_project_validation() {
    use std::io::{Error, ErrorKind};
    let err = BackendError::from(Error::new(ErrorKind::NotFound, "missing file"));
    assert!(matches!(err, BackendError::Io(_)));
}
```

- [ ] **Step 2: Run test — should already pass**

Run: `cargo test -p orchestration list_incidents_io_error -- --nocapture`

Expected: PASS (documents desired pattern)

- [ ] **Step 3: Update desktop incident commands**

In `crates/desktop/src/lib.rs`, replace all three incident command error mappers:

```rust
.map_err(|error| CommandError::from(backend.backend_err(BackendError::from(error))))
```

instead of wrapping in `BackendError::ProjectOperation(error.to_string())`.

- [ ] **Step 4: Verify desktop compiles**

Run: `cargo check -p desktop`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/lib.rs crates/orchestration/src/backend/tests.rs
git commit -m "fix(desktop): map incident IPC io errors to BackendError::Io"
```

---

### Task 4: Wave 1 verification + changelog

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Run verify**

Run: `./scripts/verify.sh`

Expected: all steps PASS

- [ ] **Step 2: Update changelog**

Add under `## Unreleased`:

```markdown
- **orchestration:** Split catch-all `BackendError::ProjectOperation` into `ProjectValidation`, `FileReference`, and `Terminal`; add `EditRevertFailed` for edit-batch revert failures (revert no longer maps to `GitFailed`).
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: changelog for BackendError split"
```

---

## Wave 2 — Decouple `WorkflowCatalog` from `ProjectRegistry`

### Task 5: Change catalog signatures to `&[Project]`

**Files:**
- Modify: `crates/orchestration/src/workflow/catalog.rs`
- Create: `crates/orchestration/src/workflow/catalog_tests.rs`
- Modify: `crates/orchestration/src/workflow/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/orchestration/src/workflow/catalog_tests.rs`:

```rust
use crate::project::ports::Project;
use crate::workflow::catalog::WorkflowCatalog;
use crate::workflow::ports::{ProjectWorkflowStore, WorkflowStore};
use engine::Workflow;
use std::io;
use std::path::Path;

struct MemWorkflowStore(Vec<Workflow>);

impl WorkflowStore for MemWorkflowStore {
    fn load(&self) -> io::Result<Vec<Workflow>> {
        Ok(self.0.clone())
    }
    fn save(&self, _workflows: &[Workflow]) -> io::Result<()> {
        Ok(())
    }
}

struct EmptyProjectWorkflows;

impl ProjectWorkflowStore for EmptyProjectWorkflows {
    fn discover(&self, _project_path: &Path) -> io::Result<Vec<Workflow>> {
        Ok(Vec::new())
    }
}

#[test]
fn load_all_accepts_project_slice_without_registry() {
    let wf = Workflow::new("demo");
    let catalog = WorkflowCatalog::new(
        Box::new(MemWorkflowStore(vec![wf.clone()])),
        Box::new(EmptyProjectWorkflows),
    );
    let projects: Vec<Project> = Vec::new();
    let loaded = catalog.load_all(&projects).expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, wf.id);
}
```

Wire in `workflow/mod.rs`:

```rust
#[cfg(test)]
mod catalog_tests;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration catalog_tests -- --nocapture`

Expected: FAIL — `load_all` expects `&ProjectRegistry`

- [ ] **Step 3: Update `catalog.rs`**

Remove `use crate::project::registry::ProjectRegistry;`.

Change every method signature from `projects: &ProjectRegistry` to `projects: &[Project]`.

In `load_all`:

```rust
    pub fn load_all(&self, projects: &[Project]) -> Result<Vec<Workflow>, BackendError> {
        let mut by_id = BTreeMap::<String, Workflow>::new();
        for workflow in self.store.load()? {
            by_id.insert(workflow.id.to_string(), workflow);
        }
        for project in projects {
            for workflow in self.project_workflows.discover(Path::new(&project.path))? {
                by_id.insert(workflow.id.to_string(), workflow);
            }
        }
        Ok(by_id.into_values().collect())
    }
```

Update `load_one`, `list`, `save`, `rename`, `delete`, `assign_to_project`, `unassign_from_project` similarly — replace `projects.load()?` with using the passed slice (callers load projects once).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration catalog_tests -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/workflow/catalog.rs crates/orchestration/src/workflow/catalog_tests.rs crates/orchestration/src/workflow/mod.rs
git commit -m "refactor(orchestration): WorkflowCatalog accepts project slice"
```

---

### Task 6: Update `AppBackend` call sites

**Files:**
- Modify: `crates/orchestration/src/backend/mod.rs`

- [ ] **Step 1: Add helper on `AppBackend`**

In `backend/mod.rs`:

```rust
    fn loaded_projects(&self) -> Result<Vec<Project>, BackendError> {
        self.projects.load()
    }
```

- [ ] **Step 2: Update every `self.workflows.*(&self.projects)` call**

Example — `list_workflows`:

```rust
    pub fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError> {
        let projects = self.loaded_projects()?;
        self.workflows.list(&projects)
    }
```

Apply the same pattern to `load_all_workflows`, `load_workflow`, `create_workflow`, `save_workflow`, `save_workflows`, `rename_workflow`, `validate_workflow`, `refresh_schedules_at` (loads workflows via catalog).

- [ ] **Step 3: Run tests**

Run: `cargo test -p orchestration backend::tests -- --nocapture`

Expected: PASS

- [ ] **Step 4: Run verify + changelog**

Run: `./scripts/verify.sh`

Add changelog line:

```markdown
- **orchestration:** `WorkflowCatalog` depends on `&[Project]` instead of `ProjectRegistry`, removing cross-entity coupling.
```

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/backend/mod.rs CHANGELOG.md
git commit -m "refactor(orchestration): backend loads projects for catalog calls"
```

---

## Wave 3 — Inject `IncidentStore` via `AppBackendDeps`

### Task 7: Add `incident_store` to deps

**Files:**
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/orchestration/src/backend/tests.rs`
- Modify: `crates/orchestration/src/run/coordinator_tests.rs` (if constructs `AppBackendDeps`)

- [ ] **Step 1: Write the failing test**

In `backend/tests.rs`, extend `AppBackendDeps` construction:

```rust
            incident_store: Box::new(FileIncidentStore::new(dir.path().join("incidents.jsonl"))),
```

- [ ] **Step 2: Run test — verify compile fails**

Run: `cargo test -p orchestration backend::tests -- --nocapture`

Expected: FAIL — field `incident_store` missing on `AppBackendDeps`

- [ ] **Step 3: Add field and wire `new`**

In `AppBackendDeps`:

```rust
use crate::incident::ports::IncidentStore;

pub struct AppBackendDeps {
    // ... existing fields ...
    pub incident_store: Box<dyn IncidentStore>,
}
```

In `default_deps`:

```rust
            incident_store: Box::new(FileIncidentStore::new(FileIncidentStore::default_path())),
```

In `AppBackend::new`, replace hardcoded store:

```rust
        let incidents = Arc::new(IncidentRecorder::with_retention_max(
            deps.incident_store,
            retention_max,
        ));
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p orchestration -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/backend/mod.rs crates/orchestration/src/backend/tests.rs
git commit -m "refactor(orchestration): inject IncidentStore via AppBackendDeps"
```

---

### Task 8: Wave 3 verification

- [ ] **Step 1: Run verify**

Run: `./scripts/verify.sh`

Expected: PASS

- [ ] **Step 2: Changelog**

```markdown
- **orchestration:** `IncidentStore` is injected through `AppBackendDeps` like other persistence ports.
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: changelog for incident store injection"
```

---

## Wave 4 — Extract run edit helpers from `RunCoordinator`

### Task 9: Create `run/edit_session.rs`

**Files:**
- Create: `crates/orchestration/src/run/edit_session.rs`
- Create: `crates/orchestration/src/run/edit_session_tests.rs`
- Modify: `crates/orchestration/src/run/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/orchestration/src/run/edit_session_tests.rs`:

```rust
use super::edit_session::ActiveEditSession;
use crate::run::state::WorkflowRunState;
use engine::{EditBatch, NodeId};
use std::path::PathBuf;

#[test]
fn active_edit_session_requires_cwd_and_run_state() {
    let session = ActiveEditSession {
        cwd: PathBuf::from("/tmp/ws"),
        run_state: WorkflowRunState::default(),
        snapshot_store: None,
        pending_engine_reverts: None,
    };
    assert!(session.cwd.is_dir() || !session.cwd.exists()); // construction only
    assert!(session.run_state.edit_batches.is_empty());
}
```

Add to `run/mod.rs`:

```rust
pub(crate) mod edit_session;

#[cfg(test)]
mod edit_session_tests;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration edit_session_tests -- --nocapture`

Expected: FAIL — module not found

- [ ] **Step 3: Implement `edit_session.rs`**

Create `crates/orchestration/src/run/edit_session.rs`:

```rust
use crate::api::FileEditPreview;
use crate::error::BackendError;
use crate::run::state::WorkflowRunState;
use crate::tools::edit::preview::preview_file_edit;
use engine::EditBatch;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ActiveEditSession {
    pub cwd: PathBuf,
    pub run_state: WorkflowRunState,
    pub snapshot_store: Option<Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>>,
    pub pending_engine_reverts: Option<Arc<Mutex<Vec<EditBatch>>>>,
}

impl ActiveEditSession {
    pub fn preview_file_edit(
        &self,
        approval_id: &str,
        tool_name: &str,
    ) -> Result<FileEditPreview, BackendError> {
        let snapshot_store = self
            .snapshot_store
            .clone()
            .ok_or(BackendError::NoActiveRun)?;
        let pending = self
            .run_state
            .pending_approvals
            .iter()
            .find(|pending| pending.approval_id == approval_id)
            .ok_or_else(|| {
                if self.run_state.pending_approvals.is_empty() {
                    BackendError::NoPendingApproval
                } else {
                    BackendError::WrongApprovalId {
                        expected: self.run_state.pending_approvals[0].approval_id.clone(),
                        received: approval_id.to_string(),
                    }
                }
            })?;
        if pending.tool_call.name != tool_name {
            return Err(BackendError::PreviewFailed(
                "preview does not match the pending tool approval".to_string(),
            ));
        }
        preview_file_edit(
            self.cwd.clone(),
            &pending.tool_call.name,
            &pending.tool_call.arguments,
            snapshot_store,
        )
        .map_err(BackendError::PreviewFailed)
    }

    pub fn git_diff_file(&self, path: &str) -> Result<String, BackendError> {
        crate::git::diff_file(&self.cwd, path).map_err(|error| BackendError::GitFailed(error.to_string()))
    }

    pub fn revert_edit_batch(&self, batch_id: &str) -> Result<(EditBatch, WorkflowRunState), BackendError> {
        let batch = self
            .run_state
            .edit_batches
            .iter()
            .find(|batch| batch.batch_id == batch_id)
            .cloned()
            .ok_or_else(|| BackendError::EditBatchNotFound(batch_id.to_string()))?;
        crate::tools::edit::batch::revert_edit_batch(&self.cwd, &batch)
            .map_err(BackendError::EditRevertFailed)?;
        if let Some(pending) = &self.pending_engine_reverts {
            pending.lock().push(batch.clone());
        }
        let mut run_state = self.run_state.clone();
        run_state
            .edit_batches
            .retain(|existing| existing.batch_id != batch_id);
        Ok((batch, run_state))
    }
}
```

Copy the exact post-revert `run_state` mutation from `coordinator.rs` `revert_edit_batch` (lines ~819–835) into `revert_edit_batch` — match existing trace/status updates.

- [ ] **Step 4: Run test**

Run: `cargo test -p orchestration edit_session_tests -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/run/edit_session.rs crates/orchestration/src/run/edit_session_tests.rs crates/orchestration/src/run/mod.rs
git commit -m "refactor(orchestration): add run edit_session helpers"
```

---

### Task 10: Delegate from `RunCoordinator`

**Files:**
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Modify: `crates/orchestration/src/run/coordinator_tests.rs`

- [ ] **Step 1: Add snapshot builder helper in coordinator**

```rust
use crate::run::edit_session::ActiveEditSession;

fn active_edit_session(session: &RunSession) -> Result<ActiveEditSession, BackendError> {
    Ok(ActiveEditSession {
        cwd: session.execution_cwd.clone().ok_or(BackendError::NoExecutionCwd)?,
        run_state: session.run_state.clone().ok_or(BackendError::NoActiveRun)?,
        snapshot_store: session.snapshot_store.clone(),
        pending_engine_reverts: session.pending_engine_reverts.clone(),
    })
}
```

- [ ] **Step 2: Replace `preview_file_edit` body**

```rust
    pub async fn preview_file_edit(
        &self,
        approval_id: &str,
        tool_name: String,
        _arguments: serde_json::Value,
    ) -> Result<FileEditPreview, BackendError> {
        let edit = {
            let session = self.session.lock().await;
            active_edit_session(&session)?
        };
        let approval_id = approval_id.to_string();
        self.runtime_handle
            .spawn_blocking(move || edit.preview_file_edit(&approval_id, &tool_name))
            .await
            .map_err(|error| BackendError::PreviewFailed(error.to_string()))?
    }
```

- [ ] **Step 3: Replace `git_diff_file` and `revert_edit_batch` similarly**

`git_diff_file`: build `ActiveEditSession` with only `cwd` required; call `git_diff_file` in `spawn_blocking`.

`revert_edit_batch`: call `edit.revert_edit_batch` in `spawn_blocking`, then apply returned `run_state` to session under lock.

- [ ] **Step 4: Run coordinator tests**

Run: `cargo test -p orchestration coordinator_tests run::execution::tests -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/run/coordinator.rs
git commit -m "refactor(orchestration): delegate edit/git ops to edit_session"
```

---

### Task 11: Wave 4 verification

- [ ] **Step 1: Run verify + acceptance**

Run: `./scripts/verify.sh`
Run: `cargo test -p orchestration --test workflow_acceptance -- --nocapture`

Expected: PASS

- [ ] **Step 2: Changelog + commit**

```markdown
- **orchestration:** Extract preview/git/revert helpers from `RunCoordinator` into `run/edit_session.rs`.
```

```bash
git add CHANGELOG.md
git commit -m "docs: changelog for edit_session extraction"
```

---

## Wave 5 — Split `events.rs` and consolidate `ToolPortImpl` state

### Task 12: Extract event handler modules

**Files:**
- Create: `crates/orchestration/src/run/execution/events/mod.rs`
- Create: `crates/orchestration/src/run/execution/events/node.rs`
- Create: `crates/orchestration/src/run/execution/events/tool.rs`
- Create: `crates/orchestration/src/run/execution/events/chat.rs`
- Create: `crates/orchestration/src/run/execution/events/subagent.rs`
- Create: `crates/orchestration/src/run/execution/events/lifecycle.rs`
- Delete: `crates/orchestration/src/run/execution/events.rs` (after move)
- Modify: `crates/orchestration/src/run/execution/mod.rs`

- [ ] **Step 1: Write the failing test**

Move existing tests from bottom of `events.rs` into `crates/orchestration/src/run/execution/event_projection_tests.rs` unchanged. Wire:

```rust
// run/execution/mod.rs
#[cfg(test)]
#[path = "event_projection_tests.rs"]
mod event_projection_tests;
```

- [ ] **Step 2: Run tests — should pass before split**

Run: `cargo test -p orchestration event_projection -- --nocapture`

Expected: PASS (baseline)

- [ ] **Step 3: Create `events/mod.rs` dispatcher**

```rust
mod chat;
mod lifecycle;
mod node;
mod subagent;
mod tool;

use crate::run::state::WorkflowRunState;
use engine::Workflow;

use super::ExecutionEvent;

pub use lifecycle::{record_entrypoint_message, record_user_input};

pub fn apply_event_to_run_state(
    workflow: &Workflow,
    state: &mut WorkflowRunState,
    event: ExecutionEvent,
) {
    match event {
        ExecutionEvent::NodeQueued { .. }
        | ExecutionEvent::NodeStarted { .. }
        | ExecutionEvent::NodeAwaitingInput { .. }
        | ExecutionEvent::NodeCompleted { .. }
        | ExecutionEvent::NodeInterrupted { .. }
        | ExecutionEvent::NodeErrored { .. }
        | ExecutionEvent::NodeFailed { .. } => node::apply(workflow, state, event),
        ExecutionEvent::ChatMessage { .. } | ExecutionEvent::ChatMessageDelta { .. } => {
            chat::apply(state, event)
        }
        ExecutionEvent::ToolCallProposed { .. }
        | ExecutionEvent::ToolApprovalRequested { .. }
        | ExecutionEvent::ToolApproved { .. }
        | ExecutionEvent::ToolDenied { .. }
        | ExecutionEvent::ToolStarted { .. }
        | ExecutionEvent::ToolRetrying { .. }
        | ExecutionEvent::ToolUpdated { .. }
        | ExecutionEvent::ToolCompleted { .. }
        | ExecutionEvent::FileChanged { .. }
        | ExecutionEvent::EditBatchRecorded { .. }
        | ExecutionEvent::ToolArtifactCreated { .. } => tool::apply(state, event),
        ExecutionEvent::SubagentsDeclared { .. }
        | ExecutionEvent::SubagentStarted { .. }
        | ExecutionEvent::SubagentCompleted { .. }
        | ExecutionEvent::SubagentFailed { .. } => subagent::apply(state, event),
        ExecutionEvent::Finished(_)
        | ExecutionEvent::Aborted
        | ExecutionEvent::Error(_)
        | ExecutionEvent::PhaseTimed { .. } => lifecycle::apply(state, event),
    }
}
```

Move each `ExecutionEvent::*` arm block from `events.rs` into the matching submodule. Move shared helpers (`find_tool_call_mut`, `remove_awaiting_node`, etc.) into the submodule that uses them, or `events/shared.rs` if used by multiple.

Update `run/execution/mod.rs`:

```rust
mod events;
```

(remove `mod events;` pointing at `events.rs` file — the folder replaces it)

- [ ] **Step 4: Run event projection tests**

Run: `cargo test -p orchestration event_projection -- --nocapture`

Expected: PASS (same behavior)

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/run/execution/events/ crates/orchestration/src/run/execution/event_projection_tests.rs crates/orchestration/src/run/execution/mod.rs
git rm crates/orchestration/src/run/execution/events.rs
git commit -m "refactor(orchestration): split events.rs into focused modules"
```

---

### Task 13: Add `ToolPortRuntimeState`

**Files:**
- Create: `crates/orchestration/src/run/execution/tool_port_state.rs`
- Modify: `crates/orchestration/src/run/execution/tool_port.rs`
- Modify: `crates/orchestration/src/run/execution/mod.rs`

- [ ] **Step 1: Write the failing test**

Add to `event_projection_tests.rs` or new `tool_port_state_tests.rs`:

```rust
use crate::run::execution::tool_port_state::ToolPortRuntimeState;
use std::collections::{BTreeMap, HashSet};
use tokio::sync::Semaphore;

#[test]
fn runtime_state_defaults_empty() {
    let state = ToolPortRuntimeState::new(BTreeMap::new());
    assert!(state.declared_subagents.is_empty());
    assert!(state.predefined_registered.is_empty());
    assert!(!state.aborted_emitted);
    assert!(state.exclusive_semaphores.is_empty());
}
```

- [ ] **Step 2: Run test — verify fails**

Run: `cargo test -p orchestration tool_port_state -- --nocapture`

Expected: FAIL

- [ ] **Step 3: Implement `tool_port_state.rs`**

```rust
use engine::{NodeId, SubagentSummary};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Debug)]
pub struct ToolPortRuntimeState {
    pub declared_subagents: BTreeMap<String, SubagentSummary>,
    pub predefined_registered: HashSet<NodeId>,
    pub proposed_tool_calls: HashSet<String>,
    pub aborted_emitted: bool,
    pub exclusive_semaphores: BTreeMap<String, Arc<Semaphore>>,
}

impl ToolPortRuntimeState {
    pub fn new(declared_subagents: BTreeMap<String, SubagentSummary>) -> Self {
        Self {
            declared_subagents,
            predefined_registered: HashSet::new(),
            proposed_tool_calls: HashSet::new(),
            aborted_emitted: false,
            exclusive_semaphores: BTreeMap::new(),
        }
    }
}
```

- [ ] **Step 4: Refactor `ToolPortImpl`**

Replace five `parking_lot::Mutex` fields with:

```rust
    runtime: parking_lot::Mutex<ToolPortRuntimeState>,
```

Update `new()` and all `.lock()` sites to use `self.runtime.lock()` and field access on `ToolPortRuntimeState`.

Remove `#[allow(clippy::too_many_arguments)]` only if arg count drops; keep if still 8 deps.

- [ ] **Step 5: Run tests**

Run: `cargo test -p orchestration -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/orchestration/src/run/execution/tool_port_state.rs crates/orchestration/src/run/execution/tool_port.rs crates/orchestration/src/run/execution/mod.rs
git commit -m "refactor(orchestration): consolidate ToolPortImpl mutex state"
```

---

### Task 14: Wave 5 verification

- [ ] **Step 1: Run verify + acceptance**

Run: `./scripts/verify.sh`
Run: `cargo test -p orchestration --test workflow_acceptance -- --nocapture`

Expected: PASS

- [ ] **Step 2: Changelog**

```markdown
- **orchestration:** Split `run/execution/events.rs` into node/tool/chat/subagent/lifecycle modules; consolidate `ToolPortImpl` runtime fields behind one mutex.
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: changelog for events split and ToolPort state"
```

---

## Wave 6 — Split oversized test modules

### Task 15: Split `backend/tests.rs`

**Files:**
- Create: `crates/orchestration/src/backend/run_ipc_tests.rs`
- Create: `crates/orchestration/src/backend/catalog_ipc_tests.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/orchestration/src/backend/tests.rs` (shrink to shared helpers only)

- [ ] **Step 1: Identify test groups**

Move tests whose names contain `run`, `start_run`, `stop_run`, `submit_`, `approval`, `interrupt`, `retry` into `run_ipc_tests.rs`.

Move tests for `list_workflows`, `save_workflow`, `create_workflow`, `assign_workflow` into `catalog_ipc_tests.rs`.

Keep `fn backend()` helper in `tests.rs` and `pub(crate) use` it from new files:

```rust
// backend/tests.rs
pub(super) fn backend() -> (AppBackend, tempfile::TempDir) { ... }

#[cfg(test)]
mod run_ipc_tests;
#[cfg(test)]
mod catalog_ipc_tests;
```

- [ ] **Step 2: Run backend tests**

Run: `cargo test -p orchestration backend:: -- --nocapture`

Expected: PASS — same test count as before split

- [ ] **Step 3: Verify no file exceeds ~400 lines**

Run: `wc -l crates/orchestration/src/backend/*tests*.rs`

Target: each file &lt; 400 lines

- [ ] **Step 4: Commit**

```bash
git add crates/orchestration/src/backend/
git commit -m "test(orchestration): split backend integration tests by domain"
```

---

### Task 16: Split `run/execution/tests.rs`

**Files:**
- Create: `crates/orchestration/src/run/execution/event_projection_tests.rs` (if not done in Wave 5)
- Create: `crates/orchestration/src/run/execution/drive_tests.rs`
- Create: `crates/orchestration/src/run/execution/tool_port_tests.rs`
- Modify: `crates/orchestration/src/run/execution/mod.rs`
- Shrink: `crates/orchestration/src/run/execution/tests.rs`

- [ ] **Step 1: Move test groups**

| Target file | Tests about |
| --- | --- |
| `event_projection_tests.rs` | `apply_event_to_run_state`, chat deltas, trace |
| `tool_port_tests.rs` | tool approval, subagent declare/call |
| `drive_tests.rs` | `spawn_interactive_workflow_run`, headless harness |
| `tests.rs` | shared fixtures only (`sample_agent_request`, `TestAi`, temp dirs) |

Wire in `mod.rs`:

```rust
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "event_projection_tests.rs"]
mod event_projection_tests;
#[cfg(test)]
#[path = "drive_tests.rs"]
mod drive_tests;
#[cfg(test)]
#[path = "tool_port_tests.rs"]
mod tool_port_tests;
```

- [ ] **Step 2: Run execution tests**

Run: `cargo test -p orchestration run::execution -- --nocapture`

Expected: PASS

- [ ] **Step 3: Final verify**

Run: `./scripts/verify.sh`

Expected: PASS

- [ ] **Step 4: Changelog + commit**

```markdown
- **orchestration:** Split oversized `backend/tests.rs` and `run/execution/tests.rs` per testing conventions.
```

```bash
git add crates/orchestration/src/run/execution/ crates/orchestration/src/backend/ CHANGELOG.md
git commit -m "test(orchestration): split execution integration tests"
```

---

## Self-review

| Smell (audit) | Task(s) | Covered? |
| --- | --- | --- |
| Catch-all `ProjectOperation` | 1–3 | Yes |
| Mislabeled `GitFailed` on revert | 2 | Yes |
| `WorkflowCatalog` → `ProjectRegistry` coupling | 5–6 | Yes |
| Hardcoded `FileIncidentStore` | 7 | Yes |
| Edit/git in `RunCoordinator` | 9–10 | Yes |
| Giant `events.rs` | 12 | Yes |
| `ToolPortImpl` mutex sprawl | 13 | Yes |
| Oversized test modules | 15–16 | Yes |
| Edit `patch.rs` framework | — | Out of scope |
| `ProviderProfile` UI fields | — | Out of scope |
| Crate-level clippy allow | — | Out of scope |

**Placeholder scan:** No TBD/TODO steps.

**Type consistency:** `ActiveEditSession`, `ToolPortRuntimeState`, and new `BackendError` variants are defined before use in later tasks.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-16-orchestration-refactor-smells.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — run tasks in this session with `executing-plans`, batch by wave with checkpoints

Which approach?

# Durable Run Persistence And Replay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist every interactive workflow run to disk so stopped or paused runs can be resumed after app restart, and completed or failed runs can be opened as read-only replay state.

**Architecture:** Keep execution semantics in `engine`; add run persistence models and ports under `crates/orchestration/src/run/`, with the file-backed adapter in `crates/orchestration/src/adapters/storage/`. `drive.rs` owns engine checkpoints, `RunCoordinator::apply_execution_event` owns projected `WorkflowRunState`, and durable checkpoints combine those two snapshots after pause, stop, completion, or failure. Desktop exposes list/replay/resume commands, while the UI treats replay as read-only state and durable resume as a normal active run with the same `runId`.

**Tech Stack:** Rust, serde/serde_json, existing `InteractiveEngineCheckpoint`, Tauri v2 commands/events, SolidJS, Vitest.

---

## Supersedes

This plan supersedes `docs/superpowers/plans/2026-06-15-run-persistence.md` for the first shippable slice. That older draft is directionally correct, but it includes fork/export work and leaves important coordinator/store details underspecified. Forking and export should be planned after durable replay/resume is working.

## File Structure

| File | Responsibility |
| --- | --- |
| `docs/architecture/run-persistence.md` | Durable persistence decisions, storage layout, checkpoint reasons, replay/resume boundary. |
| `crates/orchestration/src/run/persistence.rs` | Serializable run persistence models and workflow hash helper. |
| `crates/orchestration/src/run/ports.rs` | `RunCheckpointStore` trait used by run/coordinator. |
| `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs` | File-backed run store implementation. |
| `crates/orchestration/src/adapters/storage/mod.rs` | Exports `run_checkpoint_store`. |
| `crates/orchestration/src/run/mod.rs` | Exports `persistence` and `ports`. |
| `crates/orchestration/src/run/execution/mod.rs` | Changes checkpoint sink type from raw engine checkpoint to pending durable checkpoint. |
| `crates/orchestration/src/run/execution/drive.rs` | Emits pending durable checkpoints when engine reaches pause/terminal/abort boundaries. |
| `crates/orchestration/src/run/coordinator.rs` | Creates run records, selects run root, appends durable checkpoints, resumes from disk, replays projections. |
| `crates/orchestration/src/backend/mod.rs` | Wires `FileRunCheckpointStore`, resolves project/app run roots, delegates list/replay/resume. |
| `crates/orchestration/src/error.rs` | Adds actionable durable-run errors. |
| `crates/desktop/src/lib.rs` | Adds Tauri commands and event bridge for durable resume. |
| `crates/ui/src/lib/types.ts` | Adds run summary/replay DTOs and `runId` to `WorkflowRunState`. |
| `crates/ui/src/api.ts` | Typed wrappers for durable run commands. |
| `crates/ui/src/port.ts` | Adds durable run methods to `UiDesktopOutboundPort`. |
| `crates/ui/src/context/AppContext.tsx` | Exposes run history state/actions. |
| `crates/ui/src/context/AppProvider.tsx` | Loads run history, opens replay, resumes durable runs, and prevents replay state from acting active. |
| `crates/ui/src/panels/RunHistoryPanel.tsx` | Read-only run history UI. |
| `crates/ui/src/panels/DockPanel.tsx` | Adds the `Runs` tab and renders `RunHistoryPanel`. |
| `crates/ui/src/styles/index.css` | Adds compact dock styling for history rows/actions. |

## Storage Layout

Project workflows persist under the project root:

```text
{project}/.flow/runs/{run_id}/
├── run.json
├── checkpoints/
│   ├── 0001.json
│   └── 0002.json
└── artifacts/
```

App-only workflows persist under app data:

```text
{data_local}/openflow/runs/{run_id}/
├── run.json
├── checkpoints/
│   ├── 0001.json
│   └── 0002.json
└── artifacts/
```

## Task 1: Architecture Doc And Run Persistence Models

**Files:**
- Create: `docs/architecture/run-persistence.md`
- Create: `crates/orchestration/src/run/persistence.rs`
- Create: `crates/orchestration/src/run/ports.rs`
- Modify: `crates/orchestration/src/run/mod.rs`
- Test: `crates/orchestration/src/run/persistence.rs`

- [ ] **Step 1: Write the architecture doc**

Create `docs/architecture/run-persistence.md` with this content:

```markdown
# Run Persistence

Durable run persistence stores interactive workflow attempts as append-only run records. Engine behavior remains in `engine`; orchestration stores snapshots, resumes host resources, and projects replay state for the UI.

## Decisions

| ID | Decision |
| --- | --- |
| R1 | A run id identifies one attempt. Resume after restart keeps the same run id. |
| R2 | Project-assigned workflows store runs under `{project}/.flow/runs/`; app-only workflows store under `{data_local}/openflow/runs/`. |
| R3 | Each checkpoint stores `InteractiveEngineCheckpoint`, `WorkflowRunState`, and the artifact summaries already present in the projection. |
| R4 | Checkpoints are appended after human-input pauses, tool-approval pauses, retry pauses, user stops, successful completion, and terminal failures. |
| R5 | Read-only replay loads the latest checkpoint projection and never starts `drive.rs` or calls a provider. |
| R6 | Durable resume loads the latest checkpoint, validates it against the current workflow and workflow hash, rebuilds host resources, then starts `drive.rs` with `resume_checkpoint`. |
| R7 | Forking and export are intentionally out of this slice. |

## Replay Versus Resume

Replay is a UI inspection mode. It returns `WorkflowRunState` with `active = false` and clears pending approvals so old approval buttons cannot execute.

Resume is an execution mode. It uses the checkpoint engine state, keeps the original run id, writes future checkpoints to the same run directory, and reuses the same artifact directory.
```

- [ ] **Step 2: Add persistence models with a failing serialization test**

Create `crates/orchestration/src/run/persistence.rs`:

```rust
use crate::run::state::WorkflowRunState;
use engine::{InteractiveEngineCheckpoint, Workflow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Paused,
    Stopped,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunCheckpointReason {
    AwaitingInput,
    AwaitingToolApproval,
    AwaitingRetry,
    UserStopped,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub run_id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub workflow_hash: String,
    pub project_id: Option<String>,
    pub execution_cwd: String,
    pub artifact_root: String,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
    pub status: RunStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCheckpointPayload {
    pub seq: u32,
    pub created_at_ms: i64,
    pub reason: RunCheckpointReason,
    pub engine: InteractiveEngineCheckpoint,
    pub projection: WorkflowRunState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingRunCheckpoint {
    pub reason: RunCheckpointReason,
    pub engine: InteractiveEngineCheckpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStoreRoot {
    pub project_id: Option<String>,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub run_id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub project_id: Option<String>,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
    pub status: RunStatus,
}

#[must_use]
pub fn workflow_hash(workflow: &Workflow) -> String {
    let bytes = serde_json::to_vec(workflow).expect("workflow must serialize for run hash");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

impl RunRecord {
    #[must_use]
    pub fn summary(&self) -> RunSummary {
        RunSummary {
            run_id: self.run_id.clone(),
            workflow_id: self.workflow_id.clone(),
            workflow_name: self.workflow_name.clone(),
            project_id: self.project_id.clone(),
            started_at_ms: self.started_at_ms,
            updated_at_ms: self.updated_at_ms,
            status: self.status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Workflow;

    #[test]
    fn run_record_serializes_camel_case_fields() {
        let record = RunRecord {
            run_id: "run-1".to_string(),
            workflow_id: "wf-1".to_string(),
            workflow_name: "Demo".to_string(),
            workflow_hash: "abc".to_string(),
            project_id: Some("project-1".to_string()),
            execution_cwd: "/tmp/demo".to_string(),
            artifact_root: "/tmp/demo/.flow/runs/run-1/artifacts".to_string(),
            started_at_ms: 1,
            updated_at_ms: 2,
            status: RunStatus::Paused,
        };

        let json = serde_json::to_string(&record).expect("serialize run record");

        assert!(json.contains("runId"));
        assert!(json.contains("workflowId"));
        assert!(json.contains("artifactRoot"));
        assert!(json.contains("\"paused\""));
    }

    #[test]
    fn workflow_hash_changes_when_workflow_changes() {
        let mut first = Workflow::new("first");
        let second = first.clone();
        first.name = "changed".to_string();

        assert_ne!(workflow_hash(&first), workflow_hash(&second));
    }
}
```

- [ ] **Step 3: Run the failing model test**

Run:

```bash
cargo test -p orchestration run::persistence -- --nocapture
```

Expected: FAIL because `crate::run::persistence` and `crate::run::ports` are not exported and `sha2` is not declared for `orchestration`.

- [ ] **Step 4: Export the modules and add dependency**

Modify `crates/orchestration/src/run/mod.rs`:

```rust
pub mod coordinator;
pub mod execution;
pub mod persistence;
pub mod ports;
pub mod reasoning_defaults;
pub mod state;
```

Add `sha2 = "0.10.9"` to `[workspace.dependencies]` in `Cargo.toml`, then add `sha2.workspace = true` to `[dependencies]` in `crates/orchestration/Cargo.toml`.

- [ ] **Step 5: Add the run persistence port**

Create `crates/orchestration/src/run/ports.rs`:

```rust
use crate::run::persistence::{RunCheckpointPayload, RunRecord, RunStatus, RunStoreRoot, RunSummary};
use std::io;
use std::path::PathBuf;

pub trait RunCheckpointStore: Send + Sync {
    fn create_run(&self, root: &RunStoreRoot, record: &RunRecord) -> io::Result<()>;
    fn append_checkpoint(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
        payload: &RunCheckpointPayload,
    ) -> io::Result<()>;
    fn load_record(&self, roots: &[RunStoreRoot], run_id: &str) -> io::Result<Option<(RunStoreRoot, RunRecord)>>;
    fn load_latest_checkpoint(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
    ) -> io::Result<Option<RunCheckpointPayload>>;
    fn list_runs(&self, roots: &[RunStoreRoot], workflow_id: Option<&str>) -> io::Result<Vec<RunSummary>>;
    fn update_status(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
        status: RunStatus,
        updated_at_ms: i64,
    ) -> io::Result<()>;
    fn run_dir(&self, root: &RunStoreRoot, run_id: &str) -> PathBuf;
}
```

- [ ] **Step 6: Run the model test**

Run:

```bash
cargo test -p orchestration run::persistence -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add docs/architecture/run-persistence.md Cargo.toml crates/orchestration/Cargo.toml crates/orchestration/src/run/mod.rs crates/orchestration/src/run/persistence.rs crates/orchestration/src/run/ports.rs
git commit -m "feat: define durable run persistence model"
```

## Task 2: File Run Checkpoint Store

**Files:**
- Create: `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs`
- Modify: `crates/orchestration/src/adapters/storage/mod.rs`
- Test: `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs`

- [ ] **Step 1: Write the failing store test**

Create `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs` with the implementation shell and tests:

```rust
use crate::adapters::storage::json_file_store::atomic_write;
use crate::run::persistence::{
    RunCheckpointPayload, RunRecord, RunStatus, RunStoreRoot, RunSummary,
};
use crate::run::ports::RunCheckpointStore;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const RUN_FILE_NAME: &str = "run.json";
const CHECKPOINTS_DIR_NAME: &str = "checkpoints";

#[derive(Debug, Clone, Copy, Default)]
pub struct FileRunCheckpointStore;

impl FileRunCheckpointStore {
    #[must_use]
    pub fn app_runs_root() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(crate::adapters::storage::json_file_store::OPENFLOW_DATA_DIR_SLUG)
            .join("runs")
    }
}

fn run_dir(root: &RunStoreRoot, run_id: &str) -> PathBuf {
    root.root.join(run_id)
}

fn checkpoints_dir(root: &RunStoreRoot, run_id: &str) -> PathBuf {
    run_dir(root, run_id).join(CHECKPOINTS_DIR_NAME)
}

fn checkpoint_path(root: &RunStoreRoot, run_id: &str, seq: u32) -> PathBuf {
    checkpoints_dir(root, run_id).join(format!("{seq:04}.json"))
}

fn read_run_record(path: &Path) -> io::Result<RunRecord> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("run record {} invalid: {error}", path.display()),
        )
    })
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidData, format!("run JSON serialization failed: {error}"))
    })?;
    atomic_write(path, &text)
}

impl RunCheckpointStore for FileRunCheckpointStore {
    fn create_run(&self, root: &RunStoreRoot, record: &RunRecord) -> io::Result<()> {
        fs::create_dir_all(checkpoints_dir(root, &record.run_id))?;
        fs::create_dir_all(&record.artifact_root)?;
        write_json(&run_dir(root, &record.run_id).join(RUN_FILE_NAME), record)
    }

    fn append_checkpoint(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
        payload: &RunCheckpointPayload,
    ) -> io::Result<()> {
        fs::create_dir_all(checkpoints_dir(root, run_id))?;
        write_json(&checkpoint_path(root, run_id, payload.seq), payload)
    }

    fn load_record(&self, roots: &[RunStoreRoot], run_id: &str) -> io::Result<Option<(RunStoreRoot, RunRecord)>> {
        for root in roots {
            let path = run_dir(root, run_id).join(RUN_FILE_NAME);
            if path.exists() {
                return Ok(Some((root.clone(), read_run_record(&path)?)));
            }
        }
        Ok(None)
    }

    fn load_latest_checkpoint(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
    ) -> io::Result<Option<RunCheckpointPayload>> {
        let dir = checkpoints_dir(root, run_id);
        if !dir.exists() {
            return Ok(None);
        }
        let mut paths = fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();
        let Some(path) = paths.pop() else {
            return Ok(None);
        };
        let text = fs::read_to_string(&path)?;
        let payload = serde_json::from_str(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("checkpoint {} invalid: {error}", path.display()),
            )
        })?;
        Ok(Some(payload))
    }

    fn list_runs(&self, roots: &[RunStoreRoot], workflow_id: Option<&str>) -> io::Result<Vec<RunSummary>> {
        let mut runs = Vec::new();
        for root in roots {
            if !root.root.exists() {
                continue;
            }
            for entry in fs::read_dir(&root.root)? {
                let entry = entry?;
                let path = entry.path().join(RUN_FILE_NAME);
                if !path.exists() {
                    continue;
                }
                let record = read_run_record(&path)?;
                if workflow_id.is_some_and(|expected| expected != record.workflow_id) {
                    continue;
                }
                runs.push(record.summary());
            }
        }
        runs.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
        Ok(runs)
    }

    fn update_status(
        &self,
        root: &RunStoreRoot,
        run_id: &str,
        status: RunStatus,
        updated_at_ms: i64,
    ) -> io::Result<()> {
        let path = run_dir(root, run_id).join(RUN_FILE_NAME);
        let mut record = read_run_record(&path)?;
        record.status = status;
        record.updated_at_ms = updated_at_ms;
        write_json(&path, &record)
    }

    fn run_dir(&self, root: &RunStoreRoot, run_id: &str) -> PathBuf {
        run_dir(root, run_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::persistence::{RunCheckpointReason, RunCheckpointPayload};
    use crate::run::state::WorkflowRunState;
    use engine::{InteractiveEngineCheckpoint, Workflow};
    use std::collections::{BTreeMap, BTreeSet};

    fn root(base: &Path) -> RunStoreRoot {
        RunStoreRoot {
            project_id: Some("project-1".to_string()),
            root: base.join(".flow").join("runs"),
        }
    }

    fn record(base: &Path, run_id: &str) -> RunRecord {
        RunRecord {
            run_id: run_id.to_string(),
            workflow_id: "wf-1".to_string(),
            workflow_name: "Demo".to_string(),
            workflow_hash: "hash".to_string(),
            project_id: Some("project-1".to_string()),
            execution_cwd: base.display().to_string(),
            artifact_root: base.join(".flow").join("runs").join(run_id).join("artifacts").display().to_string(),
            started_at_ms: 100,
            updated_at_ms: 100,
            status: RunStatus::Running,
        }
    }

    fn checkpoint(seq: u32) -> RunCheckpointPayload {
        let workflow = Workflow::new("wf-1");
        RunCheckpointPayload {
            seq,
            created_at_ms: i64::from(seq),
            reason: RunCheckpointReason::AwaitingInput,
            projection: WorkflowRunState::running_for_workflow(&workflow),
            engine: InteractiveEngineCheckpoint {
                workflow_id: workflow.id,
                layer_idx: 0,
                outputs: BTreeMap::new(),
                changed_files_by_node: BTreeMap::new(),
                transcripts: BTreeMap::new(),
                events: Vec::new(),
                queued_nodes: BTreeSet::new(),
                started_invocations_by_node: BTreeMap::new(),
                awaiting_nodes: BTreeSet::new(),
                pending_tool_batches: BTreeMap::new(),
                retries_by_node: BTreeMap::new(),
                pending_retry_delay_ms: None,
                submit_output_retries_by_node: BTreeMap::new(),
                request_input_retries_by_node: BTreeMap::new(),
                entrypoint_text: None,
                interrupted_nodes: BTreeSet::new(),
                failed_nodes: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn append_and_load_latest_checkpoint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileRunCheckpointStore;
        let root = root(dir.path());
        let record = record(dir.path(), "run-a");

        store.create_run(&root, &record).expect("create run");
        store.append_checkpoint(&root, "run-a", &checkpoint(1)).expect("checkpoint 1");
        store.append_checkpoint(&root, "run-a", &checkpoint(2)).expect("checkpoint 2");
        let loaded = store
            .load_latest_checkpoint(&root, "run-a")
            .expect("load latest")
            .expect("checkpoint");

        assert_eq!(loaded.seq, 2);
    }

    #[test]
    fn list_runs_filters_by_workflow_and_sorts_newest_first() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileRunCheckpointStore;
        let root = root(dir.path());
        let mut older = record(dir.path(), "run-old");
        older.updated_at_ms = 10;
        let mut newer = record(dir.path(), "run-new");
        newer.updated_at_ms = 20;

        store.create_run(&root, &older).expect("old");
        store.create_run(&root, &newer).expect("new");
        let runs = store.list_runs(&[root], Some("wf-1")).expect("list");

        assert_eq!(runs.iter().map(|run| run.run_id.as_str()).collect::<Vec<_>>(), vec!["run-new", "run-old"]);
    }

    #[test]
    fn update_status_rewrites_run_record() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileRunCheckpointStore;
        let root = root(dir.path());
        store.create_run(&root, &record(dir.path(), "run-a")).expect("create run");

        store.update_status(&root, "run-a", RunStatus::Completed, 300).expect("update");
        let (_, loaded) = store.load_record(&[root], "run-a").expect("load").expect("record");

        assert_eq!(loaded.status, RunStatus::Completed);
        assert_eq!(loaded.updated_at_ms, 300);
    }
}
```

- [ ] **Step 2: Export the adapter**

Modify `crates/orchestration/src/adapters/storage/mod.rs`:

```rust
pub mod agent_store;
pub mod app_workflow_store;
pub mod incident_store;
pub mod json_file_store;
pub mod project_store;
pub mod project_workflow_store;
pub mod run_checkpoint_store;
pub mod settings_store;
pub mod skill_store;
pub mod template_store;

#[cfg(test)]
mod incident_store_tests;
```

- [ ] **Step 3: Run the store tests**

Run:

```bash
cargo test -p orchestration adapters::storage::run_checkpoint_store -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/orchestration/src/adapters/storage/mod.rs crates/orchestration/src/adapters/storage/run_checkpoint_store.rs
git commit -m "feat: add file run checkpoint store"
```

## Task 3: Wire Run Roots And Create Durable Run Records

**Files:**
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Modify: `crates/orchestration/src/run/execution/mod.rs`
- Modify: `crates/orchestration/src/run/execution/drive.rs`
- Test: `crates/orchestration/src/run/coordinator_tests.rs`

- [ ] **Step 1: Write failing coordinator test for stable artifact root**

Add to `crates/orchestration/src/run/coordinator_tests.rs`:

```rust
#[test]
fn durable_artifact_root_lives_under_run_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = crate::run::persistence::RunStoreRoot {
        project_id: Some("project-1".to_string()),
        root: dir.path().join(".flow").join("runs"),
    };
    let store = crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
    let artifact_root = store.run_dir(&root, "run-1").join("artifacts");

    assert_eq!(artifact_root, dir.path().join(".flow").join("runs").join("run-1").join("artifacts"));
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p orchestration durable_artifact_root_lives_under_run_directory -- --nocapture
```

Expected: FAIL with `no method named run_dir found` because the test needs `use crate::run::ports::RunCheckpointStore;`.

- [ ] **Step 3: Extend run params and session state**

In `crates/orchestration/src/run/coordinator.rs`, add imports:

```rust
use crate::run::persistence::{workflow_hash, RunRecord, RunStatus, RunStoreRoot};
use crate::run::ports::RunCheckpointStore;
use chrono::Utc;
```

Change `RunSession`:

```rust
struct RunSession {
    workflow: Option<Workflow>,
    run_state: Option<WorkflowRunState>,
    run_id: Option<String>,
    run_root: Option<RunStoreRoot>,
    project_id: Option<String>,
    execution_cwd: Option<PathBuf>,
    entrypoint: Option<String>,
    artifact_root: Option<PathBuf>,
    engine_checkpoint: Option<InteractiveEngineCheckpoint>,
    checkpoint_sink: Option<Arc<ParkingMutex<Option<crate::run::persistence::PendingRunCheckpoint>>>>,
    snapshot_store: Option<Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>>,
    lsp_settings: Option<crate::lsp::LspSettings>,
    pending_engine_reverts: Option<Arc<parking_lot::Mutex<Vec<engine::EditBatch>>>>,
    action_tx: Option<UnboundedSender<ExecutionAction>>,
    handle: Option<tokio::task::JoinHandle<()>>,
    cancel_token: Option<CancellationToken>,
    node_interrupts: Option<NodeInterrupts>,
}
```

Change `RunStartParams`:

```rust
pub struct RunStartParams<'a> {
    pub workflow: Workflow,
    pub entrypoint: Option<String>,
    pub execution_cwd: Option<String>,
    pub run_root: RunStoreRoot,
    pub settings: &'a AppSettings,
    pub transient_api_key: Option<&'a str>,
    pub agent_store: &'a dyn crate::agent::ports::AgentStore,
    pub settings_store: &'a dyn crate::settings::ports::SettingsStore,
    pub run_store: &'a dyn RunCheckpointStore,
    pub env: &'a ProviderEnv,
}
```

- [ ] **Step 4: Create run record before spawning drive**

In `RunCoordinator::start_run`, create the run id before artifact root selection and persist the record:

```rust
let run_id = Uuid::new_v4().to_string();
let artifact_root = params.run_store.run_dir(&params.run_root, &run_id).join("artifacts");
let now_ms = Utc::now().timestamp_millis();
let run_record = RunRecord {
    run_id: run_id.clone(),
    workflow_id: workflow.id.to_string(),
    workflow_name: workflow.name.clone(),
    workflow_hash: workflow_hash(&workflow),
    project_id: params.run_root.project_id.clone(),
    execution_cwd: resolved_cwd.display().to_string(),
    artifact_root: artifact_root.display().to_string(),
    started_at_ms: now_ms,
    updated_at_ms: now_ms,
    status: RunStatus::Running,
};
params.run_store.create_run(&params.run_root, &run_record)?;
```

Then replace the existing `let artifact_root = new_artifact_root();` with the durable `artifact_root` above, set `session.run_id = Some(run_id.clone())`, and set `session.run_root = Some(params.run_root.clone())`.

- [ ] **Step 5: Remove temp artifact cleanup for durable runs**

Change `finish_run_session`:

```rust
fn finish_run_session(session: &mut RunSession) {
    session.snapshot_store = None;
    session.lsp_settings = None;
    session.pending_engine_reverts = None;
    session.action_tx = None;
    session.handle = None;
    session.cancel_token = None;
    session.node_interrupts = None;
    session.checkpoint_sink = None;
    session.engine_checkpoint = None;
}
```

Keep `clear_artifact_root` only for tests or delete it if no callers remain. Durable artifacts must remain available for replay.

- [ ] **Step 6: Update backend wiring**

In `crates/orchestration/src/backend/mod.rs`, import the store and port:

```rust
use crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
use crate::run::persistence::RunStoreRoot;
use crate::run::ports::RunCheckpointStore;
```

Add `run_store` to `AppBackend`:

```rust
runs: RunCoordinator,
run_store: Box<dyn RunCheckpointStore>,
```

Initialize it in `AppBackend::new`:

```rust
run_store: Box::new(FileRunCheckpointStore),
```

Add helper methods:

```rust
fn run_roots(&self) -> Result<Vec<RunStoreRoot>, BackendError> {
    let mut roots = vec![RunStoreRoot {
        project_id: None,
        root: FileRunCheckpointStore::app_runs_root(),
    }];
    for project in self.projects.load()? {
        roots.push(RunStoreRoot {
            project_id: Some(project.id),
            root: std::path::Path::new(&project.path).join(".flow").join("runs"),
        });
    }
    Ok(roots)
}

fn run_root_for_workflow(&self, workflow_id: &str) -> Result<RunStoreRoot, BackendError> {
    for project in self.projects.load()? {
        if project.workflow_ids.iter().any(|id| id == workflow_id) {
            return Ok(RunStoreRoot {
                project_id: Some(project.id),
                root: std::path::Path::new(&project.path).join(".flow").join("runs"),
            });
        }
    }
    Ok(RunStoreRoot {
        project_id: None,
        root: FileRunCheckpointStore::app_runs_root(),
    })
}
```

In `AppBackend::start_run`, compute the root and pass the store:

```rust
let run_root = self.run_root_for_workflow(&workflow.id)?;
self.runs
    .start_run(RunStartParams {
        workflow,
        entrypoint,
        execution_cwd,
        run_root,
        settings,
        transient_api_key,
        agent_store: self.agents.store(),
        settings_store: self.settings.store(),
        run_store: self.run_store.as_ref(),
        env: self.settings.env(),
    })
    .await
    .map_err(|error| self.backend_err(error))
```

- [ ] **Step 7: Update execution params checkpoint sink type**

In `crates/orchestration/src/run/execution/mod.rs`, import `PendingRunCheckpoint`:

```rust
use crate::run::persistence::PendingRunCheckpoint;
```

Change the field:

```rust
pub checkpoint_sink: Arc<Mutex<Option<PendingRunCheckpoint>>>,
```

Update all `checkpoint_sink` construction sites to use `Arc<ParkingMutex<Option<PendingRunCheckpoint>>>`.

- [ ] **Step 8: Run targeted tests**

Run:

```bash
cargo test -p orchestration durable_artifact_root_lives_under_run_directory -- --nocapture
cargo test -p orchestration run::coordinator_tests -- --nocapture
```

Expected: PASS after fixing imports and changed session seed fields.

- [ ] **Step 9: Commit**

```bash
git add crates/orchestration/src/backend/mod.rs crates/orchestration/src/run/coordinator.rs crates/orchestration/src/run/execution/mod.rs crates/orchestration/src/run/execution/drive.rs crates/orchestration/src/run/coordinator_tests.rs
git commit -m "feat: create durable run records"
```

## Task 4: Append Checkpoints From Engine And Projection

**Files:**
- Modify: `crates/orchestration/src/run/execution/drive.rs`
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Test: `crates/orchestration/src/run/execution/tests.rs`
- Test: `crates/orchestration/src/run/coordinator_tests.rs`

- [ ] **Step 1: Write failing unit test for pending checkpoint reason**

Add to `crates/orchestration/src/run/execution/tests.rs`:

```rust
#[test]
fn pending_checkpoint_records_pause_reason() {
    let workflow = engine::Workflow::new("checkpoint-test");
    let mut engine = engine::InteractiveEngine::new(workflow, None).expect("engine");
    let checkpoint = crate::run::persistence::PendingRunCheckpoint {
        reason: crate::run::persistence::RunCheckpointReason::AwaitingInput,
        engine: engine.prepare_stop_checkpoint(),
    };

    assert_eq!(
        checkpoint.reason,
        crate::run::persistence::RunCheckpointReason::AwaitingInput
    );
}
```

- [ ] **Step 2: Add checkpoint helper in `drive.rs`**

Add imports:

```rust
use crate::run::persistence::{PendingRunCheckpoint, RunCheckpointReason};
```

Replace `snapshot_and_abort` with:

```rust
fn publish_checkpoint(
    engine: &mut InteractiveEngine,
    checkpoint_sink: &Arc<Mutex<Option<PendingRunCheckpoint>>>,
    reason: RunCheckpointReason,
) {
    *checkpoint_sink.lock() = Some(PendingRunCheckpoint {
        reason,
        engine: engine.prepare_stop_checkpoint(),
    });
}

fn snapshot_and_abort(
    engine: &mut InteractiveEngine,
    checkpoint_sink: &Arc<Mutex<Option<PendingRunCheckpoint>>>,
    event_tx: &UnboundedSender<ExecutionEvent>,
    aborted_emitted: &mut bool,
) {
    publish_checkpoint(engine, checkpoint_sink, RunCheckpointReason::UserStopped);
    abort_run(event_tx, aborted_emitted);
}
```

- [ ] **Step 3: Publish checkpoints on interaction and terminal results**

Inside `EngineRunResult::NeedsInteraction`, after all pause events are emitted and before the `while` loop:

```rust
let reason = if !approvals.is_empty() {
    RunCheckpointReason::AwaitingToolApproval
} else if !retryables.is_empty() {
    RunCheckpointReason::AwaitingRetry
} else {
    RunCheckpointReason::AwaitingInput
};
publish_checkpoint(&mut engine, &checkpoint_sink, reason);
```

Inside `EngineRunResult::Completed(report)`:

```rust
publish_checkpoint(&mut engine, &checkpoint_sink, RunCheckpointReason::Completed);
send_or_log(&event_tx, ExecutionEvent::Finished(report));
return;
```

Inside `EngineRunResult::Failed(error)` before emitting any failure event:

```rust
publish_checkpoint(&mut engine, &checkpoint_sink, RunCheckpointReason::Failed);
```

- [ ] **Step 4: Add coordinator checkpoint drain helper**

In `crates/orchestration/src/run/coordinator.rs`, add imports:

```rust
use crate::run::persistence::{PendingRunCheckpoint, RunCheckpointPayload, RunCheckpointReason};
```

Add helper functions:

```rust
fn status_for_checkpoint(reason: RunCheckpointReason) -> RunStatus {
    match reason {
        RunCheckpointReason::AwaitingInput
        | RunCheckpointReason::AwaitingToolApproval
        | RunCheckpointReason::AwaitingRetry => RunStatus::Paused,
        RunCheckpointReason::UserStopped => RunStatus::Stopped,
        RunCheckpointReason::Completed => RunStatus::Completed,
        RunCheckpointReason::Failed => RunStatus::Failed,
    }
}

fn next_checkpoint_seq(store: &dyn RunCheckpointStore, root: &RunStoreRoot, run_id: &str) -> Result<u32, BackendError> {
    Ok(store
        .load_latest_checkpoint(root, run_id)?
        .map_or(1, |payload| payload.seq.saturating_add(1)))
}
```

Add method:

```rust
fn persist_pending_checkpoint(
    run_store: &dyn RunCheckpointStore,
    root: &RunStoreRoot,
    run_id: &str,
    projection: &WorkflowRunState,
    pending: PendingRunCheckpoint,
) -> Result<(), BackendError> {
    let now_ms = Utc::now().timestamp_millis();
    let payload = RunCheckpointPayload {
        seq: next_checkpoint_seq(run_store, root, run_id)?,
        created_at_ms: now_ms,
        reason: pending.reason,
        engine: pending.engine,
        projection: projection.clone(),
    };
    run_store.append_checkpoint(root, run_id, &payload)?;
    run_store.update_status(root, run_id, status_for_checkpoint(pending.reason), now_ms)?;
    Ok(())
}
```

- [ ] **Step 5: Drain checkpoint after applying each event**

Change `RunCoordinator::apply_execution_event` signature:

```rust
pub async fn apply_execution_event(
    &self,
    event: ExecutionEvent,
    run_store: &dyn RunCheckpointStore,
) -> Result<WorkflowRunState, BackendError>
```

After `let snapshot = run_state.clone();`, drain the sink:

```rust
let pending_checkpoint = session
    .checkpoint_sink
    .as_ref()
    .and_then(|sink| sink.lock().take());
if let (Some(root), Some(run_id), Some(pending)) = (
    session.run_root.clone(),
    session.run_id.clone(),
    pending_checkpoint,
) {
    persist_pending_checkpoint(run_store, &root, &run_id, &snapshot, pending)?;
}
```

Update `AppBackend::apply_execution_event`:

```rust
self.runs
    .apply_execution_event(event, self.run_store.as_ref())
    .await
```

- [ ] **Step 6: Run checkpoint tests**

Run:

```bash
cargo test -p orchestration pending_checkpoint_records_pause_reason -- --nocapture
cargo test -p orchestration run::coordinator_tests -- --nocapture
cargo test -p orchestration execution:: -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/orchestration/src/run/execution/drive.rs crates/orchestration/src/run/execution/tests.rs crates/orchestration/src/run/coordinator.rs crates/orchestration/src/run/coordinator_tests.rs crates/orchestration/src/backend/mod.rs
git commit -m "feat: append durable run checkpoints"
```

## Task 5: Read-Only Replay And Run Listing Backend

**Files:**
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/orchestration/src/error.rs`
- Test: `crates/orchestration/src/run/coordinator_tests.rs`

- [ ] **Step 1: Add errors**

In `crates/orchestration/src/error.rs`, add variants:

```rust
#[error("run {0} not found")]
RunNotFound(String),
#[error("run {0} has no checkpoints")]
RunHasNoCheckpoints(String),
#[error("run {0} cannot be resumed because workflow {1} changed")]
RunWorkflowChanged(String, String),
```

- [ ] **Step 2: Add coordinator replay helpers**

In `RunCoordinator`, add:

```rust
pub fn list_runs(
    &self,
    run_store: &dyn RunCheckpointStore,
    roots: &[RunStoreRoot],
    workflow_id: Option<&str>,
) -> Result<Vec<crate::run::persistence::RunSummary>, BackendError> {
    Ok(run_store.list_runs(roots, workflow_id)?)
}

pub fn replay_run(
    &self,
    run_store: &dyn RunCheckpointStore,
    roots: &[RunStoreRoot],
    run_id: &str,
) -> Result<WorkflowRunState, BackendError> {
    let (root, _) = run_store
        .load_record(roots, run_id)?
        .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
    let mut checkpoint = run_store
        .load_latest_checkpoint(&root, run_id)?
        .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?;
    checkpoint.projection.active = false;
    checkpoint.projection.pending_approvals.clear();
    checkpoint.projection.awaiting_node_id = None;
    checkpoint.projection.awaiting_node_ids.clear();
    checkpoint.projection.active_manual_node_id = None;
    checkpoint.projection.active_tool_call_id = None;
    Ok(checkpoint.projection)
}
```

- [ ] **Step 3: Add backend methods**

In `AppBackend`, add:

```rust
pub fn list_runs(&self, workflow_id: Option<&str>) -> Result<Vec<crate::run::persistence::RunSummary>, BackendError> {
    let roots = self.run_roots()?;
    self.runs.list_runs(self.run_store.as_ref(), &roots, workflow_id)
}

pub fn replay_run(&self, run_id: &str) -> Result<WorkflowRunState, BackendError> {
    let roots = self.run_roots()?;
    self.runs.replay_run(self.run_store.as_ref(), &roots, run_id)
}
```

- [ ] **Step 4: Write replay test**

Add to `crates/orchestration/src/run/coordinator_tests.rs`:

```rust
#[test]
fn replay_run_returns_inactive_projection_without_pending_actions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
    let root = crate::run::persistence::RunStoreRoot {
        project_id: None,
        root: dir.path().join("runs"),
    };
    let workflow = Workflow::new("Replay");
    let mut projection = WorkflowRunState::running_for_workflow(&workflow);
    projection.run_id = Some("run-1".to_string());
    projection.awaiting_node_id = Some(NodeId("node-1".to_string()));
    projection.awaiting_node_ids.push(NodeId("node-1".to_string()));
    let record = crate::run::persistence::RunRecord {
        run_id: "run-1".to_string(),
        workflow_id: workflow.id.to_string(),
        workflow_name: workflow.name.clone(),
        workflow_hash: crate::run::persistence::workflow_hash(&workflow),
        project_id: None,
        execution_cwd: dir.path().display().to_string(),
        artifact_root: dir.path().join("runs/run-1/artifacts").display().to_string(),
        started_at_ms: 1,
        updated_at_ms: 1,
        status: crate::run::persistence::RunStatus::Paused,
    };
    store.create_run(&root, &record).expect("create run");
    store.append_checkpoint(
        &root,
        "run-1",
        &crate::run::persistence::RunCheckpointPayload {
            seq: 1,
            created_at_ms: 1,
            reason: crate::run::persistence::RunCheckpointReason::AwaitingInput,
            engine: InteractiveEngineCheckpoint {
                workflow_id: workflow.id.clone(),
                layer_idx: 0,
                outputs: Default::default(),
                changed_files_by_node: Default::default(),
                transcripts: Default::default(),
                events: Vec::new(),
                queued_nodes: Default::default(),
                started_invocations_by_node: Default::default(),
                awaiting_nodes: Default::default(),
                pending_tool_batches: Default::default(),
                retries_by_node: Default::default(),
                pending_retry_delay_ms: None,
                submit_output_retries_by_node: Default::default(),
                request_input_retries_by_node: Default::default(),
                entrypoint_text: None,
                interrupted_nodes: Default::default(),
                failed_nodes: Default::default(),
            },
            projection,
        },
    ).expect("checkpoint");
    let coordinator = RunCoordinator::new(tokio::runtime::Handle::current(), Arc::new(IncidentRecorder::new(Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl"))))));

    let replay = coordinator
        .replay_run(&store, &[root], "run-1")
        .expect("replay run");

    assert!(!replay.active);
    assert!(replay.awaiting_node_id.is_none());
    assert!(replay.awaiting_node_ids.is_empty());
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p orchestration replay_run_returns_inactive_projection_without_pending_actions -- --nocapture
cargo test -p orchestration run::coordinator_tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/orchestration/src/error.rs crates/orchestration/src/run/coordinator.rs crates/orchestration/src/backend/mod.rs crates/orchestration/src/run/coordinator_tests.rs
git commit -m "feat: list and replay durable runs"
```

## Task 6: Durable Resume After Restart

**Files:**
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`
- Test: `crates/orchestration/src/run/coordinator_tests.rs`

- [ ] **Step 1: Add durable resume params**

In `crates/orchestration/src/run/coordinator.rs`, add:

```rust
pub struct DurableResumeParams<'a> {
    pub run_id: &'a str,
    pub workflow: Workflow,
    pub root: RunStoreRoot,
    pub record: RunRecord,
    pub checkpoint: crate::run::persistence::RunCheckpointPayload,
    pub settings: &'a AppSettings,
    pub transient_api_key: Option<&'a str>,
    pub agent_store: &'a dyn crate::agent::ports::AgentStore,
    pub settings_store: &'a dyn crate::settings::ports::SettingsStore,
    pub run_store: &'a dyn RunCheckpointStore,
    pub env: &'a ProviderEnv,
}
```

- [ ] **Step 2: Implement `resume_durable_run`**

Add method to `RunCoordinator`:

```rust
pub async fn resume_durable_run(
    &self,
    params: DurableResumeParams<'_>,
) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
    if workflow_hash(&params.workflow) != params.record.workflow_hash {
        return Err(BackendError::RunWorkflowChanged(
            params.run_id.to_string(),
            params.workflow.id.to_string(),
        ));
    }
    engine::validate_checkpoint_against_workflow(&params.workflow, &params.checkpoint.engine)
        .map_err(|error| BackendError::CheckpointIncompatible(error.to_string()))?;

    let persisted_settings = params.settings_store.load()?;
    let mut provider_settings = params.settings.clone();
    merge_preserved_api_keys(&mut provider_settings, &persisted_settings);
    if let Some(provider_id) = params
        .workflow
        .settings
        .provider_id
        .as_ref()
        .filter(|provider_id| !provider_id.trim().is_empty())
    {
        provider_settings.active_provider = ProviderId::from(provider_id.as_str());
    }
    let provider_config = resolve_provider_config(&provider_settings, params.transient_api_key, params.env)?;
    let ai = create_provider(provider_config);

    let mut workflow = params.workflow;
    apply_reasoning_defaults(&mut workflow, provider_settings.active_profile());
    let agents = params.agent_store.load()?;
    let agent_snapshots = resolve_callable_agent_snapshots(&workflow, &agents);

    self.terminate_active_run(TerminationMode::Replaced).await;

    let snapshot_store = Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new());
    let lsp_settings = crate::lsp::LspSettings::from_persisted(&persisted_settings.lsp);
    let pending_engine_reverts = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let node_interrupts: NodeInterrupts = Arc::new(parking_lot::Mutex::new(std::collections::BTreeMap::new()));
    let checkpoint_sink = Arc::new(ParkingMutex::new(None));
    let artifact_root = PathBuf::from(&params.record.artifact_root);
    let execution_cwd = PathBuf::from(&params.record.execution_cwd);
    let (handle, event_rx, action_tx, cancel_token, _) = spawn_interactive_workflow_run(
        &self.runtime_handle,
        InteractiveWorkflowRunParams {
            workflow: workflow.clone(),
            entrypoint: None,
            execution_cwd: execution_cwd.clone(),
            artifact_root: artifact_root.clone(),
            resume_checkpoint: Some(params.checkpoint.engine),
            checkpoint_sink: checkpoint_sink.clone(),
            ai,
            agent_snapshots,
            snapshot_store: snapshot_store.clone(),
            lsp: lsp_settings.clone(),
            pending_engine_reverts: pending_engine_reverts.clone(),
            node_interrupts: node_interrupts.clone(),
        },
    );

    let mut resumed_state = params.checkpoint.projection;
    resumed_state.active = true;
    resumed_state.run_id = Some(params.run_id.to_string());

    let mut session = self.session.lock().await;
    session.workflow = Some(workflow);
    session.run_state = Some(resumed_state.clone());
    session.run_id = Some(params.run_id.to_string());
    session.run_root = Some(params.root);
    session.project_id = params.record.project_id;
    session.execution_cwd = Some(execution_cwd);
    session.entrypoint = None;
    session.artifact_root = Some(artifact_root);
    session.engine_checkpoint = None;
    session.checkpoint_sink = Some(checkpoint_sink);
    session.snapshot_store = Some(snapshot_store);
    session.lsp_settings = Some(lsp_settings);
    session.pending_engine_reverts = Some(pending_engine_reverts);
    session.action_tx = Some(action_tx);
    session.handle = Some(handle);
    session.cancel_token = Some(cancel_token);
    session.node_interrupts = Some(node_interrupts);
    params
        .run_store
        .update_status(session.run_root.as_ref().expect("run root"), params.run_id, RunStatus::Running, Utc::now().timestamp_millis())?;
    Ok((resumed_state, event_rx))
}
```

- [ ] **Step 3: Add backend durable resume method**

In `AppBackend`, add:

```rust
pub async fn resume_durable_run(
    &self,
    run_id: &str,
    settings: &AppSettings,
    transient_api_key: Option<&str>,
) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
    let roots = self.run_roots()?;
    let (root, record) = self
        .run_store
        .load_record(&roots, run_id)?
        .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
    let checkpoint = self
        .run_store
        .load_latest_checkpoint(&root, run_id)?
        .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?;
    let workflow = self.load_workflow(&record.workflow_id)?;
    self.runs
        .resume_durable_run(crate::run::coordinator::DurableResumeParams {
            run_id,
            workflow,
            root,
            record,
            checkpoint,
            settings,
            transient_api_key,
            agent_store: self.agents.store(),
            settings_store: self.settings.store(),
            run_store: self.run_store.as_ref(),
            env: self.settings.env(),
        })
        .await
        .map_err(|error| self.backend_err(error))
}
```

- [ ] **Step 4: Write stale-workflow guard test**

Add to `crates/orchestration/src/run/coordinator_tests.rs`:

```rust
#[test]
fn workflow_hash_detects_changed_workflow_for_resume_guard() {
    let mut workflow = Workflow::new("Resume");
    let original = crate::run::persistence::workflow_hash(&workflow);
    workflow.name = "Changed".to_string();
    let changed = crate::run::persistence::workflow_hash(&workflow);

    assert_ne!(original, changed);
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p orchestration workflow_hash_detects_changed_workflow_for_resume_guard -- --nocapture
cargo test -p orchestration run::coordinator_tests -- --nocapture
cargo test -p orchestration --test workflow_acceptance -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/orchestration/src/run/coordinator.rs crates/orchestration/src/backend/mod.rs crates/orchestration/src/run/coordinator_tests.rs
git commit -m "feat: resume durable runs after restart"
```

## Task 7: Desktop IPC Commands

**Files:**
- Modify: `crates/desktop/src/lib.rs`
- Test: `cargo test -p desktop`

- [ ] **Step 1: Add Tauri commands**

Add command functions near the current run commands in `crates/desktop/src/lib.rs`:

```rust
#[tauri::command]
fn list_runs(
    backend: tauri::State<'_, AppBackend>,
    workflow_id: Option<String>,
) -> Result<Vec<orchestration::run::persistence::RunSummary>, CommandError> {
    Ok(backend.list_runs(workflow_id.as_deref())?)
}

#[tauri::command]
fn replay_run(
    backend: tauri::State<'_, AppBackend>,
    run_id: String,
) -> Result<WorkflowRunState, CommandError> {
    Ok(backend.replay_run(&run_id)?)
}

#[tauri::command]
async fn resume_durable_run(
    backend: tauri::State<'_, AppBackend>,
    app: tauri::AppHandle,
    run_id: String,
    settings: AppSettings,
    transient_api_key: Option<String>,
) -> Result<WorkflowRunState, CommandError> {
    let (initial_state, event_rx) = backend
        .resume_durable_run(&run_id, &settings, transient_api_key.as_deref())
        .await?;
    spawn_run_event_bridge(app, event_rx, initial_state.run_id.clone());
    Ok(initial_state)
}
```

- [ ] **Step 2: Register commands**

Add these to `tauri::generate_handler![...]`:

```rust
list_runs,
replay_run,
resume_durable_run,
```

- [ ] **Step 3: Run desktop tests**

Run:

```bash
cargo test -p desktop
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/lib.rs
git commit -m "feat: expose durable run IPC"
```

## Task 8: UI Types, Port, And API Wrappers

**Files:**
- Modify: `crates/ui/src/lib/types.ts`
- Modify: `crates/ui/src/api.ts`
- Modify: `crates/ui/src/port.ts`
- Test: `crates/ui/src/api.test.ts`

- [ ] **Step 1: Add DTO types**

In `crates/ui/src/lib/types.ts`, add:

```typescript
export type DurableRunStatus = "running" | "paused" | "stopped" | "completed" | "failed";

export interface RunSummary {
  runId: string;
  workflowId: string;
  workflowName: string;
  projectId: string | null;
  startedAtMs: number;
  updatedAtMs: number;
  status: DurableRunStatus;
}
```

Also add `runId` to `WorkflowRunState` if it is missing:

```typescript
runId?: string | null;
```

- [ ] **Step 2: Add API functions**

In `crates/ui/src/api.ts`, import `RunSummary` and add:

```typescript
export function listRuns(workflowId: string | null = null) {
  return invoke<RunSummary[]>("list_runs", { workflowId });
}

export function replayRun(runId: string) {
  return invoke<WorkflowRunState>("replay_run", { runId });
}

export function resumeDurableRun(
  runId: string,
  settings: AppSettings,
  transientApiKey: string | null = null,
) {
  return invoke<WorkflowRunState>("resume_durable_run", {
    runId,
    settings,
    transientApiKey,
  });
}
```

- [ ] **Step 3: Add port methods**

In `crates/ui/src/port.ts`, import `RunSummary` and add methods to `UiDesktopOutboundPort`:

```typescript
listRuns: (workflowId?: string | null) => Promise<RunSummary[]>;
replayRun: (runId: string) => Promise<WorkflowRunState>;
resumeDurableRun: (
  runId: string,
  settings: AppSettings,
  transientApiKey?: string | null,
) => Promise<WorkflowRunState>;
```

Add adapter entries:

```typescript
listRuns: desktopApi.listRuns,
replayRun: desktopApi.replayRun,
resumeDurableRun: desktopApi.resumeDurableRun,
```

- [ ] **Step 4: Add API test**

In `crates/ui/src/api.test.ts`, add:

```typescript
import { describe, expect, it, vi } from "vitest";
import { listRuns, replayRun, resumeDurableRun } from "./api";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("durable run API", () => {
  it("passes workflowId to list_runs", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    await listRuns("wf-1");
    expect(invoke).toHaveBeenCalledWith("list_runs", { workflowId: "wf-1" });
  });

  it("passes runId to replay_run", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ active: false });
    await replayRun("run-1");
    expect(invoke).toHaveBeenCalledWith("replay_run", { runId: "run-1" });
  });

  it("passes settings to resume_durable_run", async () => {
    const settings = { active_provider: "openai" };
    vi.mocked(invoke).mockResolvedValueOnce({ active: true });
    await resumeDurableRun("run-1", settings as never, "key");
    expect(invoke).toHaveBeenCalledWith("resume_durable_run", {
      runId: "run-1",
      settings,
      transientApiKey: "key",
    });
  });
});
```

Merge these cases into the existing `describe("api desktop seam", ...)` block in `crates/ui/src/api.test.ts`, and extend the existing import from `./api` with `listRuns`, `replayRun`, and `resumeDurableRun`.

- [ ] **Step 5: Run UI tests**

Run:

```bash
npm --prefix crates/ui run test -- src/api.test.ts
npm --prefix crates/ui run typecheck
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/lib/types.ts crates/ui/src/api.ts crates/ui/src/port.ts crates/ui/src/api.test.ts
git commit -m "feat: add durable run UI API"
```

## Task 9: Run History UI And Replay State

**Files:**
- Modify: `crates/ui/src/context/AppContext.tsx`
- Modify: `crates/ui/src/context/AppProvider.tsx`
- Create: `crates/ui/src/panels/RunHistoryPanel.tsx`
- Modify: `crates/ui/src/panels/DockPanel.tsx`
- Modify: `crates/ui/src/lib/types.ts`
- Modify: `crates/ui/src/styles/index.css`
- Test: `crates/ui/src/context/AppProvider.test.tsx` if present, otherwise `npm --prefix crates/ui run typecheck`

- [ ] **Step 1: Extend bottom tab type**

In `crates/ui/src/lib/types.ts`, change:

```typescript
export type BottomTab = "overview" | "chat" | "trace" | "terminal" | "runs";
```

- [ ] **Step 2: Extend context interface**

In `crates/ui/src/context/AppContext.tsx`, import `RunSummary` and add:

```typescript
runHistory: Accessor<RunSummary[]>;
runHistoryLoading: Accessor<boolean>;
replayRunId: Accessor<string | null>;
handleRefreshRunHistory: () => Promise<void>;
handleReplayRun: (runId: string) => Promise<void>;
handleResumeDurableRun: (runId: string) => Promise<void>;
```

- [ ] **Step 3: Implement AppProvider state/actions**

In `crates/ui/src/context/AppProvider.tsx`, add signals:

```typescript
const [runHistory, setRunHistory] = createSignal<RunSummary[]>([]);
const [runHistoryLoading, setRunHistoryLoading] = createSignal(false);
const [replayRunId, setReplayRunId] = createSignal<string | null>(null);
```

Add actions:

```typescript
const handleRefreshRunHistory = async () => {
  const workflow = selectedWorkflow();
  if (!workflow) {
    setRunHistory([]);
    return;
  }
  setRunHistoryLoading(true);
  try {
    setRunHistory(await desktop.listRuns(workflow.id));
  } catch (error) {
    toast.error(formatError(error));
  } finally {
    setRunHistoryLoading(false);
  }
};

const handleReplayRun = async (runId: string) => {
  try {
    const replay = await desktop.replayRun(runId);
    setReplayRunId(runId);
    publishBackendRunState({ ...replay, active: false });
    setBottomTab("chat");
    setDockHeight((current) => clampDockHeight(current, "chat"));
  } catch (error) {
    toast.error(formatError(error));
  }
};

const handleResumeDurableRun = async (runId: string) => {
  const workflow = selectedWorkflow();
  if (!workflow || !applySchemaEditor()) return;
  try {
    const nextRunState = await desktop.resumeDurableRun(runId, settings(), transientApiKey());
    setReplayRunId(null);
    beginRunSession(nextRunState);
    await handleRefreshRunHistory();
  } catch (error) {
    toast.error(formatError(error));
  }
};
```

In the existing run-state listener path, clear replay mode when live events arrive:

```typescript
setReplayRunId(null);
```

Add these values to the provided context object:

```typescript
runHistory,
runHistoryLoading,
replayRunId,
handleRefreshRunHistory,
handleReplayRun,
handleResumeDurableRun,
```

- [ ] **Step 4: Create run history panel**

Create `crates/ui/src/panels/RunHistoryPanel.tsx`:

```tsx
import { createEffect, For, Show } from "solid-js";
import Play from "lucide-solid/icons/play";
import RotateCcw from "lucide-solid/icons/rotate-ccw";
import RefreshCw from "lucide-solid/icons/refresh-cw";
import { useAppContext } from "../context/AppContext";
import type { RunSummary } from "../lib/types";

function formatRunTime(ms: number) {
  return new Date(ms).toLocaleString();
}

function canResume(run: RunSummary) {
  return run.status === "paused" || run.status === "stopped" || run.status === "failed";
}

export function RunHistoryPanel() {
  const ctx = useAppContext();

  createEffect(() => {
    if (ctx.bottomTab() === "runs") {
      void ctx.handleRefreshRunHistory();
    }
  });

  return (
    <div class="run-history-panel">
      <header class="run-history-header">
        <div>
          <div class="eyebrow">Runs</div>
          <h3>History</h3>
        </div>
        <button
          type="button"
          class="dock-icon-action"
          title="Refresh runs"
          aria-label="Refresh runs"
          onClick={() => void ctx.handleRefreshRunHistory()}
        >
          <RefreshCw width={15} height={15} />
        </button>
      </header>

      <Show
        when={!ctx.runHistoryLoading()}
        fallback={<div class="empty-panel">Loading runs.</div>}
      >
        <Show
          when={ctx.runHistory().length > 0}
          fallback={<div class="empty-panel">No saved runs for this workflow.</div>}
        >
          <div class="run-history-list">
            <For each={ctx.runHistory()}>
              {(run) => (
                <div class="run-history-row" classList={{ active: ctx.replayRunId() === run.runId }}>
                  <div class="run-history-main">
                    <span class={`run-history-status status-${run.status}`}>{run.status}</span>
                    <strong>{run.workflowName}</strong>
                    <span>{formatRunTime(run.updatedAtMs)}</span>
                  </div>
                  <div class="run-history-actions">
                    <button
                      type="button"
                      class="dock-icon-action"
                      title="Open replay"
                      aria-label="Open replay"
                      onClick={() => void ctx.handleReplayRun(run.runId)}
                    >
                      <Play width={15} height={15} />
                    </button>
                    <Show when={canResume(run)}>
                      <button
                        type="button"
                        class="dock-icon-action"
                        title="Resume run"
                        aria-label="Resume run"
                        onClick={() => void ctx.handleResumeDurableRun(run.runId)}
                      >
                        <RotateCcw width={15} height={15} />
                      </button>
                    </Show>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}
```

- [ ] **Step 5: Add Dock tab**

In `crates/ui/src/panels/DockPanel.tsx`, import:

```typescript
import { RunHistoryPanel } from "./RunHistoryPanel";
```

Add a button in `.dock-tab-switcher`:

```tsx
<button
  classList={{ active: ctx.bottomTab() === "runs" }}
  onClick={() => ctx.handleSelectBottomTab("runs")}
>
  Runs
</button>
```

Change `TerminalOrTrace` to route the runs tab:

```tsx
function TerminalOrTrace() {
  const ctx = useAppContext();

  return (
    <Show when={ctx.bottomTab() === "terminal"} fallback={<TraceOrRuns />}>
      <TerminalPanel />
    </Show>
  );
}

function TraceOrRuns() {
  const ctx = useAppContext();

  return (
    <Show when={ctx.bottomTab() === "runs"} fallback={<TracePanel />}>
      <RunHistoryPanel />
    </Show>
  );
}
```

- [ ] **Step 6: Add CSS**

In `crates/ui/src/styles/index.css`, add:

```css
.run-history-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.run-history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}

.run-history-header h3 {
  margin: 2px 0 0;
  font-size: 14px;
  font-weight: 650;
}

.run-history-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: auto;
}

.run-history-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  min-height: 52px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--border);
}

.run-history-row.active {
  background: var(--surface-muted);
}

.run-history-main {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.run-history-main strong,
.run-history-main span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.run-history-status {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 72px;
  height: 22px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--border);
  font-size: 11px;
  text-transform: capitalize;
}

.run-history-status.status-completed {
  color: var(--success);
}

.run-history-status.status-failed {
  color: var(--danger);
}

.run-history-status.status-paused,
.run-history-status.status-stopped {
  color: var(--warning);
}

.run-history-actions {
  display: inline-flex;
  gap: 6px;
}
```

- [ ] **Step 7: Run UI checks**

Run:

```bash
npm --prefix crates/ui run typecheck
npm --prefix crates/ui run test -- src/api.test.ts
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/ui/src/context/AppContext.tsx crates/ui/src/context/AppProvider.tsx crates/ui/src/panels/RunHistoryPanel.tsx crates/ui/src/panels/DockPanel.tsx crates/ui/src/lib/types.ts crates/ui/src/styles/index.css
git commit -m "feat: add durable run history UI"
```

## Task 10: Acceptance, Docs Index, And Final Verification

**Files:**
- Modify: `docs/README.md`
- Modify: `docs/architecture/README.md`
- Modify: `docs/contributing/testing-workflows.md`
- Test: `crates/orchestration/tests/workflow_acceptance.rs`

- [ ] **Step 1: Add docs links**

Add `docs/architecture/run-persistence.md` to the architecture section in `docs/README.md` and `docs/architecture/README.md`.

- [ ] **Step 2: Add testing note**

In `docs/contributing/testing-workflows.md`, add this under "When To Run Each Layer":

````markdown
Run this when changing durable run persistence, replay, or resume behavior:

```bash
cargo test -p orchestration run::persistence adapters::storage::run_checkpoint_store run::coordinator_tests -- --nocapture
cargo test -p orchestration --test workflow_acceptance -- --nocapture
npm --prefix crates/ui run typecheck
```
````

- [ ] **Step 3: Add acceptance assertion for run ids**

In `crates/orchestration/tests/workflow_acceptance.rs`, extend `branch_join_workflow_preserves_sentinel_and_trace_contract` after the existing completed-trace assertion:

```rust
assert!(
    snapshot.run_trace.iter().any(|entry| entry.status == TraceStatus::Completed),
    "durable replay depends on completed trace entries being projected"
);
assert!(
    snapshot.chat_logs.values().any(|messages| !messages.is_empty()),
    "durable replay depends on chat logs being projected"
);
```

- [ ] **Step 4: Run narrow verification**

Run:

```bash
cargo test -p orchestration run::persistence adapters::storage::run_checkpoint_store run::coordinator_tests -- --nocapture
cargo test -p orchestration --test workflow_acceptance -- --nocapture
npm --prefix crates/ui run typecheck
npm --prefix crates/ui run test -- src/api.test.ts
```

Expected: PASS.

- [ ] **Step 5: Run full verification**

Run:

```bash
./scripts/verify.sh
```

Expected: every step reports PASS. If a step fails, fix the failing slice and rerun the specific repro command printed by `verify.sh`, then rerun `./scripts/verify.sh`.

- [ ] **Step 6: Commit**

```bash
git add docs/README.md docs/architecture/README.md docs/contributing/testing-workflows.md crates/orchestration/tests/workflow_acceptance.rs
git commit -m "docs: document durable run verification"
```

## Self-Review

**Spec coverage**

| Requirement | Covered By |
| --- | --- |
| Durable run record on disk | Tasks 1-3 |
| Append-only checkpoints | Tasks 2 and 4 |
| Engine checkpoint plus UI projection | Tasks 1 and 4 |
| Read-only replay | Tasks 5, 7, 8, 9 |
| Resume after app restart | Tasks 6 and 7 |
| Project/app storage split | Tasks 2 and 3 |
| Durable artifacts available for replay | Task 3 |
| Verification lane | Task 10 |

**Placeholder scan**

No task uses placeholder marker language or a code-free test instruction. The plan intentionally excludes fork/export instead of leaving it as unstated follow-up work.

**Type consistency**

The plan consistently uses `RunCheckpointStore`, `RunStoreRoot`, `RunRecord`, `RunSummary`, `RunCheckpointPayload`, `PendingRunCheckpoint`, and `RunCheckpointReason`. UI DTOs mirror Rust camelCase fields.

## Execution Options

Plan complete and saved to `docs/superpowers/plans/2026-06-16-durable-run-persistence-and-replay.md`. Two execution options:

**1. Subagent-Driven (recommended)** - dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** - execute tasks in this session using executing-plans, batch execution with checkpoints.

# Run Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist runs to disk with append-only checkpoints, durable artifacts, resume after restart, read-only replay, and fork-from-checkpoint.

**Architecture:** Project-scoped run records at `{project}/.flow/runs/{run_id}/`. `RunCheckpointStore` adapter in `orchestration/src/adapters/storage/`. Serialize `InteractiveEngineCheckpoint` + `WorkflowRunState` + artifact manifest at each pause/layer boundary. `drive.rs` writes checkpoints instead of only in-memory `checkpoint_sink`. Resume reconstructs engine from latest checkpoint. UI lists runs and opens read-only trace.

**Tech Stack:** Rust, serde_json, existing `InteractiveEngineCheckpoint`, Tauri v2, SolidJS.

**Reference:** `docs/ROADMAP.md` § Run checkpoint, history, and replay (decisions R1–R6).

---

## Run Record Layout

```text
{project}/.flow/runs/{run_id}/
├── run.json                 # metadata: workflow_id, started_at, status, cwd
├── checkpoints/
│   ├── 0001.json            # engine checkpoint + run state projection
│   └── 0002.json
└── artifacts/
    ├── {artifact_id}-bash.txt
    └── manifest.json        # artifact_id → relative path, tool, bytes
```

App-only workflows mirror under `{data_local}/openflow/runs/{run_id}/`.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `docs/architecture/run-persistence.md` | ADR documenting R1–R6 |
| `crates/orchestration/src/run/persistence/mod.rs` | `RunRecord`, `CheckpointPayload`, paths |
| `crates/orchestration/src/run/persistence/ports.rs` | `RunCheckpointStore` trait |
| `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs` | File impl |
| `crates/engine/src/execution/interactive_engine.rs` | Ensure checkpoint round-trip complete |
| `crates/orchestration/src/run/execution/drive.rs` | Auto-checkpoint hooks; durable artifact root |
| `crates/orchestration/src/run/coordinator.rs` | `resume_run`, `list_runs`, `load_run_projection` |
| `crates/orchestration/src/backend/mod.rs` | IPC delegates |
| `crates/desktop/src/lib.rs` | `list_runs`, `resume_run`, `replay_run`, `export_run` |
| `crates/ui/src/panels/RunHistoryPanel.tsx` | Browse past runs |
| `crates/ui/src/api.ts` | Typed IPC |

---

### Task 1: Persistence ADR + Schema

**Files:**
- Create: `docs/architecture/run-persistence.md`
- Create: `crates/orchestration/src/run/persistence/mod.rs`
- Create: `crates/orchestration/src/run/persistence/ports.rs`
- Test: `crates/orchestration/src/run/persistence/mod.rs`

- [ ] **Step 1: Write ADR**

`docs/architecture/run-persistence.md` must lock:

| ID | Decision |
| --- | --- |
| R1 | One run record per attempt; checkpoints append-only |
| R2 | Project path `.flow/runs/`; app-data mirror for non-project workflows |
| R3 | Checkpoint = `InteractiveEngineCheckpoint` + `WorkflowRunState` + artifact manifest |
| R4 | Auto-checkpoint on `AwaitInput`, `AwaitToolApproval`, layer done, terminal |
| R5 | Resume = same run id; fork = new run id copied from checkpoint |
| R6 | Fork re-executes providers from cursor (not trace-only replay) |

- [ ] **Step 2: Write failing schema test**

```rust
#[test]
fn run_record_serializes_metadata() {
    let record = RunRecord {
        run_id: "run-1".to_string(),
        workflow_id: "wf-1".to_string(),
        workflow_hash: "abc".to_string(),
        execution_cwd: "/tmp/proj".to_string(),
        started_at: "2026-06-15T12:00:00Z".to_string(),
        status: RunStatus::Paused,
    };
    let json = serde_json::to_string(&record).unwrap();
    assert!(json.contains("workflowId"));
}
```

- [ ] **Step 3: Define types**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub run_id: String,
    pub workflow_id: String,
    pub workflow_hash: String,
    pub execution_cwd: String,
    pub started_at: String,
    pub status: RunStatus,
    #[serde(default)]
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointPayload {
    pub seq: u32,
    pub created_at: String,
    pub reason: CheckpointReason,
    pub engine: engine::InteractiveEngineCheckpoint,
    pub projection: WorkflowRunState,
    pub artifact_manifest: Vec<ArtifactManifestEntry>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointReason {
    PauseInput,
    PauseApproval,
    LayerComplete,
    Stopped,
    Completed,
    Failed,
}
```

- [ ] **Step 4: Run test — PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "docs: add run persistence ADR and record schema"
```

---

### Task 2: RunCheckpointStore Adapter

**Files:**
- Create: `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs`
- Modify: `crates/orchestration/src/run/persistence/ports.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`
- Test: `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs`

- [ ] **Step 1: Define port**

```rust
pub trait RunCheckpointStore: Send + Sync {
    fn create_run(&self, record: &RunRecord) -> Result<(), RunPersistenceError>;
    fn append_checkpoint(&self, run_id: &str, payload: &CheckpointPayload) -> Result<(), RunPersistenceError>;
    fn load_latest_checkpoint(&self, run_id: &str) -> Result<Option<CheckpointPayload>, RunPersistenceError>;
    fn list_runs(&self, workflow_id: Option<&str>) -> Result<Vec<RunRecord>, RunPersistenceError>;
    fn update_status(&self, run_id: &str, status: RunStatus) -> Result<(), RunPersistenceError>;
    fn run_dir(&self, run_id: &str) -> PathBuf;
}
```

- [ ] **Step 2: Write failing round-trip test**

```rust
#[test]
fn store_append_and_load_latest_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileRunCheckpointStore::new(dir.path().to_path_buf());
    let run = sample_run_record("run-a");
    store.create_run(&run).unwrap();
    let cp1 = sample_checkpoint(1);
    let cp2 = sample_checkpoint(2);
    store.append_checkpoint("run-a", &cp1).unwrap();
    store.append_checkpoint("run-a", &cp2).unwrap();
    let loaded = store.load_latest_checkpoint("run-a").unwrap().unwrap();
    assert_eq!(loaded.seq, 2);
}
```

- [ ] **Step 3: Implement `FileRunCheckpointStore`**

- Create run dir + `run.json`.
- Write `checkpoints/{seq:04}.json` atomically (write temp + rename).
- `list_runs` reads subdirs with valid `run.json`.

- [ ] **Step 4: Run test — PASS**

- [ ] **Step 5: Commit**

---

### Task 3: Durable Artifact Root

**Files:**
- Modify: `crates/orchestration/src/run/execution/drive.rs`
- Modify: `crates/orchestration/src/tool/output.rs`
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Test: `crates/orchestration/src/run/execution/tests.rs`

- [ ] **Step 1: Write failing test — artifacts live under run dir**

```rust
#[tokio::test]
async fn drive_uses_run_scoped_artifact_root() {
    // assert ArtifactStore root == run_dir/artifacts, not openflow-run-{uuid} temp
}
```

- [ ] **Step 2: Change artifact root selection**

In `coordinator.rs` `start_run`:

```rust
let run_id = Uuid::new_v4().to_string();
let artifact_root = run_store.run_dir(&run_id).join("artifacts");
run_store.create_run(&run_record)?;
```

Pass `artifact_root` into `InteractiveWorkflowRunParams` (field already exists).

- [ ] **Step 3: Write `manifest.json` on each artifact spill**

Extend `ArtifactStore::store_text` to append manifest entry when `manifest_path` is set.

- [ ] **Step 4: Clean up old temp dirs on completion**

Remove `openflow-run-*` creation path; delete incomplete temp dirs on abort (ROADMAP item).

- [ ] **Step 5: Commit**

---

### Task 4: Auto-Checkpoint in Drive Loop

**Files:**
- Modify: `crates/orchestration/src/run/execution/drive.rs`
- Modify: `crates/orchestration/src/run/execution/events.rs`
- Test: `crates/orchestration/src/run/execution/tests.rs`

- [ ] **Step 1: Write failing checkpoint-on-pause test**

Extend existing `checkpoint_sink` tests to also call `RunCheckpointStore::append_checkpoint`.

- [ ] **Step 2: Add `checkpoint_store` to drive params**

```rust
pub struct InteractiveWorkflowRunParams<A> {
    // existing fields...
    pub run_id: String,
    pub checkpoint_store: Arc<dyn RunCheckpointStore>,
}
```

- [ ] **Step 3: Checkpoint helper**

```rust
fn persist_checkpoint(
    store: &dyn RunCheckpointStore,
    run_id: &str,
    seq: &mut u32,
    reason: CheckpointReason,
    engine: &InteractiveEngine,
    projection: &WorkflowRunState,
    manifest: &[ArtifactManifestEntry],
) -> Result<(), String> {
    *seq += 1;
    let payload = CheckpointPayload {
        seq: *seq,
        created_at: chrono_now(),
        reason,
        engine: engine.prepare_stop_checkpoint(),
        projection: projection.clone(),
        artifact_manifest: manifest.to_vec(),
    };
    store.append_checkpoint(run_id, &payload).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Call on pause events**

When `EngineRunResult::NeedsInput` or `NeedsApproval`, call `persist_checkpoint` before waiting.

On layer complete and terminal outcomes, checkpoint with matching `CheckpointReason`.

- [ ] **Step 5: Run tests — PASS**

- [ ] **Step 6: Commit**

---

### Task 5: Resume Run

**Files:**
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Modify: `crates/engine/src/execution/interactive_engine.rs`
- Modify: `crates/desktop/src/lib.rs`
- Test: `crates/orchestration/src/run/coordinator.rs` or execution tests

- [ ] **Step 1: Write failing resume test**

```rust
#[tokio::test]
async fn resume_run_reconstructs_engine_from_latest_checkpoint() {
    // start run → pause → persist → end session → resume_run(run_id) → NeedsInput again at same node
}
```

- [ ] **Step 2: Implement `resume_run`**

```rust
pub fn resume_run(&mut self, run_id: &str) -> Result<(), RunError> {
    let checkpoint = self.run_store.load_latest_checkpoint(run_id)?.ok_or(/* no checkpoint */)?;
    validate_workflow_unchanged(&checkpoint)?;
    let workflow = self.load_workflow(&checkpoint.projection.workflow_id)?;
    let engine = InteractiveEngine::from_checkpoint(workflow, checkpoint.engine)?;
    // rebuild ToolRunner with artifact_root = run_dir/artifacts
    // continue drive with resume_checkpoint = Some(...)
}
```

Reuse existing `continue_run` in-session path; `resume_run` is cross-session.

- [ ] **Step 3: IPC**

```rust
#[tauri::command]
async fn resume_run(run_id: String, state: State<'_, AppState>) -> Result<(), String> { /* ... */ }
```

- [ ] **Step 4: Stale workflow guard**

If `workflow_hash` differs, return error listing changed node ids (extend existing continuable validation).

- [ ] **Step 5: Commit**

---

### Task 6: Fork + Export

**Files:**
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: `fork_run_from_checkpoint`**

```rust
pub fn fork_run(&mut self, source_run_id: &str, checkpoint_seq: Option<u32>) -> Result<String, RunError> {
    let cp = load_checkpoint(source_run_id, checkpoint_seq)?;
    let new_run_id = Uuid::new_v4().to_string();
    copy_artifacts(&source_dir, &new_run_dir)?;
    create_run + start drive with copied engine checkpoint
    Ok(new_run_id)
}
```

- [ ] **Step 2: `export_run`**

Zip run dir via `zip` crate; IPC returns path to temp zip for user save dialog.

- [ ] **Step 3: Tests for fork creates new run id**

- [ ] **Step 4: Commit**

---

### Task 7: Run History UI

**Files:**
- Create: `crates/ui/src/panels/RunHistoryPanel.tsx`
- Modify: `crates/ui/src/api.ts`
- Modify: `crates/ui/src/panels/DockPanel.tsx` or sidebar

- [ ] **Step 1: IPC types**

```typescript
export interface RunRecordDto {
  runId: string;
  workflowId: string;
  status: string;
  startedAt: string;
  finishedAt?: string;
}
```

- [ ] **Step 2: Panel lists runs for active workflow**

Actions per row: **Open** (read-only trace), **Resume** (if paused), **Fork** (dropdown checkpoint).

- [ ] **Step 3: Read-only replay**

Load `WorkflowRunState` from latest checkpoint into UI without starting drive (no provider calls).

- [ ] **Step 4: Typecheck + vitest**

Run: `npm --prefix crates/ui run typecheck && npm --prefix crates/ui run test`

- [ ] **Step 5: `./scripts/verify.sh` + CHANGELOG + ROADMAP**

- [ ] **Step 6: Commit**

---

## Self-Review

| ROADMAP item | Task |
| --- | --- |
| Persistence policy ADR | Task 1 |
| Run record schema | Tasks 1–2 |
| Engine snapshot round-trip | Task 5 (verify existing tests) |
| RunCheckpointStore | Task 2 |
| Auto-checkpoint in drive | Task 4 |
| Durable artifact layout | Task 3 |
| Resume run | Task 5 |
| Fork from checkpoint | Task 6 |
| Export run | Task 6 |
| Run history UI | Task 7 |
| App restart offer resume | Task 7 + AppProvider bootstrap (add Step: scan incomplete runs on launch) |

**Gap to add:** Task 7b — on app launch, `list_runs` with `status: paused`; show toast "Resume run X?" — add as final step in Task 7.

---

## Suggested Execution Order

1. Tasks 1–2 (schema + store)
2. Task 3 (artifacts)
3. Task 4 (auto-checkpoint)
4. Task 5 (resume)
5. Task 6 (fork/export)
6. Task 7 (UI)

Depends on persistence policy (#6 in ROADMAP) — Task 1 ADR satisfies that gate.

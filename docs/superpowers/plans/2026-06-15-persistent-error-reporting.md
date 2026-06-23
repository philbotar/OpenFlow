# Persistent Error Reporting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture every meaningful failure — tools, AI turns, nodes, runs, subagents, backend IPC, terminal, and persistence — as structured, durable incident records that survive app restart.

**Architecture:** Add an `incident` domain in orchestration with a port-backed JSONL store under `{data_local}/openflow/incidents.jsonl`. Introduce a stable `run_id` on each run session. Record incidents from two hooks: (1) `RunCoordinator::apply_execution_event` maps `RunTelemetry` error variants, and (2) `AppBackend` records `BackendError` and store I/O failures at IPC boundaries. Expose list/dismiss via thin Tauri commands for debugging and a future UI slice — **no UI work in this plan.** Align run-scoped paths with ROADMAP #24 later (`{project}/.flow/runs/{run_id}/incidents.jsonl`).

**Tech Stack:** Rust (engine telemetry types, orchestration hex layout, Tauri IPC commands only), `./scripts/verify.sh`

**Out of scope (deferred):** Errors dock tab, `api.ts` / `port.ts` types, bootstrap payload, Settings screen controls, incident badges.

**Related docs:** [ROADMAP.md — Run lifecycle](../ROADMAP.md#run-lifecycle), [ROADMAP backlog — Error logging stored locally](../ROADMAP.md), [architecture/contract.md](../architecture/contract.md), [glossary — RunTelemetry](../glossary.md)

---

## Current gaps

| Area | Today | After this plan |
| --- | --- | --- |
| Tool failures | `ToolCompleted { is_error: true }` in live state only | Structured incident with tool name, call id, retryable flag, persisted |
| AI / node failures | `NodeErrored`, `NodeFailed`, `last_error: Option<String>` | Same events + durable incident with node scope |
| Run abort | `ExecutionEvent::Error(String)` ends run | Fatal incident + preserved history |
| Backend IPC | Toast only (ephemeral) | Incident recorded before error returns |
| Subagent failures | `SubagentFailed` in telemetry | Persisted with subagent id |
| App restart | All errors lost | Reload incidents from JSONL via IPC or direct file read |

---

## File map

| File | Responsibility |
| --- | --- |
| `crates/orchestration/src/incident/model.rs` | `IncidentRecord`, enums, `IncidentContext` |
| `crates/orchestration/src/incident/ports.rs` | `IncidentStore` trait |
| `crates/orchestration/src/incident/recorder.rs` | Map errors/events → records; append API |
| `crates/orchestration/src/incident/from_event.rs` | `RunTelemetry` → optional `IncidentRecord` |
| `crates/orchestration/src/adapters/storage/incident_store.rs` | JSONL append + list/load/dismiss |
| `crates/orchestration/src/api.rs` | `IncidentSummary` DTO for IPC |
| `crates/orchestration/src/backend/mod.rs` | Wire store + recorder; list/dismiss helpers |
| `crates/orchestration/src/run/coordinator.rs` | `run_id`, `project_id`, record on `apply_execution_event` |
| `crates/orchestration/src/run/state/mod.rs` | Optional `run_id` on `WorkflowRunState` (correlation only) |
| `crates/desktop/src/lib.rs` | `list_incidents` / `dismiss_incident` Tauri commands |
| `docs/ROADMAP.md`, `CHANGELOG.md` | Mark backlog item in progress / done |

**No engine behavior changes.** Engine keeps emitting `RunTelemetry`; orchestration owns persistence.

---

### Task 1: Incident domain model

**Files:**
- Create: `crates/orchestration/src/incident/mod.rs`
- Create: `crates/orchestration/src/incident/model.rs`
- Create: `crates/orchestration/src/incident/model_tests.rs`
- Modify: `crates/orchestration/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/orchestration/src/incident/model_tests.rs`:

```rust
use super::model::{IncidentCategory, IncidentRecord, IncidentScope, IncidentSeverity};
use engine::NodeId;
use std::collections::BTreeMap;

#[test]
fn incident_record_serializes_camel_case_for_ipc() {
    let record = IncidentRecord {
        id: "inc-1".to_string(),
        created_at_ms: 1_700_000_000_000,
        severity: IncidentSeverity::Error,
        category: IncidentCategory::Tool,
        scope: IncidentScope::Node {
            run_id: "run-1".to_string(),
            workflow_id: "wf-1".to_string(),
            node_id: NodeId("n1".to_string()),
        },
        code: "tool.timeout".to_string(),
        message: "[timeout] bash timed out after 300s".to_string(),
        hint: Some("increase timeout".to_string()),
        retryable: true,
        context: BTreeMap::from([
            ("toolName".to_string(), serde_json::json!("bash")),
            ("toolCallId".to_string(), serde_json::json!("tc-1")),
        ]),
        resolved: false,
    };
    let json = serde_json::to_value(&record).expect("serialize");
    assert_eq!(json["severity"], "error");
    assert_eq!(json["category"], "tool");
    assert_eq!(json["scope"]["type"], "node");
    assert_eq!(json["scope"]["runId"], "run-1");
    assert_eq!(json["retryable"], true);
    assert_eq!(json["resolved"], false);
}
```

Create `crates/orchestration/src/incident/model.rs`:

```rust
use engine::NodeId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSeverity {
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentCategory {
    Tool,
    AiInvoke,
    Node,
    Subagent,
    Run,
    Conversation,
    Workflow,
    Backend,
    Persistence,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum IncidentScope {
    App,
    Project { project_id: String },
    Run {
        run_id: String,
        workflow_id: String,
    },
    Node {
        run_id: String,
        workflow_id: String,
        node_id: NodeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentRecord {
    pub id: String,
    pub created_at_ms: u64,
    pub severity: IncidentSeverity,
    pub category: IncidentCategory,
    pub scope: IncidentScope,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub retryable: bool,
    #[serde(default)]
    pub context: BTreeMap<String, Value>,
    #[serde(default)]
    pub resolved: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncidentContext {
    pub run_id: Option<String>,
    pub workflow_id: Option<String>,
    pub project_id: Option<String>,
    pub node_id: Option<NodeId>,
    pub node_label: Option<String>,
}
```

Create `crates/orchestration/src/incident/mod.rs`:

```rust
mod model;
#[cfg(test)]
mod model_tests;

pub use model::{IncidentCategory, IncidentContext, IncidentRecord, IncidentScope, IncidentSeverity};
```

Add to `crates/orchestration/src/lib.rs` after `pub mod error;`:

```rust
pub mod incident;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p orchestration incident_record_serializes_camel_case_for_ipc -- --nocapture`
Expected: FAIL — module `incident` not found

- [ ] **Step 3: Implement model (code above) and lib export**

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p orchestration incident_record_serializes_camel_case_for_ipc -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/incident crates/orchestration/src/lib.rs
git commit -m "feat(incident): add structured incident record model"
```

---

### Task 2: IncidentStore port + JSONL adapter

**Files:**
- Create: `crates/orchestration/src/incident/ports.rs`
- Create: `crates/orchestration/src/adapters/storage/incident_store.rs`
- Create: `crates/orchestration/src/adapters/storage/incident_store_tests.rs`
- Modify: `crates/orchestration/src/adapters/storage/mod.rs`
- Modify: `crates/orchestration/src/incident/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/orchestration/src/adapters/storage/incident_store_tests.rs`:

```rust
use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::incident::{IncidentCategory, IncidentRecord, IncidentScope, IncidentSeverity};
use tempfile::tempdir;

fn sample_record(id: &str) -> IncidentRecord {
    IncidentRecord {
        id: id.to_string(),
        created_at_ms: 1,
        severity: IncidentSeverity::Error,
        category: IncidentCategory::Tool,
        scope: IncidentScope::App,
        code: "tool.failed".to_string(),
        message: "boom".to_string(),
        hint: None,
        retryable: false,
        context: Default::default(),
        resolved: false,
    }
}

#[test]
fn append_and_list_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path.clone());

    store.append(&sample_record("a")).unwrap();
    store.append(&sample_record("b")).unwrap();

    let listed = store.list(None).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, "a");
    assert_eq!(listed[1].id, "b");
}

#[test]
fn dismiss_marks_record_resolved() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path);

    store.append(&sample_record("x")).unwrap();
    store.dismiss("x").unwrap();

    let listed = store.list(None).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].resolved);
}
```

- [ ] **Step 2: Run test — expect FAIL**

Run: `cargo test -p orchestration append_and_list_round_trip -- --nocapture`

- [ ] **Step 3: Implement port + store**

`crates/orchestration/src/incident/ports.rs`:

```rust
use super::model::IncidentRecord;
use std::io;

#[derive(Debug, Clone, Default)]
pub struct IncidentListOptions {
    pub include_resolved: bool,
    pub limit: Option<usize>,
}

pub trait IncidentStore: Send + Sync {
    fn append(&self, record: &IncidentRecord) -> io::Result<()>;
    fn list(&self, options: Option<IncidentListOptions>) -> io::Result<Vec<IncidentRecord>>;
    fn dismiss(&self, id: &str) -> io::Result<()>;
    fn clear_resolved(&self) -> io::Result<usize>;
}
```

`crates/orchestration/src/adapters/storage/incident_store.rs`:

```rust
use crate::adapters::storage::json_file_store::{atomic_write, OPENFLOW_DATA_DIR_SLUG};
use crate::incident::model::IncidentRecord;
use crate::incident::ports::{IncidentListOptions, IncidentStore};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileIncidentStore {
    path: PathBuf,
}

impl FileIncidentStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(OPENFLOW_DATA_DIR_SLUG)
            .join("incidents.jsonl")
    }

    fn read_all(&self) -> io::Result<Vec<IncidentRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(&self.path)?;
        let reader = io::BufReader::new(file);
        let mut records = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: IncidentRecord = serde_json::from_str(&line).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("incidents.jsonl line {}: {error}", index + 1),
                )
            })?;
            records.push(record);
        }
        Ok(records)
    }

    fn write_all(&self, records: &[IncidentRecord]) -> io::Result<()> {
        let mut body = String::new();
        for record in records {
            let line = serde_json::to_string(record).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("incident serialization failed: {error}"),
                )
            })?;
            body.push_str(&line);
            body.push('\n');
        }
        atomic_write(&self.path, &body)
    }
}

impl IncidentStore for FileIncidentStore {
    fn append(&self, record: &IncidentRecord) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(record).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("incident serialization failed: {error}"),
            )
        })?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{line}")?;
        file.sync_all()?;
        Ok(())
    }

    fn list(&self, options: Option<IncidentListOptions>) -> io::Result<Vec<IncidentRecord>> {
        let options = options.unwrap_or_default();
        let mut records = self.read_all()?;
        if !options.include_resolved {
            records.retain(|record| !record.resolved);
        }
        if let Some(limit) = options.limit {
            if records.len() > limit {
                let start = records.len() - limit;
                records = records.split_off(start);
            }
        }
        Ok(records)
    }

    fn dismiss(&self, id: &str) -> io::Result<()> {
        let mut records = self.read_all()?;
        let mut found = false;
        for record in &mut records {
            if record.id == id {
                record.resolved = true;
                found = true;
                break;
            }
        }
        if !found {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("incident {id} not found"),
            ));
        }
        self.write_all(&records)
    }

    fn clear_resolved(&self) -> io::Result<usize> {
        let records = self.read_all()?;
        let before = records.len();
        let kept: Vec<_> = records.into_iter().filter(|r| !r.resolved).collect();
        let removed = before - kept.len();
        self.write_all(&kept)?;
        Ok(removed)
    }
}
```

Export in `adapters/storage/mod.rs`:

```rust
pub mod incident_store;
#[cfg(test)]
mod incident_store_tests;
```

Re-export port in `incident/mod.rs`:

```rust
pub mod ports;
pub use ports::{IncidentListOptions, IncidentStore};
```

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test -p orchestration incident_store -- --nocapture`

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/incident/ports.rs \
  crates/orchestration/src/adapters/storage/incident_store.rs \
  crates/orchestration/src/adapters/storage/incident_store_tests.rs \
  crates/orchestration/src/adapters/storage/mod.rs \
  crates/orchestration/src/incident/mod.rs
git commit -m "feat(incident): add JSONL incident store"
```

---

### Task 3: IncidentRecorder — map ToolError, AgentError, BackendError

**Files:**
- Create: `crates/orchestration/src/incident/recorder.rs`
- Create: `crates/orchestration/src/incident/recorder_tests.rs`
- Modify: `crates/orchestration/src/incident/mod.rs`

- [ ] **Step 1: Write the failing tests**

`crates/orchestration/src/incident/recorder_tests.rs`:

```rust
use super::recorder::{incident_from_tool_error, IncidentRecorder};
use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::error::BackendError;
use crate::incident::{IncidentCategory, IncidentContext, IncidentSeverity};
use crate::tool::errors::ToolError;
use engine::{AgentError, NodeId};
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn tool_timeout_maps_to_retryable_incident() {
    let err = ToolError::Timeout {
        tool: "bash".to_string(),
        after_secs: 300,
        hint: "retry".to_string(),
        partial_output: None,
    };
    let ctx = IncidentContext {
        run_id: Some("run-1".to_string()),
        workflow_id: Some("wf-1".to_string()),
        node_id: Some(NodeId("n1".to_string())),
        ..Default::default()
    };
    let record = incident_from_tool_error(&err, "tc-1", &ctx);
    assert_eq!(record.category, IncidentCategory::Tool);
    assert_eq!(record.code, "tool.timeout");
    assert!(record.retryable);
    assert_eq!(record.severity, IncidentSeverity::Error);
}

#[test]
fn agent_transient_maps_to_ai_invoke_incident() {
    let dir = tempdir().unwrap();
    let store = Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl")));
    let recorder = IncidentRecorder::new(store);
    let ctx = IncidentContext {
        run_id: Some("run-1".to_string()),
        workflow_id: Some("wf-1".to_string()),
        node_id: Some(NodeId("n1".to_string())),
        node_label: Some("Planner".to_string()),
        ..Default::default()
    };
    recorder
        .record_agent_error(&AgentError::Transient("rate limited".to_string()), &ctx)
        .unwrap();
    let listed = recorder.list_unresolved(10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].category, IncidentCategory::AiInvoke);
    assert_eq!(listed[0].code, "ai.transient");
    assert!(listed[0].retryable);
}

#[test]
fn backend_error_maps_to_backend_category() {
    let dir = tempdir().unwrap();
    let store = Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl")));
    let recorder = IncidentRecorder::new(store);
    recorder
        .record_backend(&BackendError::NoActiveRun, &IncidentContext::default())
        .unwrap();
    let listed = recorder.list_unresolved(10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].category, IncidentCategory::Backend);
    assert_eq!(listed[0].code, "backend.no_active_run");
}
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test -p orchestration incident_recorder -- --nocapture`

- [ ] **Step 3: Implement recorder**

Make `build_record` **`pub(crate)`** so `from_event.rs` can call it.

`crates/orchestration/src/incident/recorder.rs` — full implementation with `IncidentRecorder`, `scope_from_context`, `record`, `list_unresolved`, `dismiss`, `record_backend`, `record_agent_error`, `incident_from_tool_error`, `backend_error_code` (same as first draft of this plan).

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/incident/recorder.rs crates/orchestration/src/incident/recorder_tests.rs
git commit -m "feat(incident): add recorder mappers for tool, AI, backend errors"
```

---

### Task 4: Map RunTelemetry events to incidents

**Files:**
- Create: `crates/orchestration/src/incident/from_event.rs`
- Create: `crates/orchestration/src/incident/from_event_tests.rs`

- [ ] **Step 1: Write failing tests**

`crates/orchestration/src/incident/from_event_tests.rs`:

```rust
use super::from_event::incident_from_execution_event;
use crate::incident::{IncidentCategory, IncidentContext, IncidentSeverity};
use engine::{NodeId, RunTelemetry};

#[test]
fn node_errored_becomes_node_incident() {
    let ctx = IncidentContext {
        run_id: Some("run-1".to_string()),
        workflow_id: Some("wf-1".to_string()),
        ..Default::default()
    };
    let event = RunTelemetry::NodeErrored {
        node_id: NodeId("n1".to_string()),
        label: "Worker".to_string(),
        error: "model refused".to_string(),
    };
    let record = incident_from_execution_event(&event, &ctx).expect("record");
    assert_eq!(record.category, IncidentCategory::Node);
    assert_eq!(record.code, "node.errored");
    assert_eq!(record.severity, IncidentSeverity::Error);
}

#[test]
fn tool_completed_error_becomes_tool_incident() {
    let ctx = IncidentContext {
        run_id: Some("run-1".to_string()),
        workflow_id: Some("wf-1".to_string()),
        ..Default::default()
    };
    let event = RunTelemetry::ToolCompleted {
        node_id: NodeId("n1".to_string()),
        tool_call_id: "tc-1".to_string(),
        tool_name: "read".to_string(),
        content: "[not_found] missing — use grep".to_string(),
        is_error: true,
        output_meta: None,
        artifact_ids: vec![],
    };
    let record = incident_from_execution_event(&event, &ctx).expect("record");
    assert_eq!(record.category, IncidentCategory::Tool);
    assert_eq!(record.code, "tool.not_found");
}

#[test]
fn finished_does_not_emit_incident() {
    let ctx = IncidentContext::default();
    let event = RunTelemetry::Finished(engine::RunReport {
        workflow_id: "wf".into(),
        events: vec![],
        outputs: vec![],
    });
    assert!(incident_from_execution_event(&event, &ctx).is_none());
}
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test -p orchestration incident_from_execution -- --nocapture`

- [ ] **Step 3: Implement `from_event.rs`**

Map: `ToolCompleted` (error), `ToolDenied`, `NodeErrored`, `NodeFailed`, `SubagentFailed`, `Error`. Ignore `Finished`. Include `parse_tool_code` helper (same as first draft).

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/orchestration/src/incident/from_event.rs crates/orchestration/src/incident/from_event_tests.rs
git commit -m "feat(incident): map RunTelemetry failures to incident records"
```

---

### Task 5: Run session `run_id` + record on `apply_execution_event`

**Files:**
- Modify: `crates/orchestration/src/run/coordinator.rs`
- Modify: `crates/orchestration/src/run/state/mod.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/orchestration/src/run/coordinator_tests.rs`

- [ ] **Step 1: Write failing integration test**

```rust
#[tokio::test]
async fn apply_execution_event_records_tool_failure_incident() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl")));
    let coordinator = RunCoordinator::new_with_incidents(
        Handle::current(),
        Arc::new(IncidentRecorder::new(store.clone())),
    );
    // reuse existing coordinator test fixtures
    // apply_execution_event(ToolCompleted { is_error: true, ... })
    let listed = store.list(None).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].code, "tool.not_found");
}
```

- [ ] **Step 2: Run test — expect FAIL**

- [ ] **Step 3: Wire `run_id`, `project_id`, recorder on `apply_execution_event`**

Add to `RunSession`:

```rust
run_id: Option<String>,
project_id: Option<String>,
```

Set on `start_run`:

```rust
session.run_id = Some(Uuid::new_v4().to_string());
```

Add to `WorkflowRunState` (serialization only — no UI consumption yet):

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub run_id: Option<String>,
```

In `RunCoordinator::apply_execution_event` (recorder owned by coordinator or passed from backend):

```rust
let ctx = incident_context_from_session(&session);
if let Some(record) = incident_from_execution_event(&event, &ctx) {
    if let Err(error) = self.incidents.record(record) {
        log::warn!("failed to persist incident: {error}");
    }
}
apply_event_to_run_state(&workflow, run_state, event);
```

- [ ] **Step 4: Run test — expect PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(incident): record incidents from execution events"
```

---

### Task 6: Record BackendError at IPC boundary

**Files:**
- Modify: `crates/orchestration/src/backend/mod.rs`
- Modify: `crates/orchestration/src/backend/tests.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn backend_err_persists_incident_before_returning() {
    let dir = tempfile::tempdir().unwrap();
    let backend = test_backend_with_incident_store(dir.path());
    let err = backend.backend_err(BackendError::NoActiveRun);
    assert!(matches!(err, BackendError::NoActiveRun));
    let listed = backend.list_incidents(50).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].code, "backend.no_active_run");
}
```

- [ ] **Step 2: Run test — expect FAIL**

- [ ] **Step 3: Add `AppBackend` helpers**

```rust
impl AppBackend {
    pub fn backend_err(&self, error: BackendError) -> BackendError {
        let ctx = self.current_incident_context();
        if let Err(io_error) = self.incidents.record_backend(&error, &ctx) {
            log::warn!("failed to persist backend incident: {io_error}");
        }
        error
    }

    pub fn list_incidents(&self, limit: usize) -> io::Result<Vec<IncidentRecord>> {
        self.incidents.list_unresolved(limit)
    }

    pub fn dismiss_incident(&self, id: &str) -> io::Result<()> {
        self.incidents.dismiss(id)
    }

    pub fn list_incident_summaries(&self, limit: usize) -> io::Result<Vec<IncidentSummary>> {
        self.list_incidents(limit).map(|records| {
            records.into_iter().map(IncidentSummary::from).collect()
        })
    }
}
```

Wire `.map_err(|e| self.backend_err(e))?` on run lifecycle commands (`start_run`, `submit_user_input`, `resolve_tool_approval`, `continue_run`). Skip keystroke-level validation.

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(incident): persist backend IPC failures"
```

---

### Task 7: Desktop IPC (no UI wiring)

**Files:**
- Modify: `crates/orchestration/src/api.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/lib.rs` — register commands only; **do not** touch `crates/ui/**`

- [ ] **Step 1: Add DTO**

`crates/orchestration/src/api.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentSummary {
    pub id: String,
    pub created_at_ms: u64,
    pub severity: String,
    pub category: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub resolved: bool,
    pub workflow_id: Option<String>,
    pub run_id: Option<String>,
    pub node_id: Option<String>,
}

impl From<crate::incident::IncidentRecord> for IncidentSummary {
    fn from(record: crate::incident::IncidentRecord) -> Self {
        let (workflow_id, run_id, node_id) = match record.scope {
            crate::incident::IncidentScope::Node {
                run_id,
                workflow_id,
                node_id,
            } => (
                Some(workflow_id),
                Some(run_id),
                Some(node_id.0),
            ),
            crate::incident::IncidentScope::Run {
                run_id,
                workflow_id,
            } => (Some(workflow_id), Some(run_id), None),
            _ => (None, None, None),
        };
        Self {
            id: record.id,
            created_at_ms: record.created_at_ms,
            severity: format!("{:?}", record.severity).to_lowercase(),
            category: format!("{:?}", record.category).to_lowercase(),
            code: record.code,
            message: record.message,
            retryable: record.retryable,
            resolved: record.resolved,
            workflow_id,
            run_id,
            node_id,
        }
    }
}
```

- [ ] **Step 2: Add Tauri commands**

```rust
#[tauri::command]
fn list_incidents(
    backend: tauri::State<'_, AppBackend>,
    limit: Option<usize>,
) -> Result<Vec<orchestration::api::IncidentSummary>, CommandError> {
    backend
        .list_incident_summaries(limit.unwrap_or(200))
        .map_err(|error| backend.backend_err(BackendError::ProjectOperation(error.to_string())))
}

#[tauri::command]
fn dismiss_incident(
    backend: tauri::State<'_, AppBackend>,
    id: String,
) -> Result<(), CommandError> {
    backend
        .dismiss_incident(&id)
        .map_err(|error| backend.backend_err(BackendError::ProjectOperation(error.to_string())))
}
```

Register in `invoke_handler`. **Do not** extend `BootstrapPayload` or add `api.ts` wrappers.

- [ ] **Step 3: Smoke test via orchestration backend tests**

```rust
#[test]
fn list_incident_summaries_projects_records() {
    // append via recorder, call backend.list_incident_summaries, assert DTO shape
}
```

- [ ] **Step 4: Run verification subset**

Run: `cargo test -p orchestration backend::tests -p desktop -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(incident): add list/dismiss Tauri commands (no UI)"
```

---

### Task 8: Terminal + persistence error capture

**Files:**
- Modify: `crates/orchestration/src/terminal/mod.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`

- [ ] **Step 1: Write failing test** — terminal start failure produces `terminal.start_failed` incident.

- [ ] **Step 2: Run test — expect FAIL**

- [ ] **Step 3: Record errors**

Terminal failures → `IncidentCategory::Terminal`. Settings/workflow save failures → `IncidentCategory::Persistence` with codes `persistence.settings_save`, `persistence.workflow_save`.

- [ ] **Step 4: Run tests — expect PASS**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(incident): capture terminal and persistence failures"
```

---

### Task 9: Retention policy (backend only)

**Files:**
- Modify: `crates/orchestration/src/settings/model.rs`
- Modify: `crates/orchestration/src/incident/recorder.rs`
- Modify: `crates/orchestration/src/backend/mod.rs`

- [ ] **Step 1: Add settings field**

```rust
// AppSettings — default 500
#[serde(default = "default_incident_retention_max")]
pub incident_retention_max: u32,

fn default_incident_retention_max() -> u32 {
    500
}
```

- [ ] **Step 2: Prune on append**

After append, if total records > max, drop oldest resolved first, then oldest overall.

- [ ] **Step 3: Add `clear_resolved_incidents` Tauri command**

```rust
#[tauri::command]
fn clear_resolved_incidents(backend: tauri::State<'_, AppBackend>) -> Result<u32, CommandError> {
    backend.clear_resolved_incidents().map_err(...)
}
```

**No Settings screen changes.**

- [ ] **Step 4: Tests for pruning + clear**

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(incident): retention policy and clear resolved IPC"
```

---

### Task 10: Integration tests + verification

**Files:**
- Modify: `crates/orchestration/src/run/execution/tests.rs`
- Modify: `CHANGELOG.md`, `docs/ROADMAP.md`

- [ ] **Step 1: Headless execution test**

After tool failure event through coordinator, assert JSONL on disk contains tool incident with correct scope.

- [ ] **Step 2: Run full verification**

Run: `./scripts/verify.sh`
Expected: all steps PASS (no new UI tests required)

- [ ] **Step 3: Update changelog + roadmap**

`CHANGELOG.md`:

```markdown
- **Persistent error reporting (backend):** structured incidents for tool, node, run, backend, terminal, and persistence failures; JSONL store at `{data_local}/openflow/incidents.jsonl`; `list_incidents` / `dismiss_incident` Tauri commands. UI deferred.
```

Move ROADMAP backlog item "Error logging stored locally" toward Done (backend slice).

- [ ] **Step 4: Commit**

```bash
git commit -m "docs: changelog and roadmap for persistent error reporting"
```

---

## Deferred (follow-up plan)

| Item | Notes |
| --- | --- |
| Errors dock tab + badge | New plan or phase 2 |
| `api.ts` / `port.ts` / bootstrap | Wire when UI lands |
| Settings "Clear resolved errors" button | Optional UX on top of existing IPC |
| Mirror into `{project}/.flow/runs/{run_id}/` | ROADMAP #24 |
| Agent loop proposes fixes from incident history | Separate backlog item |

---

## Self-review

**Spec coverage (backend slice)**

| Requirement | Task |
| --- | --- |
| Failed tools persist | 4, 5 |
| Failed nodes / conversations | 4, 5 |
| Failed workflows / runs | 4 |
| Subagent failures | 4 |
| Backend IPC failures | 6 |
| Terminal / persistence failures | 8 |
| Survive restart | 2 |
| Query / dismiss (no UI) | 7, 9 |
| Retention | 9 |

**Placeholder scan:** Task 3 Step 3 and Task 4 Step 3 reference the recorder/from_event bodies from the first plan draft — executor must paste the full `recorder.rs` and `from_event.rs` implementations (no `// ...` stubs).

**Type consistency:** `IncidentSummary` maps from `IncidentRecord`; `run_id` on `WorkflowRunState` matches session; no `BottomTab` changes.

---

## Execution handoff

**Plan updated:** UI removed; 10 backend tasks. Saved to `docs/superpowers/plans/2026-06-15-persistent-error-reporting.md`.

**1. Subagent-Driven (recommended)** — fresh subagent per task

**2. Inline Execution** — batch in this session with checkpoints

**Which approach?**

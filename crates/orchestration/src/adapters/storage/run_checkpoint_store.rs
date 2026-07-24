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
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("run JSON serialization failed: {error}"),
        )
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

    fn load_record(
        &self,
        roots: &[RunStoreRoot],
        run_id: &str,
    ) -> io::Result<Option<(RunStoreRoot, RunRecord)>> {
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

    fn list_runs(
        &self,
        roots: &[RunStoreRoot],
        workflow_id: Option<&str>,
    ) -> io::Result<Vec<RunSummary>> {
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
                let record = match read_run_record(&path) {
                    Ok(record) => record,
                    Err(error) if error.kind() == io::ErrorKind::InvalidData => continue,
                    Err(error) => return Err(error),
                };
                if workflow_id.is_some_and(|expected| expected != record.workflow_id) {
                    continue;
                }
                runs.push(record.summary());
            }
        }
        runs.sort_by_key(|run| std::cmp::Reverse(run.updated_at_ms));
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
    use crate::run::persistence::{RunCheckpointPayload, RunCheckpointReason};
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
            workflow_snapshot: Workflow::new("Demo"),
            project_id: Some("project-1".to_string()),
            execution_cwd: base.display().to_string(),
            artifact_root: base
                .join(".flow")
                .join("runs")
                .join(run_id)
                .join("artifacts")
                .display()
                .to_string(),
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
                reads_by_node: BTreeMap::new(),
                transcripts: BTreeMap::new(),
                awaiting_nodes: BTreeSet::new(),
                pending_tool_batches: BTreeMap::new(),
                retries_by_node: BTreeMap::new(),
                transient_streaks_by_node: BTreeMap::new(),
                submit_output_retries_by_node: BTreeMap::new(),
                request_input_retries_by_node: BTreeMap::new(),
                empty_turn_retries_by_node: BTreeMap::new(),
                mixed_tool_turn_retries_by_node: BTreeMap::new(),
                auto_continue_streaks_by_node: BTreeMap::new(),
                entrypoint_text: None,
                interrupted_nodes: BTreeSet::new(),
                failed_nodes: BTreeMap::new(),
                plan_mode_source_node_id: None,
                frozen_change_evidence_packet: None,
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
        store
            .append_checkpoint(&root, "run-a", &checkpoint(1))
            .expect("checkpoint 1");
        store
            .append_checkpoint(&root, "run-a", &checkpoint(2))
            .expect("checkpoint 2");
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

        assert_eq!(
            runs.iter()
                .map(|run| run.run_id.as_str())
                .collect::<Vec<_>>(),
            vec!["run-new", "run-old"]
        );
    }

    #[test]
    fn list_runs_skips_legacy_records_missing_workflow_snapshot() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileRunCheckpointStore;
        let root = root(dir.path());
        let valid = record(dir.path(), "run-valid");

        store.create_run(&root, &valid).expect("valid run");
        let legacy_dir = run_dir(&root, "run-legacy");
        fs::create_dir_all(&legacy_dir).expect("legacy run dir");
        fs::write(
            legacy_dir.join(RUN_FILE_NAME),
            r#"{
                "runId": "run-legacy",
                "workflowId": "wf-1",
                "workflowName": "Legacy",
                "workflowHash": "hash",
                "projectId": null,
                "executionCwd": "/tmp",
                "artifactRoot": "/tmp/artifacts",
                "startedAtMs": 1,
                "updatedAtMs": 1,
                "status": "running"
            }"#,
        )
        .expect("legacy run record");

        let runs = store.list_runs(&[root], None).expect("list runs");

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "run-valid");
    }

    #[test]
    fn update_status_rewrites_run_record() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileRunCheckpointStore;
        let root = root(dir.path());
        store
            .create_run(&root, &record(dir.path(), "run-a"))
            .expect("create run");

        store
            .update_status(&root, "run-a", RunStatus::Completed, 300)
            .expect("update");
        let (_, loaded) = store
            .load_record(&[root], "run-a")
            .expect("load")
            .expect("record");

        assert_eq!(loaded.status, RunStatus::Completed);
        assert_eq!(loaded.updated_at_ms, 300);
    }
}

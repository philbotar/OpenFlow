use super::*;
use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
use crate::incident::{IncidentRecorder, IncidentScope};
use crate::run::ports::RunCheckpointStore;
use engine::NodeId;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

fn seeded_session(artifact_root: PathBuf) -> RunSession {
    RunSession {
        workflow: None,
        run_state: None,
        run_id: None,
        run_root: None,
        project_id: None,
        execution_cwd: None,
        entrypoint: None,
        artifact_root: Some(artifact_root),
        engine_checkpoint: None,
        checkpoint_sink: None,
        snapshot_store: None,
        lsp_settings: None,
        pending_engine_reverts: None,
        action_tx: None,
        handle: None,
        cancel_token: None,
        node_interrupts: None,
    }
}

#[test]
fn finish_run_session_preserves_durable_artifact_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let artifact_root = dir.path().join("artifacts");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::write(artifact_root.join("spill.txt"), "hello").expect("seed artifact");

    let mut session = seeded_session(artifact_root.clone());
    finish_run_session(&mut session);

    assert_eq!(session.artifact_root.as_ref(), Some(&artifact_root));
    assert!(artifact_root.exists());
}

#[test]
fn finish_run_session_tolerates_missing_artifact_root() {
    let mut session = seeded_session(PathBuf::from("/tmp/openflow-missing-artifact-root-test"));
    finish_run_session(&mut session);
    assert!(session.handle.is_none());
    assert!(session.checkpoint_sink.is_none());
}

#[test]
fn durable_artifact_root_lives_under_run_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = crate::run::persistence::RunStoreRoot {
        project_id: Some("project-1".to_string()),
        root: dir.path().join(".flow").join("runs"),
    };
    let store = FileRunCheckpointStore;
    let artifact_root = store.run_dir(&root, "run-1").join("artifacts");

    assert_eq!(
        artifact_root,
        dir.path()
            .join(".flow")
            .join("runs")
            .join("run-1")
            .join("artifacts")
    );
}

#[test]
fn workflow_hash_detects_changed_workflow_for_resume_guard() {
    let mut workflow = Workflow::new("Resume");
    let original = crate::run::persistence::workflow_hash(&workflow);
    workflow.name = "Changed".to_string();
    let changed = crate::run::persistence::workflow_hash(&workflow);

    assert_ne!(original, changed);
}

#[tokio::test]
async fn replay_run_returns_inactive_projection_without_pending_actions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileRunCheckpointStore;
    let root = crate::run::persistence::RunStoreRoot {
        project_id: None,
        root: dir.path().join("runs"),
    };
    let workflow = Workflow::new("Replay");
    let mut projection = WorkflowRunState::running_for_workflow(&workflow);
    projection.run_id = Some("run-1".to_string());
    projection.awaiting_node_id = Some(NodeId("node-1".to_string()));
    projection
        .awaiting_node_ids
        .push(NodeId("node-1".to_string()));
    let record = crate::run::persistence::RunRecord {
        run_id: "run-1".to_string(),
        workflow_id: workflow.id.to_string(),
        workflow_name: workflow.name.clone(),
        workflow_hash: crate::run::persistence::workflow_hash(&workflow),
        project_id: None,
        execution_cwd: dir.path().display().to_string(),
        artifact_root: dir
            .path()
            .join("runs/run-1/artifacts")
            .display()
            .to_string(),
        started_at_ms: 1,
        updated_at_ms: 1,
        status: crate::run::persistence::RunStatus::Paused,
    };
    store.create_run(&root, &record).expect("create run");
    store
        .append_checkpoint(
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
        )
        .expect("checkpoint");
    let coordinator = RunCoordinator::new(
        tokio::runtime::Handle::current(),
        Arc::new(IncidentRecorder::new(Arc::new(FileIncidentStore::new(
            dir.path().join("incidents.jsonl"),
        )))),
    );

    let replay = coordinator
        .replay_run(&store, &[root], "run-1")
        .expect("replay run");

    assert!(!replay.active);
    assert!(replay.awaiting_node_id.is_none());
    assert!(replay.awaiting_node_ids.is_empty());
}

#[tokio::test]
async fn apply_execution_event_records_tool_failure_incident() {
    let dir = tempdir().expect("tempdir");
    let store = Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl")));
    let incidents = Arc::new(IncidentRecorder::new(store));
    let run_store = FileRunCheckpointStore;
    let coordinator =
        RunCoordinator::new_with_incidents(tokio::runtime::Handle::current(), incidents.clone());

    let workflow = Workflow::new("wf-incident");
    let expected_workflow_id = workflow.id.to_string();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some("run-incident-1".to_string());
    let (action_tx, _action_rx) = tokio::sync::mpsc::unbounded_channel();
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    coordinator
        .apply_execution_event(
            ExecutionEvent::ToolCompleted {
                node_id: NodeId("node-1".to_string()),
                tool_call_id: "tool-call-1".to_string(),
                tool_name: "read".to_string(),
                content: "[not_found] file missing — use project file references".to_string(),
                is_error: true,
                output_meta: None,
                artifact_ids: vec![],
            },
            &run_store,
        )
        .await
        .expect("apply event");

    let listed = incidents.list_unresolved(10).expect("list incidents");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].code, "tool.not_found");
    match &listed[0].scope {
        IncidentScope::Node {
            run_id,
            workflow_id,
            node_id,
        } => {
            assert_eq!(run_id, "run-incident-1");
            assert_eq!(workflow_id, &expected_workflow_id);
            assert_eq!(node_id, &NodeId("node-1".to_string()));
        }
        scope => panic!("expected node scope, got {scope:?}"),
    }
}

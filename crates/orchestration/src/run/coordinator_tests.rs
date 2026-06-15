use super::*;
use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::incident::{IncidentRecorder, IncidentScope};
use engine::NodeId;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

fn seeded_session(artifact_root: PathBuf) -> RunSession {
    RunSession {
        workflow: None,
        run_state: None,
        run_id: None,
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
fn finish_run_session_removes_artifact_root_when_run_is_not_continuable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let artifact_root = dir.path().join("artifacts");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::write(artifact_root.join("spill.txt"), "hello").expect("seed artifact");

    let mut session = seeded_session(artifact_root.clone());
    finish_run_session(&mut session);

    assert!(session.artifact_root.is_none());
    assert!(!artifact_root.exists());
}

#[test]
fn finish_run_session_preserves_artifact_root_for_continuable_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let artifact_root = dir.path().join("artifacts");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::write(artifact_root.join("spill.txt"), "hello").expect("seed artifact");

    let mut session = seeded_session(artifact_root.clone());
    let workflow = Workflow::new("wf-1");
    session.engine_checkpoint = Some(InteractiveEngineCheckpoint {
        workflow_id: workflow.id,
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
    });

    finish_run_session(&mut session);

    assert_eq!(session.artifact_root.as_ref(), Some(&artifact_root));
    assert!(artifact_root.exists());
}

#[test]
fn finish_run_session_tolerates_missing_artifact_root() {
    let mut session = seeded_session(PathBuf::from("/tmp/openflow-missing-artifact-root-test"));
    finish_run_session(&mut session);
    assert!(session.artifact_root.is_none());
}

#[tokio::test]
async fn apply_execution_event_records_tool_failure_incident() {
    let dir = tempdir().expect("tempdir");
    let store = Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl")));
    let incidents = Arc::new(IncidentRecorder::new(store));
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
        .apply_execution_event(ExecutionEvent::ToolCompleted {
            node_id: NodeId("node-1".to_string()),
            tool_call_id: "tool-call-1".to_string(),
            tool_name: "read".to_string(),
            content: "[not_found] file missing — use project file references".to_string(),
            is_error: true,
            output_meta: None,
            artifact_ids: vec![],
        })
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

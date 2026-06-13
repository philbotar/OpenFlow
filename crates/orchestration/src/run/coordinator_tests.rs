use super::*;
use std::fs;

fn seeded_session(artifact_root: PathBuf) -> RunSession {
    RunSession {
        workflow: None,
        run_state: None,
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

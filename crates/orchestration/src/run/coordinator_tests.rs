use super::*;
use engine::InteractiveEngine;
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
    let mut workflow = Workflow::new("wf-1");
    workflow.nodes.push(engine::Node::agent("Agent", 0.0, 0.0));
    let mut engine = InteractiveEngine::new(workflow, None).expect("engine");
    session.engine_checkpoint = Some(engine.prepare_stop_checkpoint());

    finish_run_session(&mut session);

    assert_eq!(session.artifact_root.as_ref(), Some(&artifact_root));
    assert!(artifact_root.exists());
}

//! Coordinator unit and integration tests.

use super::checkpoint::status_for_checkpoint;
use super::session::{
    apply_user_stop_to_session, clear_artifact_root, finish_run_session, fresh_execution_resources,
    RunSession,
};
use super::{DurableResumeParams, RunCoordinator, RunStartParams, TestSessionSeed};
use crate::adapters::storage::agent_store::FileAgentStore;
use crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
use crate::adapters::storage::settings_store::FileSettingsStore;
use crate::error::BackendError;
use crate::run::execution::{ExecutionAction, ExecutionEvent, NodeInterrupts};
use crate::run::persistence::{
    workflow_hash, RunCheckpointPayload, RunCheckpointReason, RunRecord, RunStatus, RunStoreRoot,
};
use crate::run::ports::RunCheckpointStore;
use crate::run::state::{AgentStatus, WorkflowRunState};
use crate::settings::model::AppSettings;
use crate::settings::provider::ProviderEnv;
use crate::workflow::catalog::default_workflow;
use engine::{
    InteractiveEngineCheckpoint, NodeId, PendingToolApproval, ToolCall, ToolTier, Workflow,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

fn coordinator(_dir: &Path) -> RunCoordinator {
    RunCoordinator::new(tokio::runtime::Handle::current())
}

fn test_env() -> ProviderEnv {
    ProviderEnv::from_pairs([("OPENAI_API_KEY", "test-openai-key")])
}

fn empty_engine_checkpoint(workflow: &Workflow) -> InteractiveEngineCheckpoint {
    InteractiveEngineCheckpoint {
        workflow_id: workflow.id.clone(),
        layer_idx: 0,
        outputs: Default::default(),
        changed_files_by_node: Default::default(),
        reads_by_node: Default::default(),
        transcripts: Default::default(),
        awaiting_nodes: Default::default(),
        work_phase_nodes: Default::default(),
        pending_tool_batches: Default::default(),
        retries_by_node: Default::default(),
        transient_streaks_by_node: Default::default(),
        submit_output_retries_by_node: Default::default(),
        request_input_retries_by_node: Default::default(),
        empty_turn_retries_by_node: Default::default(),
        mixed_tool_turn_retries_by_node: Default::default(),
        auto_continue_streaks_by_node: Default::default(),
        entrypoint_text: None,
        interrupted_nodes: Default::default(),
        failed_nodes: Default::default(),
        plan_mode_source_node_id: None,
        frozen_change_evidence_packet: None,
    }
}

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
        runtime_config_store: None,
    }
}

fn sample_pending_approval(node_id: &str, approval_id: &str) -> PendingToolApproval {
    PendingToolApproval {
        approval_id: approval_id.to_string(),
        node_id: NodeId(node_id.to_string()),
        node_label: node_id.to_string(),
        tool_call: ToolCall {
            id: "call-1".to_string(),
            name: "write".to_string(),
            arguments: serde_json::json!({ "path": "notes.txt", "content": "hello" }),
        },
        tier: ToolTier::Write,
    }
}

struct LocalStores {
    dir: tempfile::TempDir,
    agent_store: FileAgentStore,
    settings_store: FileSettingsStore,
    run_store: FileRunCheckpointStore,
    settings: AppSettings,
    env: ProviderEnv,
    run_root: RunStoreRoot,
}

fn local_stores() -> LocalStores {
    let dir = tempdir().expect("tempdir");
    let run_root = RunStoreRoot {
        project_id: None,
        root: dir.path().join("runs"),
    };
    LocalStores {
        agent_store: FileAgentStore::new(dir.path().join("agents.json")),
        settings_store: FileSettingsStore::new(dir.path().join("settings.json")),
        run_store: FileRunCheckpointStore,
        settings: AppSettings::default(),
        env: test_env(),
        run_root,
        dir,
    }
}

fn run_start_params<'a>(stores: &'a LocalStores, workflow: Workflow) -> RunStartParams<'a> {
    RunStartParams {
        workflow,
        entrypoint: None,
        execution_cwd: None,
        run_root: stores.run_root.clone(),
        settings: &stores.settings,
        transient_api_key: None,
        agent_store: &stores.agent_store,
        settings_store: &stores.settings_store,
        run_store: &stores.run_store,
        env: &stores.env,
    }
}

// ── session helpers ──────────────────────────────────────────────────────────

#[test]
fn finish_run_session_preserves_durable_artifact_root() {
    let dir = tempdir().expect("tempdir");
    let artifact_root = dir.path().join("artifacts");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::write(artifact_root.join("spill.txt"), "hello").expect("seed artifact");

    let mut session = seeded_session(artifact_root.clone());
    finish_run_session(&mut session);

    assert_eq!(session.artifact_root.as_ref(), Some(&artifact_root));
    assert!(artifact_root.exists());
    assert!(session.handle.is_none());
    assert!(session.action_tx.is_none());
}

#[test]
fn clear_artifact_root_removes_directory() {
    let dir = tempdir().expect("tempdir");
    let artifact_root = dir.path().join("artifacts");
    fs::create_dir_all(&artifact_root).expect("create artifact root");
    fs::write(artifact_root.join("file.txt"), "x").expect("write");

    let mut session = seeded_session(artifact_root.clone());
    clear_artifact_root(&mut session);

    assert!(!artifact_root.exists());
    assert!(session.artifact_root.is_none());
}

#[test]
fn apply_user_stop_to_session_marks_run_aborted() {
    let workflow = default_workflow("Stop");
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.active = true;
    let mut session = RunSession {
        workflow: Some(workflow.clone()),
        run_state: Some(run_state),
        ..seeded_session(PathBuf::from("/tmp/unused"))
    };

    let snapshot = apply_user_stop_to_session(&mut session).expect("snapshot");

    assert!(!snapshot.active);
    assert!(session.engine_checkpoint.is_none());
}

// ── checkpoint helpers ───────────────────────────────────────────────────────

#[test]
fn status_for_checkpoint_maps_pause_and_terminal_reasons() {
    assert_eq!(
        status_for_checkpoint(RunCheckpointReason::AwaitingInput),
        RunStatus::Paused
    );
    assert_eq!(
        status_for_checkpoint(RunCheckpointReason::UserStopped),
        RunStatus::Stopped
    );
    assert_eq!(
        status_for_checkpoint(RunCheckpointReason::Completed),
        RunStatus::Completed
    );
    assert_eq!(
        status_for_checkpoint(RunCheckpointReason::Failed),
        RunStatus::Failed
    );
}

#[test]
fn durable_artifact_root_lives_under_run_directory() {
    let dir = tempdir().expect("tempdir");
    let root = RunStoreRoot {
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
    let original = workflow_hash(&workflow);
    workflow.name = "Changed".to_string();
    assert_ne!(original, workflow_hash(&workflow));
}

// ── read-only queries ────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn get_run_state_and_current_run_id_reflect_session() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    assert!(coordinator.get_run_state().await.is_none());
    assert!(coordinator.current_run_id().await.is_none());

    let workflow = default_workflow("Query");
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some("run-q".to_string());
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            action_tx: Some(action_tx),
            run_id: Some("run-q".to_string()),
            ..empty_seed_fields()
        })
        .await;

    assert_eq!(coordinator.current_run_id().await.as_deref(), Some("run-q"));
    assert!(coordinator.get_run_state().await.is_some_and(|s| s.active));
    assert!(coordinator.is_run_active().await);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn is_run_continuable_requires_stopped_run_with_checkpoint() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    assert!(!coordinator.is_run_continuable().await);

    let workflow = default_workflow("Continue");
    let checkpoint = empty_engine_checkpoint(&workflow);
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.active = false;
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            engine_checkpoint: Some(checkpoint),
            ..empty_seed_fields()
        })
        .await;

    assert!(coordinator.is_run_continuable().await);
}

// ── stop / clear ─────────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn stop_run_is_idempotent_when_inactive() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Stop");
    let run_state = WorkflowRunState::idle_for_workflow(&workflow);
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let snapshot = coordinator.stop_run().await.expect("stop inactive");
    assert!(!snapshot.active);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn stop_run_aborts_orphaned_active_session_without_handle() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Orphan");
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some("orphaned".to_string());
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let stopped = coordinator.stop_run().await.expect("stop orphaned");
    assert!(!stopped.active);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn clear_run_trace_preserves_chat_and_outputs() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Clear");
    let node_id = workflow.nodes[0].id.clone();
    let mut run_state = WorkflowRunState::idle_for_workflow(&workflow);
    run_state.chat_logs.insert(
        node_id.clone(),
        vec![engine::ChatMessage::text(
            engine::ChatRole::User,
            "keep me".to_string(),
        )],
    );
    run_state
        .outputs
        .insert(node_id.clone(), serde_json::json!({ "done": true }));
    let node_id_for_assert = node_id.clone();
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            ..empty_seed_fields()
        })
        .await;

    let cleared = coordinator
        .clear_run_trace()
        .await
        .expect("cleared")
        .expect("snapshot");
    assert!(!cleared.active);
    assert_eq!(
        cleared.chat_logs.values().next().unwrap()[0].content,
        "keep me"
    );
    assert!(cleared.outputs.contains_key(&node_id_for_assert));
}

// ── replay / list ────────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn replay_run_returns_inactive_projection_without_pending_actions() {
    let dir = tempdir().expect("tempdir");
    let store = FileRunCheckpointStore;
    let root = RunStoreRoot {
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
    projection
        .pending_approvals
        .push(sample_pending_approval("node-1", "approval-1"));

    seed_run_checkpoint(&store, &root, &workflow, "run-1", dir.path(), projection);

    let coordinator = coordinator(dir.path());
    let replay = coordinator
        .replay_run(&store, &[root], "run-1")
        .expect("replay");

    assert!(!replay.active);
    assert!(replay.awaiting_node_id.is_none());
    assert!(replay.awaiting_node_ids.is_empty());
    assert!(replay.pending_approvals.is_empty());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn replay_run_errors_when_run_or_checkpoint_missing() {
    let dir = tempdir().expect("tempdir");
    let store = FileRunCheckpointStore;
    let root = RunStoreRoot {
        project_id: None,
        root: dir.path().join("runs"),
    };
    let coordinator = coordinator(dir.path());

    assert!(matches!(
        coordinator.replay_run(&store, std::slice::from_ref(&root), "missing"),
        Err(BackendError::RunNotFound(_))
    ));

    let workflow = Workflow::new("No checkpoint");
    let record = run_record(dir.path(), &workflow, "run-nc");
    store.create_run(&root, &record).expect("create");
    assert!(matches!(
        coordinator.replay_run(&store, &[root], "run-nc"),
        Err(BackendError::RunHasNoCheckpoints(_))
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn list_runs_delegates_to_store() {
    let dir = tempdir().expect("tempdir");
    let store = FileRunCheckpointStore;
    let root = RunStoreRoot {
        project_id: None,
        root: dir.path().join("runs"),
    };
    let workflow = Workflow::new("List");
    let record = run_record(dir.path(), &workflow, "run-list");
    store.create_run(&root, &record).expect("create");

    let coordinator = RunCoordinator::new(tokio::runtime::Handle::current());
    let runs = coordinator.list_runs(&store, &[root], None).expect("list");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_id, "run-list");
}

// ── start / continue / durable resume ────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn start_run_spawns_active_session_and_persists_record() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let mut workflow = default_workflow("Start");
    workflow.settings.reasoning_effort = Some("medium".to_string());

    let (state, event_rx) = coordinator
        .start_run(run_start_params(&stores, workflow.clone()))
        .await
        .expect("start run");

    assert!(state.active);
    let run_id = state.run_id.as_deref().expect("durable run id");
    let (_, record) = stores
        .run_store
        .load_record(std::slice::from_ref(&stores.run_root), run_id)
        .expect("load run record")
        .expect("persisted run record");
    let snapshot = &record.workflow_snapshot;
    assert_eq!(workflow_hash(snapshot), record.workflow_hash);
    assert!(snapshot
        .nodes
        .iter()
        .all(|node| node.agent.reasoning_effort.as_deref() == Some("medium")));
    assert!(coordinator.is_run_active().await);
    drop(event_rx);
    let stopped = coordinator.stop_run().await.expect("stop");
    assert!(!stopped.active);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn continue_run_resumes_from_in_session_checkpoint() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let workflow = default_workflow("Continue");
    let checkpoint = empty_engine_checkpoint(&workflow);
    let resources = fresh_execution_resources(&stores.settings);
    let cwd = stores.dir.path().to_path_buf();
    let artifact_root = stores.dir.path().join("artifacts");
    fs::create_dir_all(&artifact_root).ok();

    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.active = false;
    run_state.run_id = Some("run-cont".to_string());

    coordinator
        .test_seed_full(TestSessionSeed {
            workflow: workflow.clone(),
            run_state,
            run_id: Some("run-cont".to_string()),
            engine_checkpoint: Some(checkpoint),
            execution_cwd: Some(cwd),
            artifact_root: Some(artifact_root),
            snapshot_store: Some(resources.snapshot_store.clone()),
            lsp_settings: Some(resources.lsp_settings.clone()),
            pending_engine_reverts: Some(resources.pending_engine_reverts.clone()),
            ..empty_seed_fields()
        })
        .await;

    let (resumed, event_rx) = coordinator
        .continue_run(run_start_params(&stores, workflow))
        .await
        .expect("continue");

    assert!(resumed.active);
    drop(event_rx);
    let _ = coordinator.stop_run().await;
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn continue_run_rejects_active_or_missing_checkpoint() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let workflow = default_workflow("No continue");

    assert!(matches!(
        coordinator
            .continue_run(run_start_params(&stores, workflow.clone()))
            .await,
        Err(BackendError::NoContinuableRun)
    ));

    let mut active = WorkflowRunState::running_for_workflow(&workflow);
    active.active = true;
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow: workflow.clone(),
            run_state: active,
            engine_checkpoint: Some(empty_engine_checkpoint(&workflow)),
            ..empty_seed_fields()
        })
        .await;
    assert!(matches!(
        coordinator
            .continue_run(run_start_params(&stores, workflow.clone()))
            .await,
        Err(BackendError::NoContinuableRun)
    ));

    let mut stopped = WorkflowRunState::running_for_workflow(&workflow);
    stopped.active = false;
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow: workflow.clone(),
            run_state: stopped,
            engine_checkpoint: Some(empty_engine_checkpoint(&workflow)),
            ..empty_seed_fields()
        })
        .await;
    let mut other = default_workflow("Other");
    other.name = "Other".to_string();
    assert!(matches!(
        coordinator
            .continue_run(run_start_params(&stores, other))
            .await,
        Err(BackendError::CheckpointWorkflowMismatch)
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn resume_durable_run_uses_recorded_workflow_snapshot() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let workflow = default_workflow("Durable snapshot");
    let record = run_record(stores.dir.path(), &workflow, "run-snapshot");
    let checkpoint =
        durable_checkpoint(&workflow, WorkflowRunState::running_for_workflow(&workflow));
    stores
        .run_store
        .create_run(&stores.run_root, &record)
        .expect("create run");

    let (resumed, event_rx) = coordinator
        .resume_durable_run(DurableResumeParams {
            run_id: "run-snapshot",
            root: stores.run_root.clone(),
            record,
            checkpoint,
            settings: &stores.settings,
            transient_api_key: None,
            agent_store: &stores.agent_store,
            settings_store: &stores.settings_store,
            run_store: &stores.run_store,
            env: &stores.env,
        })
        .await
        .expect("resume from recorded workflow snapshot");

    assert!(resumed.active);
    drop(event_rx);
    let _ = coordinator.stop_run().await;
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn resume_durable_run_restores_active_session() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let workflow = default_workflow("Durable resume");
    let record = run_record(stores.dir.path(), &workflow, "run-dr");
    stores
        .run_store
        .create_run(&stores.run_root, &record)
        .expect("create");

    let mut projection = WorkflowRunState::running_for_workflow(&workflow);
    projection.active = false;
    projection.run_id = Some("run-dr".to_string());
    let checkpoint = durable_checkpoint(&workflow, projection);

    stores
        .run_store
        .append_checkpoint(&stores.run_root, "run-dr", &checkpoint)
        .expect("checkpoint");

    let (resumed, event_rx) = coordinator
        .resume_durable_run(DurableResumeParams {
            run_id: "run-dr",
            root: stores.run_root.clone(),
            record,
            checkpoint,
            settings: &stores.settings,
            transient_api_key: None,
            agent_store: &stores.agent_store,
            settings_store: &stores.settings_store,
            run_store: &stores.run_store,
            env: &stores.env,
        })
        .await
        .expect("resume");

    assert!(resumed.active);
    drop(event_rx);
    let _ = coordinator.stop_run().await;
}

// ── interaction ──────────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn submit_user_input_appends_chat_and_sends_action() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Input");
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.awaiting_node_id = Some(NodeId("idea".to_string()));
    run_state.awaiting_node_ids = vec![NodeId("idea".to_string())];
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let run_state = coordinator
        .submit_user_input("idea", "hello".to_string())
        .await
        .expect("submit");

    assert_eq!(run_state.awaiting_node_id, Some(NodeId("idea".to_string())));
    assert_eq!(
        run_state
            .chat_logs
            .get(&NodeId("idea".to_string()))
            .unwrap()[0]
            .content,
        "hello"
    );
    match action_rx.recv().await.expect("action") {
        ExecutionAction::ProvideInput { node_id, text } => {
            assert_eq!(node_id, NodeId("idea".to_string()));
            assert_eq!(text, "hello");
        }
        ExecutionAction::Stop
        | ExecutionAction::ResolveApproval { .. }
        | ExecutionAction::RetryNode { .. } => {
            panic!("unexpected action")
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn submit_user_input_validates_awaiting_node() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Input");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.awaiting_node_id = Some(NodeId("idea".to_string()));
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    assert!(matches!(
        coordinator
            .submit_user_input("other", "nope".to_string())
            .await,
        Err(BackendError::WrongAwaitingNode { .. })
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn submit_tool_approval_sends_resolve_action() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Approve");
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state
        .pending_approvals
        .push(sample_pending_approval("idea", "approval-1"));
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let run_state = coordinator
        .submit_tool_approval("approval-1", true, None)
        .await
        .expect("approve");
    assert_eq!(run_state.pending_approvals.len(), 1);

    match action_rx.recv().await.expect("action") {
        ExecutionAction::ResolveApproval {
            approval_id,
            allow,
            reason,
        } => {
            assert_eq!(approval_id, "approval-1");
            assert!(allow);
            assert!(reason.is_none());
        }
        ExecutionAction::Stop
        | ExecutionAction::ProvideInput { .. }
        | ExecutionAction::RetryNode { .. } => {
            panic!("unexpected action")
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn submit_tool_approval_rejects_unknown_id() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Approve");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state
        .pending_approvals
        .push(sample_pending_approval("idea", "approval-1"));
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    assert!(matches!(
        coordinator.submit_tool_approval("wrong", true, None).await,
        Err(BackendError::WrongApprovalId { .. })
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn interrupt_node_cancels_registered_token() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Interrupt");
    let node_id = workflow.nodes[0].id.clone();
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state
        .status_by_node
        .insert(node_id.clone(), AgentStatus::Started);
    let token = CancellationToken::new();
    let node_interrupts: NodeInterrupts = Arc::new(parking_lot::Mutex::new(BTreeMap::from([(
        node_id.clone(),
        (0u8, token.clone()),
    )])));
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            action_tx: Some(action_tx),
            node_interrupts: Some(node_interrupts),
            ..empty_seed_fields()
        })
        .await;

    coordinator
        .interrupt_node(&node_id.0)
        .await
        .expect("interrupt");
    assert!(token.is_cancelled());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn interrupt_node_rejects_non_running_nodes() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Interrupt");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let run_state = WorkflowRunState::running_for_workflow(&workflow);
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    assert!(matches!(
        coordinator.interrupt_node("idea").await,
        Err(BackendError::NodeNotInterruptible(_))
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn retry_node_sends_action_for_failed_node() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Retry");
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state
        .status_by_node
        .insert(NodeId("idea".to_string()), AgentStatus::Failed);
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    coordinator.retry_node("idea").await.expect("retry");
    match action_rx.recv().await.expect("action") {
        ExecutionAction::RetryNode { node_id } => {
            assert_eq!(node_id, NodeId("idea".to_string()));
        }
        ExecutionAction::Stop
        | ExecutionAction::ProvideInput { .. }
        | ExecutionAction::ResolveApproval { .. } => {
            panic!("unexpected action")
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn retry_node_rejects_non_failed_nodes() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Retry");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let run_state = WorkflowRunState::running_for_workflow(&workflow);
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    assert!(matches!(
        coordinator.retry_node("idea").await,
        Err(BackendError::NodeNotRetryable(_))
    ));
}

// ── execution events ────────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn apply_execution_event_ignores_events_after_run_stopped() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let store = FileRunCheckpointStore;
    let workflow = default_workflow("Events");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some("stopped".to_string());
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let stopped = coordinator.stop_run().await.expect("stop");
    assert!(!stopped.active);

    let snapshot = coordinator
        .apply_execution_event(
            ExecutionEvent::NodeQueued {
                node_id: NodeId("idea".to_string()),
                label: "Idea".to_string(),
            },
            &store,
        )
        .await
        .expect("ignored");

    assert!(!snapshot.active);
    assert!(snapshot.run_trace.is_empty());
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn apply_execution_event_finishes_session_on_run_complete() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let store = FileRunCheckpointStore;
    let workflow = default_workflow("Complete");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let run_state = WorkflowRunState::running_for_workflow(&workflow);
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let snapshot = coordinator
        .apply_execution_event(ExecutionEvent::Aborted, &store)
        .await
        .expect("complete");

    assert!(!snapshot.active);
    assert!(!coordinator.is_run_active().await);
}

// ── edit / git helpers ───────────────────────────────────────────────────────

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn git_diff_file_requires_execution_cwd() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Git");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    coordinator
        .test_seed_session(
            workflow.clone(),
            WorkflowRunState::running_for_workflow(&workflow),
            action_tx,
        )
        .await;

    assert!(matches!(
        coordinator.git_diff_file("README.md".to_string()).await,
        Err(BackendError::NoExecutionCwd)
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn preview_file_edit_rejects_mismatched_tool_name() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Preview");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state
        .pending_approvals
        .push(sample_pending_approval("idea", "approval-1"));
    let resources = fresh_execution_resources(&AppSettings::default());
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            action_tx: Some(action_tx),
            execution_cwd: Some(dir.path().to_path_buf()),
            snapshot_store: Some(resources.snapshot_store),
            ..empty_seed_fields()
        })
        .await;

    assert!(matches!(
        coordinator
            .preview_file_edit("approval-1", "read".to_string(), serde_json::json!({}))
            .await,
        Err(BackendError::PreviewFailed(_))
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn revert_edit_batch_requires_known_batch() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Revert");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow: workflow.clone(),
            run_state: WorkflowRunState::running_for_workflow(&workflow),
            action_tx: Some(action_tx),
            execution_cwd: Some(dir.path().to_path_buf()),
            ..empty_seed_fields()
        })
        .await;

    assert!(matches!(
        coordinator.revert_edit_batch("missing".to_string()).await,
        Err(BackendError::EditBatchNotFound(_))
    ));
}

// ── test helpers ─────────────────────────────────────────────────────────────

fn empty_seed_fields() -> TestSessionSeed {
    TestSessionSeed {
        workflow: Workflow::new("placeholder"),
        run_state: WorkflowRunState::running_for_workflow(&Workflow::new("placeholder")),
        action_tx: None,
        run_id: None,
        run_root: None,
        project_id: None,
        execution_cwd: None,
        entrypoint: None,
        artifact_root: None,
        engine_checkpoint: None,
        checkpoint_sink: None,
        snapshot_store: None,
        lsp_settings: None,
        pending_engine_reverts: None,
        node_interrupts: None,
        runtime_config_store: None,
        cancel_token: None,
        handle: None,
    }
}

fn run_record(dir: &Path, workflow: &Workflow, run_id: &str) -> RunRecord {
    RunRecord {
        run_id: run_id.to_string(),
        workflow_id: workflow.id.to_string(),
        workflow_name: workflow.name.clone(),
        workflow_hash: workflow_hash(workflow),
        workflow_snapshot: workflow.clone(),
        project_id: None,
        execution_cwd: dir.display().to_string(),
        artifact_root: dir
            .join(format!("runs/{run_id}/artifacts"))
            .display()
            .to_string(),
        started_at_ms: 1,
        updated_at_ms: 1,
        status: RunStatus::Paused,
    }
}

fn durable_checkpoint(workflow: &Workflow, projection: WorkflowRunState) -> RunCheckpointPayload {
    RunCheckpointPayload {
        seq: 1,
        created_at_ms: 1,
        reason: RunCheckpointReason::AwaitingInput,
        engine: empty_engine_checkpoint(workflow),
        projection,
    }
}

fn seed_run_checkpoint(
    store: &FileRunCheckpointStore,
    root: &RunStoreRoot,
    workflow: &Workflow,
    run_id: &str,
    dir: &Path,
    projection: WorkflowRunState,
) {
    store
        .create_run(root, &run_record(dir, workflow, run_id))
        .expect("create run");
    store
        .append_checkpoint(root, run_id, &durable_checkpoint(workflow, projection))
        .expect("checkpoint");
}

//! Coordinator unit and integration tests.

use super::checkpoint::status_for_checkpoint;
use super::session::{
    apply_user_stop_to_session, clear_artifact_root, finish_run_session, fresh_execution_resources,
    prepare_workflow_run, RunSession,
};
use super::{DurableResumeParams, RunCoordinator, RunStartParams, TestSessionSeed};
use crate::adapters::storage::agent_store::FileAgentStore;
use crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
use crate::adapters::storage::settings_store::FileSettingsStore;
use crate::adapters::storage::skill_store::FileSkillCatalog;
use crate::api::{DurableRunContinuationInput, UserMessageInput};
use crate::error::BackendError;
use crate::mcp::client_capabilities::McpClientRequestDecision;
use crate::run::execution::{ExecutionAction, ExecutionEvent, NodeInterrupts};
use crate::run::persistence::{
    workflow_hash, PendingRunCheckpoint, RunCheckpointPayload, RunCheckpointReason, RunRecord,
    RunStatus, RunStoreRoot,
};
use crate::run::ports::RunCheckpointStore;
use crate::run::state::{AgentStatus, WorkflowRunState};
use crate::settings::model::AppSettings;
use crate::settings::provider::ProviderConfigError;
use crate::settings::provider::ProviderEnv;
use crate::workflow::catalog::default_workflow;
use engine::{
    InteractiveEngineCheckpoint, McpClientRequestKind, NodeId, PendingMcpClientRequest,
    PendingToolApproval, ToolCall, ToolTier, Workflow,
};
use image::{ImageFormat, Rgba, RgbaImage};
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
        handoffs: Default::default(),
        changed_files_by_node: Default::default(),
        reads_by_node: Default::default(),
        transcripts: Default::default(),
        awaiting_nodes: Default::default(),
        structured_input_by_node: Default::default(),
        pending_tool_batches: Default::default(),
        retries_by_node: Default::default(),
        transient_streaks_by_node: Default::default(),
        submit_output_retries_by_node: Default::default(),
        request_input_retries_by_node: Default::default(),
        empty_turn_retries_by_node: Default::default(),
        mixed_tool_turn_retries_by_node: Default::default(),
        output_truncation_retries_by_node: Default::default(),
        auto_continue_streaks_by_node: Default::default(),
        entrypoint_text: None,
        entrypoint_attachments: Vec::new(),
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
        skill_paths: Default::default(),
        execution_cwd: None,
        entrypoint: None,
        entrypoint_attachments: Vec::new(),
        artifact_root: Some(artifact_root),
        attachment_root: None,
        generation: 0,
        engine_checkpoint: None,
        active: None,
    }
}

fn sample_pending_approval(node_id: &str, approval_id: &str) -> PendingToolApproval {
    PendingToolApproval {
        approval_id: approval_id.to_string(),
        node_id: NodeId(node_id.to_string()),
        node_label: node_id.to_string(),
        tool_call: ToolCall {
            id: "call-1".to_string(),
            provider_call_id: None,
            name: "write".to_string(),
            arguments: serde_json::json!({ "path": "notes.txt", "content": "hello" }),
        },
        tier: ToolTier::Write,
    }
}

struct LocalStores {
    dir: tempfile::TempDir,
    agent_store: FileAgentStore,
    settings_store: Arc<FileSettingsStore>,
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
        settings_store: Arc::new(FileSettingsStore::new(dir.path().join("settings.json"))),
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
        invoked_skill_ids: Vec::new(),
        entrypoint: None,
        execution_cwd: None,
        run_root: stores.run_root.clone(),
        settings: &stores.settings,
        transient_api_key: None,
        agent_store: &stores.agent_store,
        skill_catalog: &FileSkillCatalog,
        settings_store: stores.settings_store.clone(),
        run_store: &stores.run_store,
        env: &stores.env,
    }
}

// ── session helpers ──────────────────────────────────────────────────────────

#[test]
fn prepare_workflow_run_requires_credentials_for_each_node_provider() {
    let stores = local_stores();
    let mut workflow = default_workflow("Mixed providers");
    workflow.settings.provider_id = Some("openai".to_string());
    workflow.nodes[0].agent.provider_id = Some("anthropic".to_string());

    let result = prepare_workflow_run(
        workflow,
        &[],
        &stores.settings,
        None,
        &stores.agent_store,
        &FileSkillCatalog,
        stores.settings_store.clone(),
        &stores.env,
        Arc::new(crate::run::resources::SharedRunResources::default()),
    );

    assert!(matches!(
        result,
        Err(BackendError::ProviderConfig(
            ProviderConfigError::MissingApiKey { provider, env_var }
        )) if provider == "Anthropic" && env_var == "ANTHROPIC_API_KEY"
    ));
}

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
    assert!(session.active.is_none());
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
        status_for_checkpoint(RunCheckpointReason::Started),
        RunStatus::Running
    );
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
        vec![engine::ChatMessage::text(engine::ChatRole::User, "keep me".to_string()).into()],
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

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn clear_run_trace_rejects_an_active_run() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Clear active");
    let (action_tx, _) = tokio::sync::mpsc::unbounded_channel();

    coordinator
        .test_seed_session(
            workflow.clone(),
            WorkflowRunState::running_for_workflow(&workflow),
            action_tx,
        )
        .await;

    assert!(matches!(
        coordinator.clear_run_trace().await,
        Err(BackendError::ActiveRun)
    ));
    assert!(coordinator.is_run_active().await);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn aborted_event_preserves_user_stopped_checkpoint_for_continue() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let store = FileRunCheckpointStore;
    let workflow = default_workflow("Stop race");
    let checkpoint = empty_engine_checkpoint(&workflow);
    let sink = Arc::new(parking_lot::Mutex::new(Some(PendingRunCheckpoint {
        reason: RunCheckpointReason::UserStopped,
        engine: checkpoint,
    })));
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some("stop-race".to_string());

    coordinator
        .test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            checkpoint_sink: Some(sink),
            ..empty_seed_fields()
        })
        .await;

    coordinator
        .apply_execution_event(ExecutionEvent::Aborted, &store)
        .await
        .expect("apply aborted event");

    assert!(coordinator.is_run_continuable().await);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn stop_run_persists_user_stopped_checkpoint_before_returning() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let store = FileRunCheckpointStore;
    let root = RunStoreRoot {
        project_id: None,
        root: dir.path().join("runs"),
    };
    let workflow = default_workflow("Persist stop");
    let run_id = "persist-stop";
    store
        .create_run(&root, &run_record(dir.path(), &workflow, run_id))
        .expect("create run");
    let mut engine_checkpoint = empty_engine_checkpoint(&workflow);
    engine_checkpoint
        .interrupted_nodes
        .insert(workflow.nodes[0].id.clone());
    let sink = Arc::new(parking_lot::Mutex::new(Some(PendingRunCheckpoint {
        reason: RunCheckpointReason::UserStopped,
        engine: engine_checkpoint,
    })));
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some(run_id.to_string());

    coordinator
        .test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            run_id: Some(run_id.to_string()),
            run_root: Some(root.clone()),
            checkpoint_sink: Some(sink),
            ..empty_seed_fields()
        })
        .await;

    let stopped = coordinator
        .stop_run_and_persist(&store)
        .await
        .expect("stop and persist");

    assert!(!stopped.active);
    let checkpoint = store
        .load_latest_checkpoint(&root, run_id)
        .expect("load checkpoint")
        .expect("persisted checkpoint");
    assert_eq!(checkpoint.reason, RunCheckpointReason::UserStopped);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn checkpoint_waits_for_pause_projection_before_persisting() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let store = FileRunCheckpointStore;
    let root = RunStoreRoot {
        project_id: None,
        root: dir.path().join("runs"),
    };
    let workflow = default_workflow("Checkpoint ordering");
    let node_id = workflow.nodes[0].id.clone();
    let run_id = "checkpoint-ordering";
    store
        .create_run(&root, &run_record(dir.path(), &workflow, run_id))
        .expect("create run");

    let mut engine_checkpoint = empty_engine_checkpoint(&workflow);
    engine_checkpoint.awaiting_nodes.insert(node_id.clone());
    let sink = Arc::new(parking_lot::Mutex::new(Some(PendingRunCheckpoint {
        reason: RunCheckpointReason::AwaitingInput,
        engine: engine_checkpoint,
    })));
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some(run_id.to_string());
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow: workflow.clone(),
            run_state,
            run_id: Some(run_id.to_string()),
            run_root: Some(root.clone()),
            checkpoint_sink: Some(sink),
            ..empty_seed_fields()
        })
        .await;

    coordinator
        .apply_execution_event(
            ExecutionEvent::NodeStarted {
                node_id: node_id.clone(),
                label: "Chat".to_string(),
            },
            &store,
        )
        .await
        .expect("apply start");
    coordinator
        .apply_execution_event(
            ExecutionEvent::ChatMessage {
                node_id: node_id.clone(),
                role: engine::ChatRole::Assistant,
                content: "What are you looking forward to this week?".to_string(),
            },
            &store,
        )
        .await
        .expect("apply assistant reply");
    coordinator
        .apply_execution_event(
            ExecutionEvent::NodeAwaitingInput {
                node_id: node_id.clone(),
                label: "Chat".to_string(),
                context: String::new(),
                is_initial: false,
                structured_input: None,
            },
            &store,
        )
        .await
        .expect("apply pause");

    let checkpoint = store
        .load_latest_checkpoint(&root, run_id)
        .expect("load checkpoint")
        .expect("checkpoint");
    assert_eq!(
        checkpoint.projection.awaiting_node_id,
        Some(node_id.clone())
    );
    assert_eq!(
        checkpoint
            .projection
            .chat_logs
            .get(&node_id)
            .and_then(|messages| messages.last())
            .map(|message| message.content.as_str()),
        Some("What are you looking forward to this week?")
    );
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
async fn start_run_persists_natural_language_name_from_entrypoint() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let workflow = default_workflow("Named run");
    let mut params = run_start_params(&stores, workflow);
    params.entrypoint = Some(UserMessageInput::text(
        "  Audit   provider retries in the workflow  ",
    ));

    let (state, event_rx) = coordinator.start_run(params).await.expect("start run");
    let run_id = state.run_id.as_deref().expect("durable run id");
    let (_, record) = stores
        .run_store
        .load_record(std::slice::from_ref(&stores.run_root), run_id)
        .expect("load run record")
        .expect("persisted run record");

    assert_eq!(
        record.name.as_deref(),
        Some("Audit provider retries in the workflow")
    );

    drop(event_rx);
    coordinator.stop_run().await.expect("stop run");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn start_run_resolves_task_prompt_skill_into_durable_workflow_snapshot() {
    let mut stores = local_stores();
    let skill_root = stores.dir.path().join("skills");
    let skill_path = skill_root.join("tdd").join("SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("create skill");
    fs::write(
        &skill_path,
        "---\nname: tdd\ndescription: Test first.\n---\n\n# TDD",
    )
    .expect("write skill");
    stores.settings.skill_search_paths = vec![skill_root.display().to_string()];

    let coordinator = coordinator(stores.dir.path());
    let mut workflow = default_workflow("Skill invocation");
    workflow.nodes[0].agent.task_prompt = "/tdd Implement the ticket.".to_string();

    let (state, event_rx) = coordinator
        .start_run(run_start_params(&stores, workflow))
        .await
        .expect("start run");

    let run_id = state.run_id.as_deref().expect("durable run id");
    let (_, record) = stores
        .run_store
        .load_record(std::slice::from_ref(&stores.run_root), run_id)
        .expect("load run record")
        .expect("persisted run record");
    let system_prompt = &record.workflow_snapshot.nodes[0].agent.system_prompt;
    assert!(system_prompt.contains("--- Invoked skills ---"));
    assert!(system_prompt.contains(&format!("/tdd: {}", skill_path.display())));
    assert!(system_prompt.contains("# TDD"));

    drop(event_rx);
    let _ = coordinator.stop_run().await;
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn start_run_persists_mcp_context_resolution_once_in_durable_workflow_snapshot() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let mut workflow = default_workflow("MCP context snapshot");
    workflow.nodes[0]
        .agent
        .mcp_resources
        .push(engine::McpResourceSelection {
            server_id: "missing-docs".to_string(),
            uri: "docs://guide".to_string(),
            max_bytes: 4096,
        });

    let (state, event_rx) = coordinator
        .start_run(run_start_params(&stores, workflow))
        .await
        .expect("start run");
    let run_id = state.run_id.as_deref().expect("durable run id");
    let (_, record) = stores
        .run_store
        .load_record(std::slice::from_ref(&stores.run_root), run_id)
        .expect("load run record")
        .expect("persisted run record");
    let snapshots = &record.workflow_snapshot.nodes[0]
        .agent
        .mcp_context_snapshots;

    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].server_id, "missing-docs");
    assert_eq!(snapshots[0].source, "docs://guide");
    assert!(snapshots[0]
        .error
        .as_deref()
        .is_some_and(|error| error.contains("not connected")));
    assert_eq!(
        workflow_hash(&record.workflow_snapshot),
        record.workflow_hash
    );

    drop(event_rx);
    let _ = coordinator.stop_run().await;
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn start_run_applies_chat_skill_ids_to_root_system_prompt() {
    let mut stores = local_stores();
    let skill_root = stores.dir.path().join("skills");
    let skill_path = skill_root.join("tdd").join("SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("create skill");
    fs::write(&skill_path, "# TDD\n\nWrite the test first.").expect("write skill");
    stores.settings.skill_search_paths = vec![skill_root.display().to_string()];

    let coordinator = coordinator(stores.dir.path());
    let workflow = default_workflow("Chat skill invocation");
    let mut params = run_start_params(&stores, workflow);
    params.entrypoint = Some(UserMessageInput::text("Please inspect the ticket"));
    params.invoked_skill_ids = vec!["tdd".to_string()];

    let (state, event_rx) = coordinator.start_run(params).await.expect("start run");
    let run_id = state.run_id.as_deref().expect("durable run id");
    let (_, record) = stores
        .run_store
        .load_record(std::slice::from_ref(&stores.run_root), run_id)
        .expect("load run record")
        .expect("persisted run record");
    let system_prompt = &record.workflow_snapshot.nodes[0].agent.system_prompt;
    assert!(system_prompt.contains("/tdd:"));
    assert!(system_prompt.contains("Write the test first."));

    drop(event_rx);
    let _ = coordinator.stop_run().await;
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn start_run_ingests_entrypoint_attachment_into_projection_and_checkpoint() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let source = stores.dir.path().join("kickoff.png");
    write_png(&source);
    let workflow = default_workflow("Attachment kickoff");
    let mut params = run_start_params(&stores, workflow);
    params.entrypoint = Some(UserMessageInput {
        text: String::new(),
        attachment_source_paths: vec![source.display().to_string()],
    });

    let (state, event_rx) = coordinator.start_run(params).await.expect("start run");

    let run_id = state.run_id.as_deref().expect("run id");
    let attachment = state
        .chat_logs
        .values()
        .flatten()
        .flat_map(|message| message.attachments.iter())
        .next()
        .expect("projected attachment");
    assert_eq!(attachment.file_name, "kickoff.png");
    let checkpoint = stores
        .run_store
        .load_latest_checkpoint(&stores.run_root, run_id)
        .expect("load checkpoint")
        .expect("checkpoint");
    assert_eq!(
        checkpoint.engine.entrypoint_attachments,
        vec![attachment.clone()]
    );
    assert!(stores
        .run_store
        .run_dir(&stores.run_root, run_id)
        .join("attachments")
        .join(format!("{}.png", attachment.id))
        .exists());

    drop(event_rx);
    let _ = coordinator.stop_run().await;
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn start_run_rejects_unknown_task_prompt_skill() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let mut workflow = default_workflow("Missing skill");
    workflow.nodes[0].agent.task_prompt = "/not-installed Implement the ticket.".to_string();

    let error = coordinator
        .start_run(run_start_params(&stores, workflow))
        .await
        .expect_err("missing skill");

    assert_eq!(
        error.to_string(),
        "skill /not-installed invoked by node \"Idea\" is not installed"
    );
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
            skill_catalog: &FileSkillCatalog,
            settings_store: stores.settings_store.clone(),
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
            skill_catalog: &FileSkillCatalog,
            settings_store: stores.settings_store.clone(),
            run_store: &stores.run_store,
            env: &stores.env,
        })
        .await
        .expect("resume");

    assert!(resumed.active);
    drop(event_rx);
    let _ = coordinator.stop_run().await;
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn resume_durable_run_with_continuation_projects_user_message() {
    let stores = local_stores();
    let coordinator = coordinator(stores.dir.path());
    let workflow = default_workflow("Durable continuation");
    let node_id = workflow.nodes[0].id.clone();
    let attachment_path = stores.dir.path().join("continuation.png");
    write_png(&attachment_path);
    let record = run_record(stores.dir.path(), &workflow, "run-message");
    stores
        .run_store
        .create_run(&stores.run_root, &record)
        .expect("create");

    let mut projection = WorkflowRunState::running_for_workflow(&workflow);
    projection.active = false;
    projection.run_id = Some("run-message".to_string());
    projection
        .status_by_node
        .insert(node_id.clone(), AgentStatus::Stopped);
    let mut checkpoint = durable_checkpoint(&workflow, projection);
    checkpoint.engine.interrupted_nodes.insert(node_id.clone());

    let (resumed, event_rx) = coordinator
        .resume_durable_run_with_continuation(
            DurableResumeParams {
                run_id: "run-message",
                root: stores.run_root.clone(),
                record,
                checkpoint,
                settings: &stores.settings,
                transient_api_key: None,
                agent_store: &stores.agent_store,
                skill_catalog: &FileSkillCatalog,
                settings_store: stores.settings_store.clone(),
                run_store: &stores.run_store,
                env: &stores.env,
            },
            Some(DurableRunContinuationInput {
                node_id: node_id.0.clone(),
                text: "Continue with verification".to_string(),
                invoked_skill_ids: Vec::new(),
                attachment_source_paths: vec![attachment_path.display().to_string()],
            }),
        )
        .await
        .expect("resume with message");

    assert!(resumed.active);
    let message = resumed
        .chat_logs
        .get(&node_id)
        .and_then(|messages| messages.last())
        .expect("continuation message");
    assert_eq!(message.content, "Continue with verification");
    assert_eq!(message.attachments.len(), 1);
    assert_eq!(
        resumed.status_by_node.get(&node_id),
        Some(&AgentStatus::Started)
    );
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
    run_state.structured_input_by_node.insert(
        NodeId("idea".to_string()),
        engine::StructuredUserInput { questions: vec![] },
    );
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let run_state = coordinator
        .submit_user_input("idea", "hello".to_string())
        .await
        .expect("submit");

    assert!(run_state.awaiting_node_id.is_none());
    assert!(run_state.awaiting_node_ids.is_empty());
    assert!(!run_state
        .structured_input_by_node
        .contains_key(&NodeId("idea".to_string())));
    assert_eq!(
        run_state
            .chat_logs
            .get(&NodeId("idea".to_string()))
            .unwrap()[0]
            .content,
        "hello"
    );
    match action_rx.recv().await.expect("action") {
        ExecutionAction::ProvideInput {
            node_id,
            text,
            attachments,
            skill_prompt,
        } => {
            assert_eq!(node_id, NodeId("idea".to_string()));
            assert_eq!(text, "hello");
            assert!(attachments.is_empty());
            assert!(skill_prompt.is_none());
        }
        ExecutionAction::Stop
        | ExecutionAction::ResolveApproval { .. }
        | ExecutionAction::RetryNode { .. }
        | ExecutionAction::ResolveMcpClientRequest { .. } => {
            panic!("unexpected action")
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn submit_user_input_retries_failed_node_with_new_message() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("Retry with input");
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state
        .status_by_node
        .insert(NodeId("idea".to_string()), AgentStatus::Failed);
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let run_state = coordinator
        .submit_user_input("idea", "Try again".to_string())
        .await
        .expect("submit failed-node input");

    assert_eq!(
        run_state.status_by_node[&NodeId("idea".to_string())],
        AgentStatus::Started
    );
    match action_rx.recv().await.expect("action") {
        ExecutionAction::ProvideInput { node_id, text, .. } => {
            assert_eq!(node_id, NodeId("idea".to_string()));
            assert_eq!(text, "Try again");
        }
        ExecutionAction::Stop
        | ExecutionAction::ResolveApproval { .. }
        | ExecutionAction::RetryNode { .. }
        | ExecutionAction::ResolveMcpClientRequest { .. } => {
            panic!("unexpected action")
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn submit_user_input_accepts_active_conversation_node() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let mut workflow = default_workflow("Live input");
    let node = workflow.nodes.first_mut().expect("workflow node");
    node.id = NodeId::from("idea");
    node.agent.request_user_input = true;
    node.agent.conversation_mode = true;
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state
        .status_by_node
        .insert(NodeId::from("idea"), AgentStatus::RunningTool);
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    let run_state = coordinator
        .submit_user_input("idea", "while the tool runs".to_string())
        .await
        .expect("submit live input");

    assert_eq!(
        run_state.chat_logs[&NodeId::from("idea")][0].content,
        "while the tool runs"
    );
    assert!(matches!(
        action_rx.recv().await.expect("action"),
        ExecutionAction::ProvideInput { node_id, text, .. }
            if node_id.0 == "idea" && text == "while the tool runs"
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn submit_user_message_copies_attachment_and_sends_same_ref_atomically() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let source = dir.path().join("reply.png");
    write_png(&source);
    let workflow = default_workflow("Attachment reply");
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some("run-reply".to_string());
    run_state.awaiting_node_id = Some(NodeId("idea".to_string()));
    run_state.awaiting_node_ids = vec![NodeId("idea".to_string())];
    coordinator
        .test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            action_tx: Some(action_tx),
            run_id: Some("run-reply".to_string()),
            artifact_root: Some(dir.path().join("run-reply/artifacts")),
            ..empty_seed_fields()
        })
        .await;

    let state = coordinator
        .submit_user_message_with_skill_ids(
            "idea",
            UserMessageInput {
                text: "What is shown?".to_string(),
                attachment_source_paths: vec![source.display().to_string()],
            },
            &[],
        )
        .await
        .expect("submit attachment");

    let projected = &state.chat_logs[&NodeId("idea".to_string())][0].attachments[0];
    match action_rx.recv().await.expect("action") {
        ExecutionAction::ProvideInput {
            text,
            attachments,
            skill_prompt,
            ..
        } => {
            assert_eq!(text, "What is shown?");
            assert_eq!(attachments, vec![projected.clone()]);
            assert!(skill_prompt.is_none());
        }
        _ => panic!("unexpected action"),
    }
    assert!(dir
        .path()
        .join("run-reply/attachments")
        .join(format!("{}.png", projected.id))
        .exists());
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
        | ExecutionAction::RetryNode { .. }
        | ExecutionAction::ResolveMcpClientRequest { .. } => {
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
async fn resolve_mcp_client_request_rejects_stale_id_then_sends_matching_action() {
    let dir = tempdir().expect("tempdir");
    let coordinator = coordinator(dir.path());
    let workflow = default_workflow("MCP callback");
    let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state
        .pending_mcp_client_requests
        .push(PendingMcpClientRequest {
            request_id: "current-request".to_string(),
            server_id: "github".to_string(),
            node_id: "idea".into(),
            tool_call_id: "tool-call-1".to_string(),
            tool_name: "mcp_6_github_search".to_string(),
            kind: McpClientRequestKind::Sampling,
            message: "Approve sampling".to_string(),
            requested_schema: None,
            url: None,
            max_tokens: Some(32),
        });
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;
    let decision = McpClientRequestDecision {
        allow: false,
        content: None,
    };

    assert!(matches!(
        coordinator
            .resolve_mcp_client_request("stale-request", decision.clone())
            .await,
        Err(BackendError::WrongMcpClientRequestId { .. })
    ));
    assert!(matches!(
        action_rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));

    coordinator
        .resolve_mcp_client_request("current-request", decision)
        .await
        .expect("resolve current request");
    match action_rx.recv().await.expect("action") {
        ExecutionAction::ResolveMcpClientRequest {
            request_id,
            decision,
        } => {
            assert_eq!(request_id, "current-request");
            assert!(!decision.allow);
        }
        other => panic!("unexpected action: {other:?}"),
    }
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
        | ExecutionAction::ResolveApproval { .. }
        | ExecutionAction::ResolveMcpClientRequest { .. } => {
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

fn write_png(path: &Path) {
    RgbaImage::from_pixel(2, 2, Rgba([20, 40, 60, 255]))
        .save_with_format(path, ImageFormat::Png)
        .expect("write png");
}

fn run_record(dir: &Path, workflow: &Workflow, run_id: &str) -> RunRecord {
    RunRecord {
        run_id: run_id.to_string(),
        name: Some(format!("{} run", workflow.name)),
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

#[test]
fn direct_chat_checkpoint_migration_discards_only_structured_input() {
    let workflow = default_workflow("Direct chat checkpoint");
    let node_id = workflow.nodes[0].id.clone();
    let structured_input = engine::StructuredUserInput {
        questions: Vec::new(),
    };
    let mut projection = WorkflowRunState::running_for_workflow(&workflow);
    projection.awaiting_node_id = Some(node_id.clone());
    projection.awaiting_node_ids = vec![node_id.clone()];
    projection
        .structured_input_by_node
        .insert(node_id.clone(), structured_input.clone());
    let mut checkpoint = durable_checkpoint(&workflow, projection);
    checkpoint.engine.awaiting_nodes.insert(node_id.clone());
    checkpoint
        .engine
        .structured_input_by_node
        .insert(node_id.clone(), structured_input);

    checkpoint.discard_structured_user_input();

    assert!(checkpoint.engine.structured_input_by_node.is_empty());
    assert!(checkpoint.projection.structured_input_by_node.is_empty());
    assert!(checkpoint.engine.awaiting_nodes.contains(&node_id));
    assert_eq!(
        checkpoint.projection.awaiting_node_id,
        Some(node_id.clone())
    );
    assert_eq!(checkpoint.projection.awaiting_node_ids, vec![node_id]);
}

#[test]
fn direct_chat_checkpoint_repairs_missing_assistant_message_in_transcript_order() {
    let workflow = default_workflow("Direct chat repair");
    let node_id = workflow.nodes[0].id.clone();
    let mut projection = WorkflowRunState::running_for_workflow(&workflow);
    projection.chat_logs.insert(
        node_id.clone(),
        vec![
            engine::ChatMessage::text(engine::ChatRole::User, "Ask me a question".to_string())
                .into(),
            engine::ChatMessage::text(engine::ChatRole::User, "yooo".to_string()).into(),
            engine::ChatMessage::text(engine::ChatRole::Assistant, "What is up?".to_string())
                .into(),
        ],
    );
    let mut checkpoint = durable_checkpoint(&workflow, projection);
    checkpoint.engine.entrypoint_text = Some("Ask me a question".to_string());
    checkpoint.engine.transcripts.insert(
        node_id.clone(),
        vec![
            engine::AgentTranscriptItem::AssistantMessage {
                content: "What are you looking forward to this week?".to_string(),
            },
            engine::AgentTranscriptItem::UserMessage {
                content: "yooo".to_string(),
                attachments: Vec::new(),
            },
            engine::AgentTranscriptItem::AssistantMessage {
                content: "What is up?".to_string(),
            },
        ],
    );

    assert!(checkpoint.repair_direct_chat_projection());

    let visible = checkpoint
        .projection
        .chat_logs
        .get(&node_id)
        .expect("chat messages")
        .iter()
        .map(|message| (message.role.clone(), message.content.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        visible,
        vec![
            (engine::ChatRole::User, "Ask me a question"),
            (
                engine::ChatRole::Assistant,
                "What are you looking forward to this week?"
            ),
            (engine::ChatRole::User, "yooo"),
            (engine::ChatRole::Assistant, "What is up?"),
        ]
    );
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

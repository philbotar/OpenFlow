use super::*;
use crate::chat::{Chat, ChatConfig, ChatStore};
use crate::run::execution::{ExecutionAction, ExecutionEvent};
use crate::run::state::WorkflowRunState;
use crate::settings::model::{AppSettings, ProviderProfile, ProviderTransport};
use crate::workflow::catalog::default_workflow;
use crate::workflow::ports::{WorkflowStore, WorkflowStoreState};
use engine::{ApprovalMode, ChatRole, Node, NodeId, Workflow, WorkflowId};
use providers::ProviderId;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

fn project_dir(dir: &tempfile::TempDir) -> String {
    let path = dir.path().join("project-repo");
    std::fs::create_dir_all(&path).expect("project dir");
    path.to_string_lossy().into_owned()
}

fn backend() -> (AppBackend, tempfile::TempDir) {
    let dir = tempdir().expect("tempdir");
    let chat_store = FileChatStore::new(dir.path().join("chats.json"));
    backend_with_chat_store(Box::new(chat_store), dir)
}

fn backend_with_chat_store(
    chat_store: Box<dyn ChatStore>,
    dir: tempfile::TempDir,
) -> (AppBackend, tempfile::TempDir) {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let backend = AppBackend::new(
        AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new(dir.path().join("workflows.json"))),
            chat_store,
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new(dir.path().join("agents.json"))),
            project_store: Box::new(FileProjectStore::new(dir.path().join("projects.json"))),
            settings_store: std::sync::Arc::new(FileSettingsStore::new(
                dir.path().join("settings.json"),
            )),
            skill_catalog: Box::new(FileSkillCatalog),
            attachment_store: Arc::new(FileRunAttachmentStore::default()),
            env: ProviderEnv::from_pairs([
                ("OPENAI_API_KEY", "openai-key"),
                ("OPENAI_COMPATIBLE_API_KEY", "compatible-key"),
            ]),
            runtime_handle: runtime.handle().clone(),
        },
        Some(runtime),
    );
    (backend, dir)
}

struct FailingChatStore {
    chats: Mutex<Vec<Chat>>,
    fail_saves: AtomicBool,
}

impl ChatStore for Arc<FailingChatStore> {
    fn load(&self) -> io::Result<Vec<Chat>> {
        Ok(self.chats.lock().expect("chat store lock").clone())
    }

    fn save(&self, _chats: &[Chat]) -> io::Result<()> {
        if self.fail_saves.load(Ordering::SeqCst) {
            Err(io::Error::other("attach save failed"))
        } else {
            Ok(())
        }
    }
}

#[cfg_attr(miri, ignore)]
#[test]
fn fresh_profile_includes_matt_pocock_idea_to_ship_example() {
    let (backend, _dir) = backend();

    let workflows = backend.load_all_workflows().expect("load workflows");
    let example = workflows
        .iter()
        .find(|workflow| workflow.id == "matt-pocock-idea-to-ship")
        .expect("seeded example");

    assert_eq!(example.name, "Matt Pocock skills: idea to ship");
    assert_eq!(example.nodes.len(), 4);
}

#[cfg_attr(miri, ignore)]
#[test]
fn existing_workflow_with_seed_id_is_not_overwritten() {
    let (backend, dir) = backend();
    let mut existing = Workflow::new("My edited workflow");
    existing.id = WorkflowId("matt-pocock-idea-to-ship".to_string());
    FileWorkflowStore::new(dir.path().join("workflows.json"))
        .save(std::slice::from_ref(&existing))
        .expect("prepopulate workflow");

    let loaded = backend
        .load_workflow("matt-pocock-idea-to-ship")
        .expect("load existing workflow");

    assert_eq!(loaded, existing);
}

#[cfg_attr(miri, ignore)]
#[test]
fn incomplete_matt_pocock_seed_is_repaired() {
    let (backend, dir) = backend();
    let mut incomplete = Workflow::new("Matt Pocock skills: idea to ship");
    incomplete.id = WorkflowId("matt-pocock-idea-to-ship".to_string());
    for (id, label, x) in [
        ("select-ticket", "2. Select frontier ticket", 528.0),
        ("commit-gate", "4. Human commit gate", 1392.0),
    ] {
        let mut node = Node::agent(label, x, 96.0);
        node.id = NodeId(id.to_string());
        incomplete.nodes.push(node);
    }
    let mut state = WorkflowStoreState {
        workflows: vec![incomplete],
        ..WorkflowStoreState::default()
    };
    state
        .applied_seeds
        .insert("matt-pocock-idea-to-ship".to_string());
    FileWorkflowStore::new(dir.path().join("workflows.json"))
        .save_state(&state)
        .expect("prepopulate incomplete seed");

    let repaired = backend
        .load_workflow("matt-pocock-idea-to-ship")
        .expect("load repaired workflow");

    assert_eq!(
        repaired
            .nodes
            .iter()
            .map(|node| &*node.id)
            .collect::<Vec<_>>(),
        [
            "shape-work",
            "select-ticket",
            "implement-ticket",
            "commit-gate",
        ]
    );
    assert_eq!(repaired.edges.len(), 3);
}

#[cfg_attr(miri, ignore)]
#[test]
fn deleted_seeded_example_stays_deleted() {
    let (backend, _dir) = backend();
    backend.load_all_workflows().expect("seed workflows");

    backend
        .delete_workflow("matt-pocock-idea-to-ship")
        .expect("delete seeded example");

    assert!(backend
        .list_workflows()
        .expect("list workflows")
        .iter()
        .all(|workflow| workflow.id != "matt-pocock-idea-to-ship"));
}

#[cfg_attr(miri, ignore)]
#[test]
fn create_chat_is_valid_and_does_not_create_a_workflow() {
    let (backend, _dir) = backend();
    let workflow_ids_before = backend
        .list_workflows()
        .expect("list workflows")
        .into_iter()
        .map(|workflow| workflow.id)
        .collect::<Vec<_>>();

    let chat = backend.create_chat().expect("create chat");
    let serialized = serde_json::to_value(&chat).expect("serialize chat");

    assert_eq!(chat.title, "New chat");
    assert!(chat.run_id.is_none());
    assert!(serialized.get("workflow").is_none());
    assert!(serialized.get("nodes").is_none());
    assert_eq!(serialized["config"]["approvalMode"], "read_only");
    assert!(serialized["config"]["reasoningEffort"].is_null());
    assert!(serialized["config"]["projectId"].is_null());
    assert_eq!(
        backend
            .list_workflows()
            .expect("list workflows after chat")
            .into_iter()
            .map(|workflow| workflow.id)
            .collect::<Vec<_>>(),
        workflow_ids_before
    );
}

#[cfg_attr(miri, ignore)]
#[test]
fn delete_chat_removes_only_the_requested_chat() {
    let (backend, _dir) = backend();
    let deleted = backend.create_chat().expect("create deleted chat");
    let survivor = backend.create_chat().expect("create surviving chat");

    backend
        .block_on_test(backend.delete_chat(&deleted.id))
        .expect("delete chat");

    assert_eq!(backend.list_chats().expect("list chats"), vec![survivor]);
    assert!(matches!(
        backend.block_on_test(backend.delete_chat(&deleted.id)),
        Err(BackendError::ChatNotFound(id)) if id == deleted.id
    ));
}

#[cfg_attr(miri, ignore)]
#[test]
fn chat_runtime_config_persists_with_project_scope() {
    let (backend, dir) = backend();
    let project = backend
        .create_project_from_directory(project_dir(&dir))
        .expect("create project");
    let chat = backend.create_chat().expect("create chat");
    let config = ChatConfig {
        model: Some("gpt-5".to_string()),
        approval_mode: ApprovalMode::AlwaysAsk,
        reasoning_effort: Some("high".to_string()),
        reasoning_budget_tokens: Some(24_000),
        project_id: Some(project.id),
    };

    let updated = backend
        .update_chat_config(&chat.id, config.clone())
        .expect("update chat config");
    let reopened = backend
        .list_chats()
        .expect("list chats")
        .into_iter()
        .find(|item| item.id == chat.id)
        .expect("configured chat");

    assert_eq!(updated.config, config);
    assert_eq!(reopened.config, config);
}

#[cfg_attr(miri, ignore)]
#[test]
fn start_chat_attaches_a_durable_run_without_creating_a_workflow() {
    let (backend, dir) = backend();
    let project = backend
        .create_project_from_directory(project_dir(&dir))
        .expect("create project");
    let chat = backend.create_chat().expect("create chat");
    let chat = backend
        .update_chat_config(
            &chat.id,
            ChatConfig {
                project_id: Some(project.id.clone()),
                ..ChatConfig::default()
            },
        )
        .expect("scope chat to project");

    backend.block_on_test(async {
        let (updated, initial_state, _event_rx) = backend
            .start_chat(
                &chat.id,
                Some("Explain durable runs".to_string()),
                &AppSettings::default(),
                None,
            )
            .await
            .expect("start chat");

        assert_eq!(updated.run_id, initial_state.run_id);
        assert!(updated.run_id.is_some());
        assert_eq!(updated.title, "Explain durable runs");
        assert!(initial_state.chat_logs.values().flatten().any(|message| {
            message.role == ChatRole::User && message.content == "Explain durable runs"
        }));
        let replay = backend
            .replay_run(updated.run_id.as_deref().expect("chat run id"))
            .expect("replay initial chat checkpoint");
        assert!(replay.chat_logs.values().flatten().any(|message| {
            message.role == ChatRole::User && message.content == "Explain durable runs"
        }));
        assert_eq!(
            backend
                .list_runs(Some(&chat.id))
                .expect("list chat runs")
                .first()
                .and_then(|run| run.project_id.as_deref()),
            Some(project.id.as_str())
        );
        assert!(backend
            .list_workflows()
            .expect("list workflows")
            .iter()
            .all(|workflow| workflow.id != chat.id));

        backend.stop_run().await.expect("stop chat");
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn start_chat_stops_run_when_chat_attachment_fails() {
    let chat = Chat {
        id: "chat-transactional".to_string(),
        title: "New chat".to_string(),
        config: ChatConfig::default(),
        run_id: None,
        created_at_ms: 1,
        updated_at_ms: 1,
    };
    let store = Arc::new(FailingChatStore {
        chats: Mutex::new(vec![chat.clone()]),
        fail_saves: AtomicBool::new(true),
    });
    let (backend, _dir) =
        backend_with_chat_store(Box::new(Arc::clone(&store)), tempdir().expect("tempdir"));

    backend.block_on_test(async {
        let error = backend
            .start_chat(
                &chat.id,
                Some("Do not persist before validation".to_string()),
                &AppSettings::default(),
                None,
            )
            .await
            .expect_err("attachment failure");

        assert!(error.to_string().contains("attach save failed"));
        assert!(!backend.is_run_active().await);
        let chats = store.chats.lock().expect("chat store lock");
        assert_eq!(chats[0].title, "New chat");
        assert!(chats[0].run_id.is_none());
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn start_workflow_authoring_returns_session_id() {
    let (backend, _dir) = backend();
    let started = backend
        .start_workflow_authoring(None, None)
        .expect("start authoring");
    assert!(!started.session_id.is_empty());
    assert_eq!(started.draft.as_ref().expect("draft").nodes.len(), 4);
}

#[cfg_attr(miri, ignore)]
#[test]
fn create_and_load_workflow_round_trips() {
    let (backend, _dir) = backend();
    let workflow = backend
        .create_workflow("Workflow 1".to_string())
        .expect("create workflow");

    let items = backend.list_workflows().expect("list workflows");
    let loaded = backend.load_workflow(&workflow.id).expect("load workflow");

    assert!(items
        .iter()
        .any(|item| item.id == workflow.id.to_string() && item.name == "Workflow 1"));
    assert_eq!(loaded.id, workflow.id);
    assert_eq!(loaded.nodes.len(), 1);
}

#[cfg_attr(miri, ignore)]
#[test]
fn save_workflows_overwrites_store() {
    let (backend, _dir) = backend();
    let first = backend
        .create_workflow("One".to_string())
        .expect("create first workflow");
    let second = backend
        .create_workflow("Two".to_string())
        .expect("create second workflow");

    backend
        .save_workflows(std::slice::from_ref(&first))
        .expect("save workflows");

    let items = backend.list_workflows().expect("list workflows");
    assert!(items.iter().any(|item| item.id == first.id.to_string()));
    assert!(items.iter().all(|item| item.id != second.id.to_string()));
    assert_eq!(
        backend
            .load_workflow(&second.id)
            .expect_err("missing second workflow")
            .to_string(),
        format!("workflow {} not found", second.id)
    );
}

#[cfg_attr(miri, ignore)]
#[test]
fn create_and_load_agents_round_trip() {
    let (backend, _dir) = backend();
    let agent = backend
        .create_agent_definition("Research Agent".to_string())
        .expect("create agent");

    let items = backend.list_agents().expect("list agents");
    let loaded = backend.load_agents().expect("load agents");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Research Agent");
    assert_eq!(loaded, vec![agent]);
}

#[cfg_attr(miri, ignore)]
#[test]
fn save_agents_overwrites_store() {
    let (backend, _dir) = backend();
    let first = backend
        .create_agent_definition("One".to_string())
        .expect("create first agent");
    backend
        .create_agent_definition("Two".to_string())
        .expect("create second agent");

    backend
        .save_agents(std::slice::from_ref(&first))
        .expect("save agents");

    let items = backend.list_agents().expect("list agents");
    let loaded = backend.load_agents().expect("load agents");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, first.id);
    assert_eq!(loaded, vec![first]);
}

#[cfg_attr(miri, ignore)]
#[test]
fn create_agent_node_without_template_uses_default_node() {
    let (backend, _dir) = backend();

    let node = backend
        .create_agent_node(2, 32.0, 48.0, None)
        .expect("create default node");

    assert_eq!(node.label, "Agent 3");
    assert_eq!(node.position.x, 32.0);
    assert_eq!(node.position.y, 48.0);
    assert_eq!(node.agent, engine::AgentNodeConfig::default());
}

#[cfg_attr(miri, ignore)]
#[test]
fn create_agent_node_from_template_id_copies_agent_config() {
    let (backend, _dir) = backend();
    let mut agent = backend
        .create_agent_definition("Research Agent".to_string())
        .expect("create agent");
    agent.system_prompt = "system".to_string();
    agent.task_prompt = "task".to_string();
    agent.model = "gpt-template".to_string();
    agent.output_schema =
        serde_json::json!({ "type": "object", "properties": { "ok": { "type": "boolean" } } });
    agent.auto_start = false;
    agent.tools.approval_mode = Some(engine::ApprovalMode::AlwaysAsk);
    backend
        .save_agents(std::slice::from_ref(&agent))
        .expect("save agent");

    let node = backend
        .create_agent_node(0, 12.0, 24.0, Some(&agent.id))
        .expect("create templated node");

    assert_eq!(node.label, "Research Agent");
    assert_eq!(node.position.x, 12.0);
    assert_eq!(node.position.y, 24.0);
    assert_eq!(node.agent.system_prompt, "system");
    assert_eq!(node.agent.task_prompt, "task");
    assert_eq!(node.agent.model, "gpt-template");
    assert_eq!(
        node.agent.output_schema,
        serde_json::json!({ "type": "object", "properties": { "ok": { "type": "boolean" } } })
    );
    assert!(node.agent.auto_start);
    assert_eq!(
        node.agent.tools.approval_mode,
        Some(engine::ApprovalMode::AlwaysAsk)
    );
}

#[cfg_attr(miri, ignore)]
#[test]
fn custom_provider_readiness_allows_a_trusted_endpoint_without_a_key() {
    let mut settings = AppSettings {
        active_provider: ProviderId::from("custom_openai_compatible"),
        ..AppSettings::default()
    };
    settings.providers.insert(
        ProviderId::from("custom_openai_compatible"),
        ProviderProfile {
            transport: ProviderTransport::ChatCompletions,
            ..ProviderProfile::compatible_default()
        },
    );

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let readiness = AppBackend::new(
        AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new("/tmp/unused-workflows.json")),
            chat_store: Box::new(FileChatStore::new("/tmp/unused-chats.json")),
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new("/tmp/unused-agents.json")),
            project_store: Box::new(FileProjectStore::new("/tmp/unused-projects.json")),
            settings_store: std::sync::Arc::new(FileSettingsStore::new(
                "/tmp/unused-settings.json",
            )),
            skill_catalog: Box::new(FileSkillCatalog),
            attachment_store: Arc::new(FileRunAttachmentStore::default()),
            env: ProviderEnv::default(),
            runtime_handle: runtime.handle().clone(),
        },
        Some(runtime),
    )
    .resolve_provider_readiness(&settings, None);

    assert!(readiness.ready);
    assert_eq!(readiness.message, "Ready");
    assert_eq!(readiness.env_var, "OPENAI_COMPATIBLE_API_KEY");
}

#[cfg_attr(miri, ignore)]
#[test]
fn start_run_ignores_legacy_non_auto_start_flag() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let mut workflow = Workflow::new("Manual run");
        let mut node = Node::agent("Review", 0.0, 0.0);
        node.id = NodeId("review".to_string());
        node.agent.auto_start = false;
        workflow.nodes = vec![node];

        let (initial_state, mut event_rx) = backend
            .start_run(workflow, None, None, &AppSettings::default(), None)
            .await
            .expect("start run");

        assert!(initial_state.active);
        assert!(initial_state.awaiting_node_id.is_none());

        let first = event_rx.recv().await.expect("queued event");
        let second = event_rx.recv().await.expect("started event");
        assert!(matches!(
            first,
            ExecutionEvent::NodeQueued { ref node_id, ref label }
                if node_id == "review" && label == "Review"
        ));
        assert!(matches!(
            second,
            ExecutionEvent::NodeStarted { ref node_id, ref label }
                if node_id == "review" && label == "Review"
        ));

        let stopped = backend.stop_run().await.expect("stop run");
        assert!(!stopped.active);
        assert!(stopped.last_error.is_none());
        assert!(backend.is_run_continuable().await);
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn start_run_with_entrypoint_records_chat_for_legacy_non_auto_start_root() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let mut workflow = Workflow::new("Manual kickoff");
        let mut node = Node::agent("Review", 0.0, 0.0);
        node.id = NodeId("review".to_string());
        node.agent.auto_start = false;
        workflow.nodes = vec![node];

        let (initial_state, _event_rx) = backend
            .start_run(
                workflow,
                Some("Plan ORCHID-91".to_string()),
                None,
                &AppSettings::default(),
                None,
            )
            .await
            .expect("start run");

        assert_eq!(
            initial_state
                .chat_logs
                .get(&NodeId("review".into()))
                .map_or(0, Vec::len),
            1
        );

        backend.stop_run().await.expect("stop run");
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn start_run_with_entrypoint_records_chat_for_auto_start_root() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let mut workflow = Workflow::new("Auto kickoff");
        let mut node = Node::agent("Plan", 0.0, 0.0);
        node.id = NodeId("plan".to_string());
        node.agent.auto_start = true;
        workflow.nodes = vec![node];

        let (initial_state, _event_rx) = backend
            .start_run(
                workflow,
                Some("Plan ORCHID-91".to_string()),
                None,
                &AppSettings::default(),
                None,
            )
            .await
            .expect("start run");

        let log = initial_state
            .chat_logs
            .get(&NodeId("plan".into()))
            .expect("chat log");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].role, engine::ChatRole::User);

        backend.stop_run().await.expect("stop run");
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn stop_run_is_idempotent_when_inactive() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let workflow = default_workflow("Workflow");
        let run_state = WorkflowRunState::idle_for_workflow(&workflow);
        backend
            .runs
            .test_seed_session(workflow.clone(), run_state.clone(), {
                let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                tx
            })
            .await;

        let snapshot = backend.stop_run().await.expect("stop inactive run");
        assert!(!snapshot.active);
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn stop_run_aborts_orphaned_active_session_without_handle() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let workflow = default_workflow("Workflow");
        let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
        run_state.run_id = Some("orphaned-run".to_string());
        backend
            .runs
            .test_seed_session(workflow, run_state, {
                let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                tx
            })
            .await;

        let stopped = backend.stop_run().await.expect("stop orphaned run");
        assert!(!stopped.active);
        assert!(backend
            .get_run_state()
            .await
            .is_some_and(|state| !state.active));
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn apply_execution_event_ignores_events_after_run_stopped() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let workflow = default_workflow("Workflow");
        let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
        run_state.run_id = Some("stopped-run".to_string());
        backend
            .runs
            .test_seed_session(workflow, run_state, {
                let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                tx
            })
            .await;

        let stopped = backend.stop_run().await.expect("stop run");
        assert!(!stopped.active);

        let snapshot = backend
            .apply_execution_event(ExecutionEvent::NodeQueued {
                node_id: NodeId("idea".to_string()),
                label: "Idea".to_string(),
            })
            .await
            .expect("ignored stale event");

        assert!(!snapshot.active);
        assert!(snapshot
            .run_trace
            .iter()
            .all(|entry| entry.node_id != NodeId("idea".to_string())));
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn submit_user_input_updates_snapshot_and_sends_action() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let workflow = default_workflow("Workflow");
        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
        run_state.awaiting_node_id = Some(NodeId("idea".to_string()));
        run_state.awaiting_node_ids = vec![NodeId("idea".to_string())];
        backend
            .runs
            .test_seed_session(workflow, run_state, action_tx)
            .await;

        let run_state = backend
            .submit_user_input("idea", "Continue with approvals".to_string())
            .await
            .expect("submit input");

        assert!(run_state.awaiting_node_id.is_none());
        assert!(run_state.awaiting_node_ids.is_empty());
        assert_eq!(
            run_state
                .chat_logs
                .get(&NodeId("idea".to_string()))
                .unwrap()
                .last()
                .unwrap()
                .content,
            "Continue with approvals"
        );
        match action_rx.recv().await.expect("action") {
            ExecutionAction::ProvideInput {
                node_id,
                text,
                attachments,
                skill_prompt,
            } => {
                assert_eq!(node_id, NodeId("idea".to_string()));
                assert_eq!(text, "Continue with approvals");
                assert!(attachments.is_empty());
                assert!(skill_prompt.is_none());
            }
            ExecutionAction::ResolveApproval { .. } => {
                panic!("unexpected approval action");
            }
            ExecutionAction::Stop => panic!("unexpected stop action"),
            ExecutionAction::RetryNode { .. } => panic!("unexpected retry action"),
        }
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn submit_tool_approval_updates_snapshot_and_sends_action() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let workflow = default_workflow("Workflow");
        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
        run_state.pending_approvals = vec![engine::PendingToolApproval {
            approval_id: "approval-1".to_string(),
            node_id: NodeId::from("idea"),
            node_label: "Idea".to_string(),
            tool_call: engine::ToolCall {
                id: "call-1".to_string(),
                provider_call_id: None,
                name: "read".to_string(),
                arguments: serde_json::json!({ "path": "README.md" }),
            },
            tier: engine::ToolTier::Read,
        }];
        backend
            .runs
            .test_seed_session(workflow, run_state, action_tx)
            .await;

        let run_state = backend
            .submit_tool_approval("approval-1", true, None)
            .await
            .expect("submit approval");

        assert_eq!(run_state.pending_approvals.len(), 1);
        match action_rx.recv().await.expect("action") {
            ExecutionAction::ResolveApproval {
                approval_id,
                allow,
                reason: _,
            } => {
                assert_eq!(approval_id, "approval-1");
                assert!(allow);
            }
            ExecutionAction::ProvideInput { .. } => {
                panic!("unexpected input action");
            }
            ExecutionAction::Stop => panic!("unexpected stop action"),
            ExecutionAction::RetryNode { .. } => panic!("unexpected retry action"),
        }
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn copy_workflow_to_project_creates_independent_copy() {
    let (backend, dir) = backend();
    let project_a_path = dir.path().join("project-a");
    let project_b_path = dir.path().join("project-b");
    std::fs::create_dir_all(&project_a_path).expect("project-a dir");
    std::fs::create_dir_all(&project_b_path).expect("project-b dir");

    let workflow = backend
        .create_workflow("Source Flow".to_string())
        .expect("create workflow");
    let project_a = backend
        .create_project_from_directory(project_a_path.to_string_lossy().into_owned())
        .expect("create project a");
    backend
        .assign_workflow_to_project(&project_a.id, &workflow.id.to_string())
        .expect("assign workflow to a");

    let project_b = backend
        .create_project_from_directory(project_b_path.to_string_lossy().into_owned())
        .expect("create project b");

    let result = backend
        .copy_workflow_to_project(&project_b.id, &workflow.id.to_string())
        .expect("copy workflow");

    assert_ne!(result.workflow.id, workflow.id);
    assert_eq!(result.workflow.name, "Source Flow copy");

    let project_a_loaded = result
        .projects
        .iter()
        .find(|project| project.id == project_a.id)
        .expect("project a");
    let project_b_loaded = result
        .projects
        .iter()
        .find(|project| project.id == project_b.id)
        .expect("project b");
    assert_eq!(project_a_loaded.workflow_ids, vec![workflow.id.to_string()]);
    assert_eq!(
        project_b_loaded.workflow_ids,
        vec![result.workflow.id.to_string()]
    );

    let source = backend
        .load_workflow(&workflow.id.to_string())
        .expect("load source");
    let copy = backend
        .load_workflow(&result.workflow.id.to_string())
        .expect("load copy");
    assert_eq!(source.name, "Source Flow");
    assert_eq!(copy.name, "Source Flow copy");
}

#[cfg_attr(miri, ignore)]
#[test]
fn assign_workflow_to_project_round_trips() {
    let (backend, dir) = backend();
    let workflow = backend
        .create_workflow("Flow".to_string())
        .expect("create workflow");
    let project = backend
        .create_project_from_directory(project_dir(&dir))
        .expect("create project");

    let projects = backend
        .assign_workflow_to_project(&project.id, &workflow.id.to_string())
        .expect("assign workflow");

    assert_eq!(projects[0].workflow_ids, vec![workflow.id.to_string()]);
    let loaded = backend.list_projects().expect("list projects");
    assert_eq!(loaded[0].workflow_ids, vec![workflow.id.to_string()]);
}

#[cfg_attr(miri, ignore)]
#[test]
fn rename_workflow_updates_list_and_load() {
    let (backend, _dir) = backend();
    let workflow = backend
        .create_workflow("Original".to_string())
        .expect("create workflow");

    let renamed = backend
        .rename_workflow(&workflow.id, "Renamed".to_string())
        .expect("rename workflow");

    assert_eq!(renamed.name, "Renamed");
    let items = backend.list_workflows().expect("list workflows");
    assert!(items
        .iter()
        .any(|item| item.id == workflow.id.to_string() && item.name == "Renamed"));
    assert_eq!(
        backend
            .load_workflow(&workflow.id)
            .expect("load workflow")
            .name,
        "Renamed"
    );
}

#[cfg_attr(miri, ignore)]
#[test]
fn load_and_save_settings_round_trip() {
    let (backend, _dir) = backend();
    let mut settings = backend.load_settings(None).expect("load settings");
    settings.settings.active_provider = "openai".into();

    backend
        .save_settings(&settings.settings)
        .expect("save settings");
    let loaded = backend.load_settings(None).expect("reload settings");
    assert_eq!(
        loaded.settings.active_provider,
        settings.settings.active_provider
    );
}

#[cfg_attr(miri, ignore)]
#[test]
fn get_run_state_is_none_when_idle() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let state = backend.get_run_state().await;
        assert!(state.is_none());
    });
}

#[cfg_attr(miri, ignore)]
#[test]
fn unassign_workflow_from_project_round_trips() {
    let (backend, dir) = backend();
    let workflow = backend
        .create_workflow("Flow".to_string())
        .expect("create workflow");
    let project = backend
        .create_project_from_directory(project_dir(&dir))
        .expect("create project");
    backend
        .assign_workflow_to_project(&project.id, &workflow.id.to_string())
        .expect("assign workflow");

    let projects = backend
        .unassign_workflow_from_project(&project.id, &workflow.id.to_string())
        .expect("unassign workflow");

    assert!(projects[0].workflow_ids.is_empty());
}

#[cfg_attr(miri, ignore)]
#[test]
fn delete_workflow_removes_independent_workflow() {
    let (backend, _dir) = backend();
    let workflow = backend
        .create_workflow("Delete me".to_string())
        .expect("create workflow");

    backend
        .delete_workflow(&workflow.id.to_string())
        .expect("delete workflow");

    assert!(backend
        .list_workflows()
        .expect("list")
        .iter()
        .all(|item| item.id != workflow.id.to_string()));
    assert!(backend
        .load_workflow(&workflow.id)
        .expect_err("workflow gone")
        .to_string()
        .contains("not found"));
}

#[cfg_attr(miri, ignore)]
#[test]
fn delete_workflow_removes_project_assigned_workflow() {
    let (backend, dir) = backend();
    let workflow = backend
        .create_workflow("Project flow".to_string())
        .expect("create workflow");
    let project = backend
        .create_project_from_directory(project_dir(&dir))
        .expect("create project");
    backend
        .assign_workflow_to_project(&project.id, &workflow.id.to_string())
        .expect("assign workflow");

    let projects = backend
        .delete_workflow(&workflow.id.to_string())
        .expect("delete workflow");

    assert!(projects[0].workflow_ids.is_empty());
    assert!(backend
        .list_workflows()
        .expect("list")
        .iter()
        .all(|item| item.id != workflow.id.to_string()));
}

#[cfg_attr(miri, ignore)]
#[test]
fn submit_tool_approval_denied_forwards_reason() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let workflow = default_workflow("Workflow");
        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
        run_state.pending_approvals = vec![engine::PendingToolApproval {
            approval_id: "approval-2".to_string(),
            node_id: NodeId::from("idea"),
            node_label: "Idea".to_string(),
            tool_call: engine::ToolCall {
                id: "call-2".to_string(),
                provider_call_id: None,
                name: "bash".to_string(),
                arguments: serde_json::json!({ "command": "echo hi" }),
            },
            tier: engine::ToolTier::Write,
        }];
        backend
            .runs
            .test_seed_session(workflow, run_state, action_tx)
            .await;

        backend
            .submit_tool_approval("approval-2", false, Some("Too risky".to_string()))
            .await
            .expect("submit denial");

        match action_rx.recv().await.expect("action") {
            ExecutionAction::ResolveApproval {
                approval_id,
                allow,
                reason,
            } if approval_id == "approval-2" && !allow => {
                assert_eq!(reason.as_deref(), Some("Too risky"));
            }
            ExecutionAction::ResolveApproval { .. }
            | ExecutionAction::ProvideInput { .. }
            | ExecutionAction::Stop
            | ExecutionAction::RetryNode { .. } => {
                panic!("unexpected action variant");
            }
        }
    });
}

#[cfg_attr(miri, ignore)] // ponytail: Miri cannot emulate git subprocess (fork)
#[test]
fn list_project_file_references_returns_gitignore_aware_matches() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("git init");
    std::fs::create_dir_all(dir.path().join("src")).expect("create src");
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("write main");
    std::fs::write(dir.path().join(".gitignore"), "ignored.rs\n").expect("write gitignore");
    std::fs::write(dir.path().join("ignored.rs"), "ignored\n").expect("write ignored");

    let (backend, _guard) = backend();
    let refs = backend
        .list_project_file_references(
            dir.path().to_str().expect("utf8 path").to_string(),
            Some("rs".to_string()),
            Some(20),
        )
        .expect("list refs");

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].path, "src/main.rs");
}

#[cfg_attr(miri, ignore)]
#[test]
fn saving_workflow_refreshes_schedule_statuses() {
    let (backend, _dir) = backend();
    let mut workflow = backend
        .create_workflow("Scheduled".to_string())
        .expect("create workflow");
    workflow.settings.schedule = Some(engine::WorkflowSchedule {
        cron: "*/15 * * * *".to_string(),
        enabled: true,
        timezone: "UTC".to_string(),
    });

    backend.save_workflow(workflow).expect("save workflow");

    let statuses = backend.list_schedule_statuses();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].workflow_name, "Scheduled");
    assert!(statuses[0].next_run_at.is_some());
}

#[cfg_attr(miri, ignore)]
#[test]
fn tick_schedules_advances_next_run_without_reload() {
    let (backend, _dir) = backend();
    let mut workflow = backend
        .create_workflow("Scheduled".to_string())
        .expect("create workflow");
    workflow.settings.schedule = Some(engine::WorkflowSchedule {
        cron: "0 9 * * *".to_string(),
        enabled: true,
        timezone: "UTC".to_string(),
    });
    backend.save_workflow(workflow).expect("save workflow");
    backend
        .refresh_schedules_at("2026-06-16T08:00:00Z".parse().expect("timestamp"))
        .expect("refresh");

    backend.tick_schedules_at("2026-06-16T10:00:00Z".parse().expect("timestamp"));

    let statuses = backend.list_schedule_statuses();
    assert_eq!(
        statuses[0].next_run_at.expect("next").to_rfc3339(),
        "2026-06-17T09:00:00+00:00"
    );
}

#[cfg_attr(miri, ignore)]
#[test]
fn due_schedule_candidate_uses_workflow_id() {
    let (backend, _dir) = backend();
    let mut workflow = backend
        .create_workflow("Scheduled".to_string())
        .expect("create workflow");
    workflow.settings.schedule = Some(engine::WorkflowSchedule {
        cron: "*/15 * * * *".to_string(),
        enabled: true,
        timezone: "UTC".to_string(),
    });
    let workflow_id = workflow.id.to_string();
    backend.save_workflow(workflow).expect("save workflow");
    backend
        .refresh_schedules_at("2026-06-16T00:01:00Z".parse().expect("timestamp"))
        .expect("refresh");

    let candidate = backend
        .block_on_test(
            backend.claim_due_scheduled_run_at("2026-06-16T00:15:00Z".parse().expect("timestamp")),
        )
        .expect("claim result")
        .expect("candidate");

    assert_eq!(candidate.workflow_id, workflow_id);
}

#[test]
fn search_api_key_round_trip() {
    let (backend, _dir) = backend();
    assert_eq!(backend.load_search_api_key("brave").unwrap(), None);

    backend.save_search_api_key("brave", " bk-123 ").unwrap();
    assert_eq!(
        backend.load_search_api_key("brave").unwrap(),
        Some("bk-123".to_string())
    );

    let loaded = backend.load_settings(None).unwrap();
    assert!(loaded
        .settings
        .search
        .keys
        .values()
        .all(|key| key.is_empty()));

    backend.delete_search_api_key("brave").unwrap();
    assert_eq!(backend.load_search_api_key("brave").unwrap(), None);
}

#[test]
fn delete_provider_api_key_clears_stored_key() {
    let (backend, _dir) = backend();

    backend
        .save_provider_api_key("openai", "sk-secret")
        .expect("save key");
    backend
        .delete_provider_api_key("openai")
        .expect("delete key");

    assert_eq!(backend.load_provider_api_key("openai").unwrap(), None);
}

#[test]
fn load_provider_api_key_ignores_bedrock_aws_profile() {
    let (backend, dir) = backend();
    let store = FileSettingsStore::new(dir.path().join("settings.json"));
    let mut settings = store.load().unwrap();
    settings
        .providers
        .get_mut(&ProviderId::from("bedrock"))
        .expect("bedrock profile")
        .aws_profile = "bedrock".to_string();
    store.save(&settings).unwrap();

    assert_eq!(backend.load_provider_api_key("bedrock").unwrap(), None);
}

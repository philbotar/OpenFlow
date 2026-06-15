use super::*;
use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::incident::IncidentRecorder;
use crate::run::execution::{ExecutionAction, ExecutionEvent};
use crate::settings::model::{ProviderProfile, ProviderTransport};
use crate::workflow::catalog::default_workflow;
use engine::{Node, NodeId};
use providers::ProviderId;
use std::sync::Arc;
use tempfile::tempdir;

fn backend() -> (AppBackend, tempfile::TempDir) {
    let dir = tempdir().expect("tempdir");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let backend = AppBackend::new(
        AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new(dir.path().join("workflows.json"))),
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new(dir.path().join("agents.json"))),
            project_store: Box::new(FileProjectStore::new(dir.path().join("projects.json"))),
            settings_store: Box::new(FileSettingsStore::new(dir.path().join("settings.json"))),
            skill_catalog: Box::new(FileSkillCatalog),
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

#[test]
fn start_workflow_authoring_returns_session_id() {
    let (backend, _dir) = backend();
    let session_id = backend.start_workflow_authoring(None);
    assert!(!session_id.is_empty());
}

#[test]
fn create_and_load_workflow_round_trips() {
    let (backend, _dir) = backend();
    let workflow = backend
        .create_workflow("Workflow 1".to_string())
        .expect("create workflow");

    let items = backend.list_workflows().expect("list workflows");
    let loaded = backend.load_workflow(&workflow.id).expect("load workflow");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Workflow 1");
    assert_eq!(loaded.id, workflow.id);
    assert_eq!(loaded.nodes.len(), 1);
}

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
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, first.id.to_string());
    assert_eq!(
        backend
            .load_workflow(&second.id)
            .expect_err("missing second workflow")
            .to_string(),
        format!("workflow {} not found", second.id)
    );
}

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
    agent.tools.catalog.tools = vec![engine::ToolRef {
        name: "search".to_string(),
        tier: Some(engine::ToolTier::Read),
    }];
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
    assert!(!node.agent.auto_start);
    assert_eq!(
        node.agent.tools.catalog.tools,
        vec![engine::ToolRef {
            name: "search".to_string(),
            tier: Some(engine::ToolTier::Read),
        }]
    );
    assert_eq!(
        node.agent.tools.approval_mode,
        Some(engine::ApprovalMode::AlwaysAsk)
    );
}

#[test]
fn provider_readiness_reports_missing_key() {
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
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new("/tmp/unused-agents.json")),
            project_store: Box::new(FileProjectStore::new("/tmp/unused-projects.json")),
            settings_store: Box::new(FileSettingsStore::new("/tmp/unused-settings.json")),
            skill_catalog: Box::new(FileSkillCatalog),
            env: ProviderEnv::default(),
            runtime_handle: runtime.handle().clone(),
        },
        Some(runtime),
    )
    .resolve_provider_readiness(&settings, None);

    assert!(!readiness.ready);
    assert_eq!(readiness.env_var, "OPENAI_COMPATIBLE_API_KEY");
}

#[test]
fn terminal_start_failure_records_incident() {
    use crate::incident::IncidentCategory;

    let (mut backend, dir) = backend();
    backend.incidents = Arc::new(IncidentRecorder::new(Arc::new(FileIncidentStore::new(
        dir.path().join("incidents.jsonl"),
    ))));

    let error = backend
        .start_terminal(
            Some("/definitely/not/a/real/openflow/terminal/path"),
            80,
            24,
        )
        .expect_err("invalid cwd should fail");
    assert!(matches!(error, BackendError::ProjectOperation(_)));

    let incidents = backend.list_incidents(10).expect("list incidents");
    assert_eq!(incidents.len(), 1);
    assert_eq!(incidents[0].code, "terminal.start_failed");
    assert_eq!(incidents[0].category, IncidentCategory::Terminal);
}

#[test]
fn backend_err_persists_incident_before_returning() {
    let (mut backend, dir) = backend();
    backend.incidents = Arc::new(IncidentRecorder::new(Arc::new(FileIncidentStore::new(
        dir.path().join("incidents.jsonl"),
    ))));

    let error = backend.backend_err(BackendError::NoActiveRun);
    assert!(matches!(error, BackendError::NoActiveRun));

    let incidents = backend.list_incidents(10).expect("list incidents");
    assert_eq!(incidents.len(), 1);
    assert_eq!(incidents[0].code, "backend.no_active_run");
}

#[test]
fn list_incident_summaries_projects_records() {
    use crate::incident::{IncidentCategory, IncidentRecord, IncidentScope, IncidentSeverity};

    let (mut backend, dir) = backend();
    backend.incidents = Arc::new(IncidentRecorder::new(Arc::new(FileIncidentStore::new(
        dir.path().join("incidents.jsonl"),
    ))));

    backend
        .incidents()
        .record(IncidentRecord {
            id: "inc-1".to_string(),
            created_at_ms: 12_345,
            severity: IncidentSeverity::Warning,
            category: IncidentCategory::Node,
            scope: IncidentScope::Node {
                run_id: "run-1".to_string(),
                workflow_id: "wf-1".to_string(),
                node_id: NodeId("node-a".to_string()),
            },
            code: "node.failed".to_string(),
            message: "something broke".to_string(),
            hint: None,
            retryable: true,
            context: Default::default(),
            resolved: false,
        })
        .expect("record incident");

    let summaries = backend
        .list_incident_summaries(10)
        .expect("list incident summaries");
    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!(summary.id, "inc-1");
    assert_eq!(summary.created_at_ms, 12_345);
    assert_eq!(summary.severity, "warning");
    assert_eq!(summary.category, "node");
    assert_eq!(summary.code, "node.failed");
    assert_eq!(summary.message, "something broke");
    assert!(summary.retryable);
    assert!(!summary.resolved);
    assert_eq!(summary.workflow_id.as_deref(), Some("wf-1"));
    assert_eq!(summary.run_id.as_deref(), Some("run-1"));
    assert_eq!(summary.node_id.as_deref(), Some("node-a"));
}

#[test]
fn clear_resolved_incidents_removes_dismissed_records() {
    use crate::incident::{IncidentCategory, IncidentRecord, IncidentScope, IncidentSeverity};

    let (mut backend, dir) = backend();
    backend.incidents = Arc::new(IncidentRecorder::new(Arc::new(FileIncidentStore::new(
        dir.path().join("incidents.jsonl"),
    ))));

    backend
        .incidents()
        .record(IncidentRecord {
            id: "keep".to_string(),
            created_at_ms: 1,
            severity: IncidentSeverity::Error,
            category: IncidentCategory::Backend,
            scope: IncidentScope::App,
            code: "backend.io".to_string(),
            message: "keep".to_string(),
            hint: None,
            retryable: false,
            context: Default::default(),
            resolved: false,
        })
        .expect("record keep");
    backend
        .incidents()
        .record(IncidentRecord {
            id: "remove".to_string(),
            created_at_ms: 2,
            severity: IncidentSeverity::Error,
            category: IncidentCategory::Backend,
            scope: IncidentScope::App,
            code: "backend.io".to_string(),
            message: "remove".to_string(),
            hint: None,
            retryable: false,
            context: Default::default(),
            resolved: false,
        })
        .expect("record remove");
    backend.dismiss_incident("remove").expect("dismiss remove");

    let removed = backend
        .clear_resolved_incidents()
        .expect("clear resolved incidents");
    assert_eq!(removed, 1);

    let active = backend.list_incidents(10).expect("list incidents");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "keep");
}

#[test]
fn start_run_returns_initial_state_and_manual_events() {
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
        let second = event_rx.recv().await.expect("awaiting event");
        assert!(matches!(
            first,
            ExecutionEvent::NodeQueued { ref node_id, ref label }
                if node_id == "review" && label == "Review"
        ));
        assert!(matches!(
            second,
            ExecutionEvent::NodeAwaitingInput { ref node_id, ref label, is_initial: true, .. }
                if node_id == "review" && label == "Review"
        ));

        let stopped = backend.stop_run().await.expect("stop run");
        assert!(!stopped.active);
        assert!(stopped.last_error.is_none());
        assert!(backend.is_run_continuable().await);
    });
}

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
            ExecutionAction::ProvideInput { node_id, text } => {
                assert_eq!(node_id, NodeId("idea".to_string()));
                assert_eq!(text, "Continue with approvals");
            }
            ExecutionAction::ResolveApproval { .. } => {
                panic!("unexpected approval action");
            }
            ExecutionAction::Stop => panic!("unexpected stop action"),
            ExecutionAction::RetryNode { .. } => panic!("unexpected retry action"),
        }
    });
}

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

        assert!(run_state.pending_approvals.is_empty());
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

#[test]
fn assign_workflow_to_project_round_trips() {
    let (backend, _dir) = backend();
    let workflow = backend
        .create_workflow("Flow".to_string())
        .expect("create workflow");
    let project = backend
        .create_project_from_directory(std::env::temp_dir().to_string_lossy().into_owned())
        .expect("create project");

    let projects = backend
        .assign_workflow_to_project(&project.id, &workflow.id.to_string())
        .expect("assign workflow");

    assert_eq!(projects[0].workflow_ids, vec![workflow.id.to_string()]);
    let loaded = backend.load_projects().expect("load projects");
    assert_eq!(loaded[0].workflow_ids, vec![workflow.id.to_string()]);
}

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
    assert_eq!(items[0].name, "Renamed");
    assert_eq!(
        backend
            .load_workflow(&workflow.id)
            .expect("load workflow")
            .name,
        "Renamed"
    );
}

#[test]
fn load_and_save_settings_round_trip() {
    let (backend, _dir) = backend();
    let mut settings = backend.load_settings().expect("load settings");
    settings.active_provider = "openai".into();

    backend.save_settings(&settings).expect("save settings");
    let loaded = backend.load_settings().expect("reload settings");
    assert_eq!(loaded.active_provider, settings.active_provider);
}

#[test]
fn get_run_state_is_none_when_idle() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let state = backend.get_run_state().await;
        assert!(state.is_none());
    });
}

#[test]
fn unassign_workflow_from_project_round_trips() {
    let (backend, _dir) = backend();
    let workflow = backend
        .create_workflow("Flow".to_string())
        .expect("create workflow");
    let project = backend
        .create_project_from_directory(std::env::temp_dir().to_string_lossy().into_owned())
        .expect("create project");
    backend
        .assign_workflow_to_project(&project.id, &workflow.id.to_string())
        .expect("assign workflow");

    let projects = backend
        .unassign_workflow_from_project(&project.id, &workflow.id.to_string())
        .expect("unassign workflow");

    assert!(projects[0].workflow_ids.is_empty());
}

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
                name: "bash".to_string(),
                arguments: serde_json::json!({ "command": "echo hi" }),
            },
            tier: engine::ToolTier::Exec,
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

#[test]
fn read_project_file_references_returns_bounded_content() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("src")).expect("create src");
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn value() -> u8 { 7 }\n",
    )
    .expect("write lib");

    let (backend, _guard) = backend();
    let refs = backend
        .read_project_file_references(
            dir.path().to_str().expect("utf8 path").to_string(),
            vec!["src/lib.rs".to_string()],
        )
        .expect("read refs");

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].path, "src/lib.rs");
    assert_eq!(refs[0].content, "pub fn value() -> u8 { 7 }\n");
    assert!(!refs[0].truncated);
}

use super::*;
use crate::run::execution::{ExecutionAction, ExecutionEvent};
use crate::run::state::WorkflowRunState;
use crate::settings::model::{AppSettings, ProviderProfile, ProviderTransport};
use crate::workflow::catalog::default_workflow;
use engine::{Node, NodeId, Workflow};
use providers::ProviderId;
use tempfile::tempdir;

fn project_dir(dir: &tempfile::TempDir) -> String {
    let path = dir.path().join("project-repo");
    std::fs::create_dir_all(&path).expect("project dir");
    path.to_string_lossy().into_owned()
}

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

#[cfg_attr(miri, ignore)]
#[test]
fn start_workflow_authoring_returns_session_id() {
    let (backend, _dir) = backend();
    let session_id = backend.start_workflow_authoring(None);
    assert!(!session_id.is_empty());
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

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Workflow 1");
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
    assert!(!node.agent.auto_start);
    assert_eq!(
        node.agent.tools.approval_mode,
        Some(engine::ApprovalMode::AlwaysAsk)
    );
}

#[cfg_attr(miri, ignore)]
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

#[cfg_attr(miri, ignore)]
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

#[cfg_attr(miri, ignore)]
#[test]
fn start_run_with_entrypoint_skips_chat_for_manual_root() {
    let (backend, _dir) = backend();
    backend.block_on_test(async {
        let mut workflow = Workflow::new("Manual kickoff");
        let mut node = Node::agent("Review", 0.0, 0.0);
        node.id = NodeId("review".to_string());
        node.agent.auto_start = false;
        workflow.nodes = vec![node];

        let (initial_state, mut event_rx) = backend
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
            0
        );

        let _ = event_rx.recv().await;
        let _ = event_rx.recv().await;
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

        assert_eq!(run_state.awaiting_node_id, Some(NodeId("idea".to_string())));
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
    assert_eq!(items[0].name, "Renamed");
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

    assert!(backend.list_workflows().expect("list").is_empty());
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
    assert!(backend.list_workflows().expect("list").is_empty());
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

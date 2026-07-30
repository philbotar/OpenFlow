use engine::{Edge, Node, NodeId, Workflow};
use orchestration::adapters::storage::agent_store::FileAgentStore;
use orchestration::adapters::storage::run_attachment_store::FileRunAttachmentStore;
use orchestration::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
use orchestration::adapters::storage::settings_store::FileSettingsStore;
use orchestration::adapters::storage::skill_store::FileSkillCatalog;
use orchestration::api::UserMessageInput;
use orchestration::run::coordinator::{RunCoordinator, RunStartParams};
use orchestration::run::persistence::RunStoreRoot;
use orchestration::run::state::WorkflowRunState;
use orchestration::settings::model::AppSettings;
use orchestration::settings::ports::{SettingsStore, SkillCatalog};
use orchestration::settings::provider::ProviderEnv;
use providers::ProviderId;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

const ENABLE_ENV: &str = "OPENFLOW_LIVE_MULTI_PROVIDER_SMOKE";
const PRIMARY_PROVIDER_ENV: &str = "OPENFLOW_LIVE_AI_PRIMARY_PROVIDER";
const SECONDARY_PROVIDER_ENV: &str = "OPENFLOW_LIVE_AI_SECONDARY_PROVIDER";
const SENTINEL: &str = "ORCHID-91";

struct SmokeProvider {
    id: ProviderId,
    label: String,
    model: String,
    reasoning_effort: Option<String>,
    reasoning_budget_tokens: Option<u32>,
}

fn smoke_provider(settings: &AppSettings, id: ProviderId) -> Result<SmokeProvider, String> {
    let profile = settings
        .providers
        .get(&id)
        .ok_or_else(|| format!("saved settings do not contain provider {id}"))?;
    let model = profile
        .default_model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .ok_or_else(|| format!("provider {id} has no default model"))?
        .to_string();
    let reasoning_option = profile.reasoning_effort_options.first();
    let reasoning_effort = reasoning_option.map(|option| option.value.clone());
    let reasoning_budget_tokens = reasoning_option
        .filter(|option| option.uses_budget_tokens)
        .and_then(|option| {
            profile
                .default_reasoning_budget_tokens
                .get(&option.value)
                .copied()
        });

    Ok(SmokeProvider {
        id,
        label: profile.display_name.clone(),
        model,
        reasoning_effort,
        reasoning_budget_tokens,
    })
}

fn smoke_node(
    id: &str,
    label: &str,
    task_prompt: String,
    provider: &SmokeProvider,
    provider_override: bool,
) -> Node {
    let mut node = Node::agent(label, 0.0, 0.0);
    node.id = NodeId::from(id);
    node.agent.provider_id = provider_override.then(|| provider.id.to_string());
    node.agent.model.clone_from(&provider.model);
    node.agent
        .reasoning_effort
        .clone_from(&provider.reasoning_effort);
    node.agent.reasoning_budget_tokens = provider.reasoning_budget_tokens;
    node.agent.handoff = engine::HandoffSpec::Json;
    node.agent.system_prompt = concat!(
        "You are running an automated live multi-provider smoke test. ",
        "Do not call repository tools or request human input. ",
        "Submit only valid JSON matching the output schema. ",
        "Preserve project_code exactly."
    )
    .to_string();
    node.agent.task_prompt = task_prompt;
    node
}

fn mixed_provider_workflow(primary: &SmokeProvider, secondary: &SmokeProvider) -> Workflow {
    let mut workflow = Workflow::new("Manual live multi-provider smoke");
    workflow.settings.provider_id = Some(primary.id.to_string());

    let mut planning = smoke_node(
        "planning",
        "Plan with override provider",
        format!(
            "Read the entrypoint. Return project_code exactly as {SENTINEL}, planning_provider \
             exactly as {}, and a short plan_summary.",
            secondary.id
        ),
        secondary,
        true,
    );
    planning.agent.output_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "project_code": { "type": "string" },
            "planning_provider": { "type": "string" },
            "plan_summary": { "type": "string" }
        },
        "required": ["project_code", "planning_provider", "plan_summary"]
    });

    let mut implementation = smoke_node(
        "implementation",
        "Implement with shared provider",
        format!(
            "Read the upstream planning JSON. Preserve project_code exactly, return \
             implementation_provider exactly as {}, and add a short implementation_summary.",
            primary.id
        ),
        primary,
        false,
    );
    implementation.agent.output_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "project_code": { "type": "string" },
            "implementation_provider": { "type": "string" },
            "implementation_summary": { "type": "string" }
        },
        "required": [
            "project_code",
            "implementation_provider",
            "implementation_summary"
        ]
    });

    workflow.nodes = vec![planning, implementation];
    workflow.edges = vec![Edge::new("planning", "implementation")];
    workflow
}

async fn await_terminal_state(
    coordinator: &RunCoordinator,
    run_store: &FileRunCheckpointStore,
    initial_state: WorkflowRunState,
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<engine::RunTelemetry>,
) -> Result<WorkflowRunState, String> {
    let mut last_state = initial_state;
    let terminal = tokio::time::timeout(Duration::from_mins(3), async {
        while let Some(event) = event_rx.recv().await {
            let stop_error = match &event {
                engine::RunTelemetry::NodeErrored { node_id, error, .. }
                | engine::RunTelemetry::NodeFailed { node_id, error, .. } => {
                    Some(format!("node {node_id} failed: {error}"))
                }
                engine::RunTelemetry::Error(error) => Some(format!("workflow failed: {error}")),
                engine::RunTelemetry::Aborted => Some("workflow aborted".to_string()),
                _ => None,
            };
            match &event {
                engine::RunTelemetry::NodeStarted { node_id, label } => {
                    println!("START node={node_id} label={label}");
                }
                engine::RunTelemetry::UsageReported {
                    node_id,
                    usage,
                    model,
                    ..
                } => {
                    println!(
                        "USAGE node={node_id} model={model} total_tokens={}",
                        usage.total_tokens
                    );
                }
                engine::RunTelemetry::NodeCompleted { node_id, .. } => {
                    println!("DONE  node={node_id}");
                }
                engine::RunTelemetry::NodeAwaitingInput { node_id, .. } => {
                    println!("WAIT  node={node_id} requested human input");
                }
                engine::RunTelemetry::NodeErrored { node_id, error, .. }
                | engine::RunTelemetry::NodeFailed { node_id, error, .. }
                | engine::RunTelemetry::AiInvokeFailed { node_id, error, .. } => {
                    println!("ERROR node={node_id}: {error}");
                }
                engine::RunTelemetry::PhaseTimed {
                    phase,
                    label,
                    node_id,
                    duration_ms,
                } => {
                    println!(
                        "TIME  phase={phase} node={} label={label} duration_ms={duration_ms}",
                        node_id
                            .as_ref()
                            .map_or_else(|| "-".to_string(), ToString::to_string)
                    );
                }
                engine::RunTelemetry::OutputRepairStarted { node_id, model } => {
                    println!("REPAIR node={node_id} model={model}");
                }
                engine::RunTelemetry::OutputRepairFailed { node_id, reason } => {
                    println!("REPAIR-FAIL node={node_id}: {reason}");
                }
                engine::RunTelemetry::Finished(_) => println!("FINISH workflow"),
                engine::RunTelemetry::Error(error) => println!("ERROR workflow: {error}"),
                engine::RunTelemetry::Aborted => println!("ABORT workflow"),
                _ => {}
            }
            last_state = coordinator
                .apply_execution_event(event, run_store)
                .await
                .map_err(|error| format!("apply execution event: {error}"))?;
            if !last_state.active {
                return Ok(last_state.clone());
            }
            if let Some(error) = stop_error {
                return Err(error);
            }
        }
        Err("run event channel closed before a terminal state".to_string())
    })
    .await;

    if let Ok(state) = terminal {
        return state;
    }

    let _ = coordinator.stop_run().await;
    Err(format!(
        "mixed-provider run did not finish within 180 seconds; status={:?}; last_error={:?}",
        last_state.status_by_node, last_state.last_error
    ))
}

fn assert_provider_output(
    state: &WorkflowRunState,
    node_id: &str,
    provider_field: &str,
    provider: &SmokeProvider,
) -> Result<(), String> {
    let output = state
        .outputs
        .get(&NodeId::from(node_id))
        .ok_or_else(|| format!("node {node_id} produced no output"))?;
    if output["project_code"] != SENTINEL {
        return Err(format!("node {node_id} did not preserve {SENTINEL}"));
    }
    if output[provider_field] != provider.id.as_str() {
        return Err(format!(
            "node {node_id} did not report {} in {provider_field}: {output}",
            provider.id
        ));
    }
    let usage = state
        .context_window_by_node
        .get(&NodeId::from(node_id))
        .ok_or_else(|| format!("node {node_id} emitted no usage record"))?;
    if usage.model != provider.model {
        return Err(format!(
            "node {node_id} used model {} instead of {}",
            usage.model, provider.model
        ));
    }
    Ok(())
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
#[ignore = "manual only: run ./scripts/smoke-live-multi-provider.sh"]
async fn saved_profiles_complete_one_mixed_provider_workflow() {
    assert_eq!(
        std::env::var(ENABLE_ENV).as_deref(),
        Ok("1"),
        "run this manual smoke via ./scripts/smoke-live-multi-provider.sh"
    );

    let settings_path = FileSettingsStore::default_path();
    let settings = FileSettingsStore::new(&settings_path)
        .load()
        .unwrap_or_else(|error| panic!("load {}: {error}", settings_path.display()));
    let primary_id = std::env::var(PRIMARY_PROVIDER_ENV)
        .map_or_else(|_| settings.active_provider.clone(), ProviderId::from);
    let secondary_id = std::env::var(SECONDARY_PROVIDER_ENV)
        .map(ProviderId::from)
        .expect("set OPENFLOW_LIVE_AI_SECONDARY_PROVIDER to a configured provider");
    assert_ne!(
        primary_id, secondary_id,
        "primary and secondary provider must differ"
    );
    let primary = smoke_provider(&settings, primary_id).expect("load primary provider profile");
    let secondary =
        smoke_provider(&settings, secondary_id).expect("load secondary provider profile");
    println!(
        "Live mixed-provider smoke: override={} ({}) model={} -> shared={} ({}) model={}",
        secondary.label, secondary.id, secondary.model, primary.label, primary.id, primary.model
    );

    let temp = tempdir().expect("tempdir");
    let agent_store = FileAgentStore::new(temp.path().join("agents.json"));
    let skill_catalog = FileSkillCatalog;
    let settings_store: Arc<dyn SettingsStore> = Arc::new(FileSettingsStore::new(&settings_path));
    let run_store = FileRunCheckpointStore;
    let env = ProviderEnv::from_system();
    let coordinator = RunCoordinator::with_attachment_store(
        tokio::runtime::Handle::current(),
        Arc::new(FileRunAttachmentStore::default()),
    );
    let workflow = mixed_provider_workflow(&primary, &secondary);
    let (initial_state, event_rx) = coordinator
        .start_run(RunStartParams {
            workflow,
            invoked_skill_ids: Vec::new(),
            entrypoint: Some(UserMessageInput::text(format!(
                "Create a tiny implementation plan for project_code {SENTINEL}."
            ))),
            execution_cwd: Some(temp.path().display().to_string()),
            run_root: RunStoreRoot {
                project_id: None,
                root: temp.path().join("runs"),
            },
            settings: &settings,
            transient_api_key: None,
            agent_store: &agent_store,
            skill_catalog: &skill_catalog as &dyn SkillCatalog,
            settings_store,
            run_store: &run_store,
            env: &env,
        })
        .await
        .expect("start mixed-provider workflow");
    let final_state =
        match await_terminal_state(&coordinator, &run_store, initial_state, event_rx).await {
            Ok(state) => state,
            Err(error) => {
                let _ = coordinator.stop_run().await;
                panic!("finish mixed-provider workflow: {error}");
            }
        };
    assert!(
        final_state.last_error.is_none(),
        "mixed-provider run failed: {:?}",
        final_state.last_error
    );
    assert_provider_output(&final_state, "planning", "planning_provider", &secondary)
        .expect("validate override-provider node");
    assert_provider_output(
        &final_state,
        "implementation",
        "implementation_provider",
        &primary,
    )
    .expect("validate shared-provider node");

    println!(
        "PASS  mixed-provider workflow: planning={} implementation={}",
        secondary.id, primary.id
    );
}

use crate::run::persistence::PendingRunCheckpoint;
use crate::tools::{ArtifactStore, ToolRegistry, ToolRunner};
use engine::{
    AiPort, EditBatch, InteractiveEngine, InteractiveEngineCheckpoint, OutputRepairPolicy,
    RepairingAiPort, Workflow,
};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::super::ai_adapter::AiInvocationAdapter;
use super::super::tool_port::ToolPortImpl;
use super::super::InteractiveWorkflowRunParams;
use super::super::ResumeContinuation;

/// Run-scoped AI stack: provider → overseer repair → invocation adapter.
type RunAiAdapter<A> = AiInvocationAdapter<RepairingAiPort<A>>;

/// Wired ports and engine state for one interactive run.
pub(super) struct RunWiring<A>
where
    A: AiPort + Send + Sync + 'static,
{
    pub engine: InteractiveEngine,
    pub ai_adapter: Arc<RunAiAdapter<A>>,
    pub review_ai: Arc<RepairingAiPort<A>>,
    pub tool_port: ToolPortImpl<RunAiAdapter<A>>,
    pub workflow: Arc<Workflow>,
    pub pending_engine_reverts: Arc<Mutex<Vec<EditBatch>>>,
    pub checkpoint_sink: Arc<Mutex<Option<PendingRunCheckpoint>>>,
    pub aborted_emitted: Arc<Mutex<bool>>,
    pub mcp_client_request_rx:
        Option<tokio::sync::mpsc::UnboundedReceiver<crate::adapters::mcp::McpRunClientRequest>>,
}

/// Construct a fresh engine or restore one from a persisted checkpoint.
/// Resumed runs call `prepare_resume()` to re-queue nodes that were mid-flight.
fn build_engine(
    workflow: Workflow,
    entrypoint: Option<String>,
    entrypoint_attachments: Vec<engine::ChatAttachmentRef>,
    resume_checkpoint: Option<InteractiveEngineCheckpoint>,
    resume_continuation: Option<ResumeContinuation>,
    project_repository_root: Option<String>,
) -> Result<InteractiveEngine, String> {
    match resume_checkpoint {
        Some(checkpoint) => {
            let awaiting_continuation = resume_continuation.as_ref().is_some_and(|continuation| {
                checkpoint.awaiting_nodes.contains(&continuation.node_id)
            });
            let mut engine =
                InteractiveEngine::from_checkpoint(workflow, checkpoint, project_repository_root)
                    .map_err(|error| error.to_string())?;
            if let Some(continuation) = resume_continuation {
                let result = if awaiting_continuation {
                    engine.on_human_message(
                        &continuation.node_id,
                        &continuation.text,
                        continuation.attachments,
                    )
                } else {
                    engine.retry_node_with_message(
                        &continuation.node_id,
                        &continuation.text,
                        continuation.attachments,
                    )
                };
                result.map_err(|error| error.to_string())?;
                if let Some(skill_prompt) = continuation.skill_prompt {
                    engine.append_system_prompt(&continuation.node_id, &skill_prompt);
                }
            }
            let failures = engine.prepare_resume();
            if !failures.is_empty() {
                log::warn!("prepare_resume could not retry nodes: {failures:?}");
            }
            Ok(engine)
        }
        None => InteractiveEngine::new_with_entrypoint_attachments(
            workflow,
            entrypoint,
            entrypoint_attachments,
            project_repository_root,
        )
        .map_err(|error| error.to_string()),
    }
}

pub fn new_artifact_root() -> PathBuf {
    std::env::temp_dir().join(format!("openflow-run-{}", Uuid::new_v4()))
}

#[must_use]
pub fn new_in_memory_snapshot_store(
) -> Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore> {
    Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new())
}

async fn close_mcp_after_setup_error(
    clients: &crate::adapters::mcp::McpRunClients,
    error: String,
) -> String {
    if let Err(close_error) = clients.close().await {
        log::warn!("failed to close MCP clients after run setup error: {close_error}");
    }
    error
}

fn report_skipped_mcp(
    event_tx: &tokio::sync::mpsc::UnboundedSender<super::super::ExecutionEvent>,
    notice_node_id: Option<&engine::NodeId>,
    message: String,
) {
    log::warn!("{message}");
    let Some(node_id) = notice_node_id else {
        return;
    };
    super::super::send_or_log(
        event_tx,
        super::super::ExecutionEvent::ChatMessage {
            node_id: node_id.clone(),
            role: engine::ChatRole::System,
            content: message,
        },
    );
}

pub(super) async fn wire_run<A>(
    params: InteractiveWorkflowRunParams<A>,
    event_tx: tokio::sync::mpsc::UnboundedSender<super::super::ExecutionEvent>,
    cancel_token: CancellationToken,
) -> Result<RunWiring<A>, String>
where
    A: AiPort + Send + Sync + 'static,
{
    let InteractiveWorkflowRunParams {
        mut workflow,
        entrypoint,
        entrypoint_attachments,
        execution_cwd,
        project_repository_root,
        artifact_root,
        attachment_root,
        attachment_store,
        resume_checkpoint,
        resume_continuation,
        checkpoint_sink,
        ai,
        agent_snapshots,
        snapshot_store,
        lsp,
        pending_engine_reverts,
        node_interrupts,
        context_window_sizes,
        mcp,
        prepared_mcp,
        search,
        runtime_config_store,
        tool_budget,
        mutation_gate: _,
    } = params;

    let mcp_notice_node_id = entrypoint
        .as_deref()
        .and_then(|id| workflow.nodes.iter().find(|node| node.id.0 == id))
        .or_else(|| workflow.nodes.first())
        .map(|node| node.id.clone());
    let mut tool_registry = ToolRegistry::new();
    let effective_servers = crate::adapters::mcp::effective_mcp_servers(&mcp, &execution_cwd);
    let effective_mcp = crate::settings::model::McpSettings {
        servers: effective_servers,
        discover_external: mcp.discover_external,
        disabled_discovered_ids: mcp.disabled_discovered_ids.clone(),
        registry_base_url: mcp.registry_base_url.clone(),
    };

    let (mcp_clients, mut mcp_issues) = match prepared_mcp {
        Some(prepared) => prepared,
        None => {
            crate::adapters::mcp::McpRunClients::connect_for_run(
                &effective_mcp,
                project_repository_root.as_deref(),
            )
            .await
        }
    };
    mcp_clients.resolve_workflow_context(&mut workflow).await;
    let mut engine = match build_engine(
        workflow.clone(),
        entrypoint,
        entrypoint_attachments,
        resume_checkpoint,
        resume_continuation,
        project_repository_root
            .as_ref()
            .map(|path| path.display().to_string()),
    ) {
        Ok(engine) => engine,
        Err(error) => return Err(close_mcp_after_setup_error(&mcp_clients, error).await),
    };
    engine.set_runtime_config_store(runtime_config_store.clone());
    let (definitions, definition_issues) = mcp_clients.list_all_tool_definitions().await;
    mcp_issues.extend(definition_issues);
    for issue in mcp_issues {
        report_skipped_mcp(&event_tx, mcp_notice_node_id.as_ref(), issue.to_string());
    }

    for definition in definitions {
        let tool_name = definition.name.clone();
        let tool = crate::tool::registry::RegisteredTool {
            definition,
            kind: crate::tool::registry::BuiltinToolKind::Mcp,
        };
        if let Err(error) = tool_registry.extend_mcp(vec![tool]) {
            report_skipped_mcp(
                &event_tx,
                mcp_notice_node_id.as_ref(),
                format!("MCP tool `{tool_name}` was skipped: {error}"),
            );
        }
    }

    if search.enabled
        && search.has_configured_keys()
        && crate::tool::web_search::resolve_binary(&search).is_ok()
    {
        tool_registry.register_web_search();
    }

    let handoff_root = crate::run::handoff::handoff_root_for_artifact_root(&artifact_root);
    let artifacts = match ArtifactStore::new(artifact_root) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            return Err(close_mcp_after_setup_error(&mcp_clients, error.to_string()).await);
        }
    };

    let mcp_client_request_rx = mcp_clients.take_client_request_receiver();
    let tool_runner = Arc::new(
        ToolRunner::new(
            tool_registry,
            execution_cwd,
            artifacts,
            cancel_token.clone(),
            snapshot_store,
        )
        .with_mcp_clients(mcp_clients)
        .with_search_settings(search),
    );
    let workflow = Arc::new(workflow);
    let handoff_specs = workflow
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.agent.handoff.clone()))
        .collect();
    let repair_policy = OutputRepairPolicy::from_workflow_settings(&workflow.settings);
    let repairing = Arc::new(RepairingAiPort::new(ai, repair_policy));
    let node_interrupts_for_tools = node_interrupts.clone();
    let ai_adapter = Arc::new(
        AiInvocationAdapter::new(
            Arc::clone(&repairing),
            event_tx.clone(),
            node_interrupts,
            cancel_token.clone(),
            context_window_sizes,
        )
        .with_attachment_store(attachment_root, attachment_store)
        .with_handoff_store(handoff_root, handoff_specs),
    );
    let aborted_emitted = Arc::new(Mutex::new(false));
    let tool_port = ToolPortImpl::new(
        Arc::clone(&tool_runner),
        lsp,
        Arc::clone(&workflow),
        Arc::new(agent_snapshots),
        Arc::clone(&ai_adapter),
        cancel_token,
        event_tx,
        node_interrupts_for_tools,
        Arc::clone(&aborted_emitted),
        runtime_config_store,
        tool_budget,
    );

    Ok(RunWiring {
        engine,
        ai_adapter,
        review_ai: repairing,
        tool_port,
        workflow,
        pending_engine_reverts,
        checkpoint_sink,
        aborted_emitted,
        mcp_client_request_rx,
    })
}

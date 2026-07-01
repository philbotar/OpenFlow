use crate::run::persistence::PendingRunCheckpoint;
use crate::tools::{ArtifactStore, ToolRegistry, ToolRunner};
use engine::{AiPort, EditBatch, InteractiveEngine, InteractiveEngineCheckpoint, Workflow};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::super::ai_adapter::AiInvocationAdapter;
use super::super::tool_port::ToolPortImpl;
use super::super::InteractiveWorkflowRunParams;

/// Wired ports and engine state for one interactive run.
pub(super) struct RunWiring<A>
where
    A: AiPort + Send + Sync + 'static,
{
    pub engine: InteractiveEngine,
    pub ai_adapter: Arc<AiInvocationAdapter<A>>,
    pub tool_port: ToolPortImpl<AiInvocationAdapter<A>>,
    pub workflow: Arc<Workflow>,
    pub pending_engine_reverts: Arc<Mutex<Vec<EditBatch>>>,
    pub checkpoint_sink: Arc<Mutex<Option<PendingRunCheckpoint>>>,
    pub aborted_emitted: Arc<Mutex<bool>>,
}

/// Construct a fresh engine or restore one from a persisted checkpoint.
/// Resumed runs call `prepare_resume()` to re-queue nodes that were mid-flight.
fn build_engine(
    workflow: Workflow,
    entrypoint: Option<String>,
    resume_checkpoint: Option<InteractiveEngineCheckpoint>,
    project_repository_root: Option<String>,
) -> Result<InteractiveEngine, String> {
    match resume_checkpoint {
        Some(checkpoint) => {
            InteractiveEngine::from_checkpoint(workflow, checkpoint, project_repository_root)
                .map(|mut engine| {
                    let failures = engine.prepare_resume();
                    if !failures.is_empty() {
                        log::warn!("prepare_resume could not retry nodes: {failures:?}");
                    }
                    engine
                })
                .map_err(|error| error.to_string())
        }
        None => InteractiveEngine::new(workflow, entrypoint, project_repository_root)
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

pub(super) async fn wire_run<A>(
    params: InteractiveWorkflowRunParams<A>,
    event_tx: tokio::sync::mpsc::UnboundedSender<super::super::ExecutionEvent>,
    cancel_token: CancellationToken,
) -> Result<RunWiring<A>, String>
where
    A: AiPort + Send + Sync + 'static,
{
    let InteractiveWorkflowRunParams {
        workflow,
        entrypoint,
        execution_cwd,
        project_repository_root,
        artifact_root,
        resume_checkpoint,
        checkpoint_sink,
        ai,
        agent_snapshots,
        snapshot_store,
        lsp,
        pending_engine_reverts,
        node_interrupts,
        context_window_sizes,
        mcp,
    } = params;

    let engine = build_engine(
        workflow.clone(),
        entrypoint,
        resume_checkpoint,
        project_repository_root
            .as_ref()
            .map(|path| path.display().to_string()),
    )?;

    let mut tool_registry = ToolRegistry::new();
    let effective_servers = crate::adapters::mcp::effective_mcp_servers(&mcp, &execution_cwd);
    let effective_mcp = crate::settings::model::McpSettings {
        servers: effective_servers,
        discover_external: mcp.discover_external,
        disabled_discovered_ids: mcp.disabled_discovered_ids.clone(),
    };

    let mcp_clients = crate::adapters::mcp::McpRunClients::connect(&effective_mcp)
        .await
        .map_err(|error| error.to_string())?;

    let definitions = mcp_clients
        .list_all_tool_definitions()
        .await
        .map_err(|error| error.to_string())?;
    let mcp_tools = definitions
        .into_iter()
        .map(|definition| crate::tool::registry::RegisteredTool {
            definition,
            kind: crate::tool::registry::BuiltinToolKind::Mcp,
        })
        .collect();
    tool_registry
        .extend_mcp(mcp_tools)
        .map_err(|error| error.to_string())?;

    let artifacts = ArtifactStore::new(artifact_root).map_err(|error| error.to_string())?;

    let tool_runner = Arc::new(
        ToolRunner::new(
            tool_registry,
            execution_cwd,
            artifacts,
            cancel_token.clone(),
            snapshot_store,
        )
        .with_mcp_clients(mcp_clients),
    );
    let workflow = Arc::new(workflow);
    let ai = Arc::new(ai);
    let node_interrupts_for_tools = node_interrupts.clone();
    let ai_adapter = Arc::new(AiInvocationAdapter::new(
        Arc::clone(&ai),
        event_tx.clone(),
        node_interrupts,
        cancel_token.clone(),
        context_window_sizes,
    ));
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
    );

    Ok(RunWiring {
        engine,
        ai_adapter,
        tool_port,
        workflow,
        pending_engine_reverts,
        checkpoint_sink,
        aborted_emitted,
    })
}

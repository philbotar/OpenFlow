mod ai_adapter;
mod drive;
mod events;
mod headless;
mod subagents;
mod tool_port;

pub use ai_adapter::AiInvocationAdapter;

use crate::lsp::LspSettings;
use crate::run::persistence::PendingRunCheckpoint;
use crate::run::state::{RunTraceEntry, ToolArtifactSummary, ToolCallSummary};
use engine::{
    AiPort, CallableAgent, ChatMessage, EditBatch, InteractiveEngineCheckpoint, NodeId, RunReport,
    RunTelemetry, Workflow,
};
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;

pub use drive::{new_artifact_root, new_in_memory_snapshot_store};
pub use events::{
    apply_event_to_run_state, record_entrypoint_message, record_user_input,
    should_record_entrypoint_in_chat,
};
pub use headless::run_workflow_headless;

/// Interactive run telemetry; canonical type is [`engine::RunTelemetry`].
pub type ExecutionEvent = RunTelemetry;

/// Per-node interrupt tokens keyed by node id and model attempt.
pub type NodeInterrupts = Arc<Mutex<BTreeMap<NodeId, (u8, CancellationToken)>>>;

pub fn send_or_log(event_tx: &UnboundedSender<ExecutionEvent>, event: ExecutionEvent) {
    if let Err(error) = event_tx.send(event) {
        log::warn!("failed to send execution event; run consumer dropped: {error:?}");
    }
}

fn emit_phase_timed(
    event_tx: &UnboundedSender<RunTelemetry>,
    phase: &str,
    label: &str,
    node_id: Option<NodeId>,
    started: Instant,
) {
    let duration_ms = started.elapsed().as_millis() as u64;
    log::info!("[perf] {phase} · {label}: {duration_ms}ms");
    let _ = event_tx.send(RunTelemetry::PhaseTimed {
        phase: phase.to_string(),
        label: label.to_string(),
        node_id,
        duration_ms,
    });
}

pub enum ExecutionAction {
    ProvideInput {
        node_id: NodeId,
        text: String,
    },
    ResolveApproval {
        approval_id: String,
        allow: bool,
        reason: Option<String>,
    },
    RetryNode {
        node_id: NodeId,
    },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualInput {
    pub node_id: NodeId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResponse {
    pub approval_id: String,
    pub allow: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunSnapshot {
    pub report: RunReport,
    pub run_trace: Vec<RunTraceEntry>,
    pub chat_logs: BTreeMap<NodeId, Vec<ChatMessage>>,
    pub outputs: BTreeMap<NodeId, Value>,
    pub pending_approvals: Vec<engine::PendingToolApproval>,
    pub tool_calls_by_node: BTreeMap<NodeId, Vec<ToolCallSummary>>,
    pub tool_artifacts: BTreeMap<String, ToolArtifactSummary>,
    pub changed_files: Vec<engine::FileChangeRecord>,
    pub edit_batches: Vec<engine::EditBatch>,
}

#[derive(Debug, Error)]
pub enum WorkflowExecutionError {
    #[error("{0}")]
    Execution(String),
    #[error("node {node_id} failed: {message}")]
    NodeFailed { node_id: NodeId, message: String },
    #[error("node {0} requested manual input but no scripted input was provided")]
    MissingManualInput(NodeId),
    #[error("tool approval {0} was requested but no scripted approval was provided")]
    MissingApproval(String),
    #[error("node {0} failed or was interrupted and requires manual retry but no scripted retry was provided")]
    MissingRetry(NodeId),
}

pub fn resolve_execution_cwd(execution_cwd: Option<&str>) -> Result<PathBuf, String> {
    match execution_cwd
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None => Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        Some(path) => {
            let expanded = expand_tilde(path);
            let canonical = expanded.canonicalize().map_err(|error| {
                format!("execution folder is not a valid directory ({path}): {error}")
            })?;
            if !canonical.is_dir() {
                return Err(format!("execution folder is not a directory: {path}"));
            }
            Ok(canonical)
        }
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(rest);
    }
    PathBuf::from(path)
}

pub struct InteractiveWorkflowRunParams<A> {
    pub workflow: Workflow,
    pub entrypoint: Option<String>,
    pub execution_cwd: PathBuf,
    pub artifact_root: PathBuf,
    pub resume_checkpoint: Option<InteractiveEngineCheckpoint>,
    pub checkpoint_sink: Arc<Mutex<Option<PendingRunCheckpoint>>>,
    pub ai: A,
    pub agent_snapshots: BTreeMap<String, CallableAgent>,
    pub snapshot_store: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    pub lsp: LspSettings,
    pub pending_engine_reverts: Arc<parking_lot::Mutex<Vec<EditBatch>>>,
    pub node_interrupts: NodeInterrupts,
    pub context_window_sizes: BTreeMap<String, u32>,
}

pub fn spawn_interactive_workflow_run<A>(
    runtime_handle: &tokio::runtime::Handle,
    params: InteractiveWorkflowRunParams<A>,
) -> (
    tokio::task::JoinHandle<()>,
    UnboundedReceiver<ExecutionEvent>,
    UnboundedSender<ExecutionAction>,
    CancellationToken,
    NodeInterrupts,
)
where
    A: AiPort + Send + Sync + 'static,
{
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();
    let drive_cancel_token = cancel_token.clone();
    let node_interrupts = params.node_interrupts.clone();
    let handle = runtime_handle.spawn(async move {
        drive::drive_interactive_workflow(params, event_tx, action_rx, drive_cancel_token).await;
    });
    (handle, event_rx, action_tx, cancel_token, node_interrupts)
}

#[cfg(test)]
mod tests;

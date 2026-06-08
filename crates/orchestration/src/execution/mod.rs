#![allow(
    clippy::derive_partial_eq_without_eq,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::missing_panics_doc,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::redundant_clone,
    clippy::significant_drop_tightening,
    clippy::too_many_lines
)]

mod drive;
mod events;
mod headless;
mod subagents;

use crate::agent_store::AgentDefinition;
use crate::state::{RunTraceEntry, ToolArtifactSummary, ToolCallSummary};
use domain::{
    AiPort, ChatMessage, ChatRole, NodeId, RunReport, SubagentSummary, ToolCall, Workflow,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub use events::{apply_event_to_run_state, record_user_input};
pub use headless::run_workflow_headless;

/// Collect snapshotted agent definitions referenced by workflow node settings.
#[must_use]
pub fn resolve_callable_agent_snapshots(
    workflow: &Workflow,
    agents: &[AgentDefinition],
) -> BTreeMap<String, AgentDefinition> {
    let mut requested = HashSet::new();
    for node in &workflow.nodes {
        if node.agent.allow_all_callable_agents {
            for agent in agents {
                requested.insert(agent.id.clone());
            }
        } else {
            for id in &node.agent.callable_agents {
                if !id.trim().is_empty() {
                    requested.insert(id.clone());
                }
            }
        }
    }
    agents
        .iter()
        .filter(|agent| requested.contains(&agent.id))
        .map(|agent| (agent.id.clone(), agent.clone()))
        .collect()
}

#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    NodeQueued {
        node_id: NodeId,
        label: String,
    },
    NodeStarted {
        node_id: NodeId,
        label: String,
    },
    ChatMessage {
        node_id: NodeId,
        role: ChatRole,
        content: String,
    },
    NodeAwaitingInput {
        node_id: NodeId,
        label: String,
        context: String,
        is_initial: bool,
    },
    ToolCallProposed {
        node_id: NodeId,
        label: String,
        tool_call: ToolCall,
    },
    ToolApprovalRequested {
        request: domain::PendingToolApproval,
    },
    ToolApproved {
        approval_id: String,
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
    },
    ToolDenied {
        approval_id: String,
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        reason: String,
    },
    ToolStarted {
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        arguments: Value,
    },
    ToolCompleted {
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        content: String,
        is_error: bool,
        output_meta: Option<domain::ToolOutputMeta>,
        artifact_ids: Vec<String>,
    },
    ToolArtifactCreated {
        node_id: NodeId,
        artifact: ToolArtifactSummary,
    },
    NodeCompleted {
        node_id: NodeId,
        label: String,
        output: Value,
    },
    NodeFailed {
        node_id: NodeId,
        label: String,
        error: String,
    },
    Finished(RunReport),
    Error(String),
    SubagentsDeclared {
        node_id: NodeId,
        summaries: Vec<SubagentSummary>,
    },
    SubagentStarted {
        node_id: NodeId,
        subagent_id: String,
    },
    SubagentCompleted {
        node_id: NodeId,
        subagent_id: String,
    },
    SubagentFailed {
        node_id: NodeId,
        subagent_id: String,
        error: String,
    },
}

pub enum ExecutionAction {
    ProvideInput(String),
    ResolveApproval { approval_id: String, allow: bool },
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunSnapshot {
    pub report: RunReport,
    pub run_trace: Vec<RunTraceEntry>,
    pub chat_logs: BTreeMap<NodeId, Vec<ChatMessage>>,
    pub outputs: BTreeMap<NodeId, Value>,
    pub pending_approvals: Vec<domain::PendingToolApproval>,
    pub tool_calls_by_node: BTreeMap<NodeId, Vec<ToolCallSummary>>,
    pub tool_artifacts: BTreeMap<String, ToolArtifactSummary>,
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

pub fn spawn_interactive_workflow_run<A>(
    runtime: &tokio::runtime::Runtime,
    workflow: Workflow,
    entrypoint: Option<String>,
    execution_cwd: PathBuf,
    ai: A,
    agent_snapshots: BTreeMap<String, AgentDefinition>,
) -> (
    tokio::task::JoinHandle<()>,
    UnboundedReceiver<ExecutionEvent>,
    UnboundedSender<ExecutionAction>,
)
where
    A: AiPort + Send + Sync + 'static,
{
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = runtime.spawn(async move {
        drive::drive_interactive_workflow(
            workflow,
            entrypoint,
            execution_cwd,
            ai,
            event_tx,
            action_rx,
            agent_snapshots,
        )
        .await;
    });
    (handle, event_rx, action_tx)
}

#[cfg(test)]
mod tests;

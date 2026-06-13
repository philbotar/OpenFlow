use super::{InteractiveEngine, PendingToolBatch};
use crate::conversation::AgentTranscriptItem;
use crate::execution::RunEvent;
use crate::graph::validation::WorkflowValidationError;
use crate::graph::{NodeId, Workflow, WorkflowId};
use crate::tools::{FileChangeRecord, ToolCall};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointPendingToolBatch {
    pub approval_id: String,
    pub node_id: NodeId,
    pub tool_calls: Vec<ToolCall>,
    pub requires_approval: bool,
}

impl From<&PendingToolBatch> for CheckpointPendingToolBatch {
    fn from(batch: &PendingToolBatch) -> Self {
        Self {
            approval_id: batch.approval_id.clone(),
            node_id: batch.node_id.clone(),
            tool_calls: batch.tool_calls.clone(),
            requires_approval: batch.requires_approval,
        }
    }
}

impl From<&CheckpointPendingToolBatch> for PendingToolBatch {
    fn from(batch: &CheckpointPendingToolBatch) -> Self {
        Self {
            approval_id: batch.approval_id.clone(),
            node_id: batch.node_id.clone(),
            tool_calls: batch.tool_calls.clone(),
            requires_approval: batch.requires_approval,
        }
    }
}

/// Serializable engine state for in-session resume after user stop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveEngineCheckpoint {
    pub workflow_id: WorkflowId,
    pub layer_idx: usize,
    pub outputs: BTreeMap<NodeId, serde_json::Value>,
    pub changed_files_by_node: BTreeMap<NodeId, Vec<FileChangeRecord>>,
    pub transcripts: BTreeMap<NodeId, Vec<AgentTranscriptItem>>,
    pub events: Vec<RunEvent>,
    pub queued_nodes: BTreeSet<NodeId>,
    pub started_invocations_by_node: BTreeMap<NodeId, u8>,
    pub awaiting_nodes: BTreeSet<NodeId>,
    pub pending_tool_batches: BTreeMap<String, CheckpointPendingToolBatch>,
    pub retries_by_node: BTreeMap<NodeId, u8>,
    pub pending_retry_delay_ms: Option<u64>,
    pub submit_output_retries_by_node: BTreeMap<NodeId, u8>,
    pub request_input_retries_by_node: BTreeMap<NodeId, u8>,
    pub entrypoint_text: Option<String>,
    pub interrupted_nodes: BTreeSet<NodeId>,
    pub failed_nodes: BTreeMap<NodeId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CheckpointError {
    #[error(transparent)]
    Validation(#[from] WorkflowValidationError),
    #[error("checkpoint workflow id {checkpoint} does not match workflow {workflow}")]
    WorkflowMismatch {
        checkpoint: WorkflowId,
        workflow: WorkflowId,
    },
}

impl InteractiveEngine {
    /// Normalize engine state for stop and produce a resumable checkpoint.
    pub fn prepare_stop_checkpoint(&mut self) -> InteractiveEngineCheckpoint {
        self.interrupted_nodes
            .extend(std::mem::take(&mut self.in_flight_ai));
        self.pending_tool_batches.retain(|_, batch| {
            batch.requires_approval || !self.interrupted_nodes.contains(&batch.node_id)
        });
        self.pending_retry_delay = None;

        InteractiveEngineCheckpoint {
            workflow_id: self.workflow.id.clone(),
            layer_idx: self.layer_idx,
            outputs: self.outputs.clone(),
            changed_files_by_node: self.changed_files_by_node.clone(),
            transcripts: self.transcripts.clone(),
            events: self.events.clone(),
            queued_nodes: self.queued_nodes.clone(),
            started_invocations_by_node: self.started_invocations_by_node.clone(),
            awaiting_nodes: self.awaiting_nodes.clone(),
            pending_tool_batches: self
                .pending_tool_batches
                .iter()
                .map(|(id, batch)| (id.clone(), CheckpointPendingToolBatch::from(batch)))
                .collect(),
            retries_by_node: self.retries_by_node.clone(),
            pending_retry_delay_ms: self
                .pending_retry_delay
                .and_then(|delay| u64::try_from(delay.as_millis()).ok()),
            submit_output_retries_by_node: self.submit_output_retries_by_node.clone(),
            request_input_retries_by_node: self.request_input_retries_by_node.clone(),
            entrypoint_text: self.entrypoint_text.clone(),
            interrupted_nodes: self.interrupted_nodes.clone(),
            failed_nodes: self.failed_nodes.clone(),
        }
    }

    /// Restore an engine from a checkpoint for the given workflow.
    ///
    /// # Errors
    /// Returns an error when the workflow fails validation or ids do not match.
    pub fn from_checkpoint(
        workflow: Workflow,
        checkpoint: InteractiveEngineCheckpoint,
    ) -> Result<Self, CheckpointError> {
        if workflow.id != checkpoint.workflow_id {
            return Err(CheckpointError::WorkflowMismatch {
                checkpoint: checkpoint.workflow_id,
                workflow: workflow.id,
            });
        }

        let layers = crate::graph::validation::execution_layers(&workflow)?;
        let upstream_map = crate::execution::build_upstream_map(&workflow);
        let node_index = workflow
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.clone(), index))
            .collect();

        Ok(Self {
            workflow,
            upstream_map,
            node_index,
            layers,
            layer_idx: checkpoint.layer_idx,
            outputs: checkpoint.outputs,
            changed_files_by_node: checkpoint.changed_files_by_node,
            transcripts: checkpoint.transcripts,
            events: checkpoint.events,
            queued_nodes: checkpoint.queued_nodes,
            started_invocations_by_node: checkpoint.started_invocations_by_node,
            awaiting_nodes: checkpoint.awaiting_nodes,
            in_flight_ai: BTreeSet::new(),
            pending_tool_batches: checkpoint
                .pending_tool_batches
                .iter()
                .map(|(id, batch)| (id.clone(), PendingToolBatch::from(batch)))
                .collect(),
            retries_by_node: checkpoint.retries_by_node,
            pending_retry_delay: checkpoint.pending_retry_delay_ms.map(Duration::from_millis),
            submit_output_retries_by_node: checkpoint.submit_output_retries_by_node,
            request_input_retries_by_node: checkpoint.request_input_retries_by_node,
            entrypoint_text: checkpoint.entrypoint_text,
            terminal_error: None,
            interrupted_nodes: checkpoint.interrupted_nodes,
            failed_nodes: checkpoint.failed_nodes,
        })
    }

    /// Auto-retry interrupted nodes so continue does not require manual retry.
    pub fn prepare_resume(&mut self) {
        let interrupted = self.interrupted_nodes.iter().cloned().collect::<Vec<_>>();
        for node_id in interrupted {
            let _ = self.retry_node(&node_id);
        }
    }
}

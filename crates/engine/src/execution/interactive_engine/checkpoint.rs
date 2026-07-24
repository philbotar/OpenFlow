use super::{FrozenChangeEvidencePacket, InteractiveEngine, PendingToolBatch};
use crate::conversation::AgentTranscriptItem;
use crate::graph::validation::WorkflowValidationError;
use crate::graph::{NodeId, Workflow, WorkflowId};
use crate::ports::ToolAccessPolicy;
use crate::tools::{FileChangeRecord, ReadRecord, ToolCall};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointPendingToolBatch {
    pub approval_id: String,
    pub node_id: NodeId,
    pub tool_calls: Vec<ToolCall>,
    pub requires_approval: bool,
    #[serde(default)]
    pub tool_access_policy: ToolAccessPolicy,
}

impl From<&PendingToolBatch> for CheckpointPendingToolBatch {
    fn from(batch: &PendingToolBatch) -> Self {
        Self {
            approval_id: batch.approval_id.clone(),
            node_id: batch.node_id.clone(),
            tool_calls: batch.tool_calls.clone(),
            requires_approval: batch.requires_approval,
            tool_access_policy: batch.tool_access_policy,
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
            tool_access_policy: batch.tool_access_policy,
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
    #[serde(default)]
    pub reads_by_node: BTreeMap<NodeId, Vec<ReadRecord>>,
    pub transcripts: BTreeMap<NodeId, Vec<AgentTranscriptItem>>,
    pub awaiting_nodes: BTreeSet<NodeId>,
    #[serde(default)]
    pub plan_mode_source_node_id: Option<NodeId>,
    #[serde(default)]
    pub frozen_change_evidence_packet: Option<FrozenChangeEvidencePacket>,
    pub pending_tool_batches: BTreeMap<String, CheckpointPendingToolBatch>,
    pub retries_by_node: BTreeMap<NodeId, u8>,
    #[serde(default)]
    pub transient_streaks_by_node: BTreeMap<NodeId, u8>,
    pub submit_output_retries_by_node: BTreeMap<NodeId, u8>,
    pub request_input_retries_by_node: BTreeMap<NodeId, u8>,
    #[serde(default)]
    pub empty_turn_retries_by_node: BTreeMap<NodeId, u8>,
    #[serde(default)]
    pub mixed_tool_turn_retries_by_node: BTreeMap<NodeId, u8>,
    #[serde(default)]
    pub auto_continue_streaks_by_node: BTreeMap<NodeId, u8>,
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
    #[error("checkpoint references node ids not present in workflow: {missing:?}")]
    StaleNodeIds { missing: Vec<NodeId> },
    #[error("checkpoint has an invalid frozen change evidence packet")]
    InvalidFrozenChangeEvidencePacket,
}

#[must_use]
fn collect_checkpoint_node_ids(checkpoint: &InteractiveEngineCheckpoint) -> BTreeSet<NodeId> {
    let mut ids = BTreeSet::new();
    ids.extend(checkpoint.outputs.keys().cloned());
    ids.extend(checkpoint.transcripts.keys().cloned());
    ids.extend(checkpoint.changed_files_by_node.keys().cloned());
    ids.extend(checkpoint.reads_by_node.keys().cloned());
    ids.extend(checkpoint.awaiting_nodes.iter().cloned());
    ids.extend(checkpoint.plan_mode_source_node_id.iter().cloned());
    ids.extend(
        checkpoint
            .frozen_change_evidence_packet
            .iter()
            .map(|packet| packet.source_node_id.clone()),
    );
    ids.extend(checkpoint.interrupted_nodes.iter().cloned());
    ids.extend(checkpoint.failed_nodes.keys().cloned());
    ids.extend(checkpoint.retries_by_node.keys().cloned());
    ids.extend(
        checkpoint
            .pending_tool_batches
            .values()
            .map(|batch| batch.node_id.clone()),
    );
    ids
}

/// # Errors
/// Returns `CheckpointError::WorkflowMismatch` or `CheckpointError::StaleNodeIds` when invalid.
pub fn validate_checkpoint_against_workflow(
    workflow: &Workflow,
    checkpoint: &InteractiveEngineCheckpoint,
) -> Result<(), CheckpointError> {
    if workflow.id != checkpoint.workflow_id {
        return Err(CheckpointError::WorkflowMismatch {
            checkpoint: checkpoint.workflow_id.clone(),
            workflow: workflow.id.clone(),
        });
    }
    let valid: BTreeSet<NodeId> = workflow.nodes.iter().map(|n| n.id.clone()).collect();
    let referenced = collect_checkpoint_node_ids(checkpoint);
    let missing: Vec<NodeId> = referenced
        .into_iter()
        .filter(|id| !valid.contains(id))
        .collect();
    if !missing.is_empty() {
        return Err(CheckpointError::StaleNodeIds { missing });
    }
    if let Some(packet) = &checkpoint.frozen_change_evidence_packet {
        let source_node_id = checkpoint.plan_mode_source_node_id.as_ref().or_else(|| {
            workflow
                .settings
                .plan_mode
                .as_ref()
                .map(|plan_mode| &plan_mode.evidence_source_node_id)
        });
        if !packet.is_valid()
            || source_node_id != Some(&packet.source_node_id)
            || checkpoint.outputs.get(&packet.source_node_id) != Some(&packet.content)
        {
            return Err(CheckpointError::InvalidFrozenChangeEvidencePacket);
        }
    }
    Ok(())
}

impl InteractiveEngine {
    /// Normalize engine state for stop and produce a resumable checkpoint.
    pub fn prepare_stop_checkpoint(&mut self) -> InteractiveEngineCheckpoint {
        self.interrupted_nodes
            .extend(std::mem::take(&mut self.in_flight_ai));
        self.retry_after_by_node.clear();

        InteractiveEngineCheckpoint {
            workflow_id: self.workflow.id.clone(),
            layer_idx: self.layer_idx,
            outputs: self.outputs.clone(),
            changed_files_by_node: self.changed_files_by_node.clone(),
            reads_by_node: self.reads_by_node.clone(),
            transcripts: self.transcripts.clone(),
            awaiting_nodes: self.awaiting_nodes.clone(),
            plan_mode_source_node_id: self.plan_mode_source_node_id.clone(),
            frozen_change_evidence_packet: self.frozen_change_evidence_packet.clone(),
            pending_tool_batches: self
                .pending_tool_batches
                .iter()
                .map(|(id, batch)| (id.clone(), CheckpointPendingToolBatch::from(batch)))
                .collect(),
            retries_by_node: self.retries_by_node.clone(),
            transient_streaks_by_node: self.transient_streaks_by_node.clone(),
            submit_output_retries_by_node: self.submit_output_retries_by_node.clone(),
            request_input_retries_by_node: self.request_input_retries_by_node.clone(),
            empty_turn_retries_by_node: self.empty_turn_retries_by_node.clone(),
            mixed_tool_turn_retries_by_node: self.mixed_tool_turn_retries_by_node.clone(),
            auto_continue_streaks_by_node: self.auto_continue_streaks_by_node.clone(),
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
        project_repository_root: Option<String>,
    ) -> Result<Self, CheckpointError> {
        validate_checkpoint_against_workflow(&workflow, &checkpoint)?;

        let plan_mode_source_node_id = checkpoint.plan_mode_source_node_id.clone().or_else(|| {
            workflow
                .settings
                .plan_mode
                .as_ref()
                .map(|plan_mode| plan_mode.evidence_source_node_id.clone())
        });

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
            reads_by_node: checkpoint.reads_by_node,
            read_calls: 0,
            redundant_reads: 0,
            tokens_in: 0,
            transcripts: checkpoint.transcripts,
            awaiting_nodes: checkpoint.awaiting_nodes,
            in_flight_ai: BTreeSet::new(),
            plan_mode_source_node_id,
            frozen_change_evidence_packet: checkpoint.frozen_change_evidence_packet,
            in_flight_tools: BTreeSet::new(),
            pending_tool_batches: checkpoint
                .pending_tool_batches
                .iter()
                .map(|(id, batch)| (id.clone(), PendingToolBatch::from(batch)))
                .collect(),
            retries_by_node: checkpoint.retries_by_node,
            transient_streaks_by_node: checkpoint.transient_streaks_by_node,
            retry_after_by_node: BTreeMap::new(),
            submit_output_retries_by_node: checkpoint.submit_output_retries_by_node,
            request_input_retries_by_node: checkpoint.request_input_retries_by_node,
            empty_turn_retries_by_node: checkpoint.empty_turn_retries_by_node,
            mixed_tool_turn_retries_by_node: checkpoint.mixed_tool_turn_retries_by_node,
            auto_continue_streaks_by_node: checkpoint.auto_continue_streaks_by_node,
            entrypoint_text: checkpoint.entrypoint_text,
            project_repository_root,
            terminal_error: None,
            interrupted_nodes: checkpoint.interrupted_nodes,
            failed_nodes: checkpoint.failed_nodes,
            runtime_config_store: None,
        })
    }

    /// Auto-retry interrupted nodes so continue does not require manual retry.
    pub fn prepare_resume(&mut self) -> Vec<NodeId> {
        let interrupted = self.interrupted_nodes.iter().cloned().collect::<Vec<_>>();
        let mut failures = Vec::new();
        for node_id in interrupted {
            if self.retry_node(&node_id).is_err() {
                failures.push(node_id);
            }
        }
        failures
    }
}

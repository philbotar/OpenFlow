//! Run telemetry and aggregated results from workflow execution.

use crate::graph::validation::WorkflowValidationError;
use crate::graph::{NodeId, WorkflowId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum RunError {
    #[error(transparent)]
    Validation(#[from] WorkflowValidationError),
    #[error("node {node_id} failed: {message}")]
    NodeFailed { node_id: NodeId, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRunOutput {
    pub node_id: NodeId,
    pub output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized as `snake_case`; legacy `PascalCase` values remain accepted for saved run reports.
#[serde(rename_all = "snake_case")]
pub enum RunEventKind {
    #[serde(alias = "Queued")]
    Queued,
    #[serde(alias = "Started")]
    Started,
    #[serde(alias = "Retrying")]
    Retrying,
    #[serde(alias = "Completed")]
    Completed,
    #[serde(alias = "Failed")]
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunEvent {
    pub node_id: NodeId,
    pub kind: RunEventKind,
    pub message: String,
    pub output: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunReport {
    pub workflow_id: WorkflowId,
    pub events: Vec<RunEvent>,
    pub outputs: Vec<NodeRunOutput>,
}

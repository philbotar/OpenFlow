//! Run errors and aggregated results from workflow execution.

use crate::graph::validation::WorkflowValidationError;
use crate::graph::{NodeId, WorkflowId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeFailureKind {
    MissingUpstreamOutput(Vec<NodeId>),
    NodeIdFromLayersNotFound,
    PendingToolNodeNotFound,
    AwaitingNodeNotFound,
    LayerStalledInFlight,
    NoRunnableNodesInLayer,
    ToolCallNodeNotFound,
    NodeMustExist,
    MisroutedCompletion(String),
    NoModelConfigured { label: String },
    Agent(String),
    EngineInput(String),
}

impl fmt::Display for NodeFailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingUpstreamOutput(missing) => {
                let upstream_list = missing
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "upstream output missing from: {upstream_list}")
            }
            Self::NodeIdFromLayersNotFound => {
                f.write_str("node id from layers not found in workflow")
            }
            Self::PendingToolNodeNotFound => f.write_str("pending tool node no longer exists"),
            Self::AwaitingNodeNotFound => f.write_str("awaiting node no longer exists"),
            Self::LayerStalledInFlight => {
                f.write_str("layer stalled waiting for in-flight model calls")
            }
            Self::NoRunnableNodesInLayer => f.write_str("no runnable nodes in current layer"),
            Self::ToolCallNodeNotFound => f.write_str("tool-call node no longer exists"),
            Self::NodeMustExist => f.write_str("node must exist"),
            Self::MisroutedCompletion(message)
            | Self::Agent(message)
            | Self::EngineInput(message) => f.write_str(message),
            Self::NoModelConfigured { label } => write!(
                f,
                "node \"{label}\" has no model configured — select a model in the inspector before running"
            ),
        }
    }
}

#[derive(Debug, Clone, Error)]
pub enum RunError {
    #[error(transparent)]
    Validation(#[from] WorkflowValidationError),
    #[error("node {node_id} failed: {kind}")]
    NodeFailed {
        node_id: NodeId,
        kind: NodeFailureKind,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRunOutput {
    pub node_id: NodeId,
    pub output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunReport {
    pub workflow_id: WorkflowId,
    pub outputs: Vec<NodeRunOutput>,
    #[serde(default)]
    pub read_calls: u32,
    #[serde(default)]
    pub redundant_reads: u32,
    #[serde(default)]
    pub tokens_in: u32,
}

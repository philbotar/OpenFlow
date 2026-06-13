use crate::settings::provider::ProviderConfigError;
use engine::{NodeId, WorkflowValidationError};
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Validation(#[from] WorkflowValidationError),
    #[error(transparent)]
    ProviderConfig(#[from] ProviderConfigError),
    #[error("workflow {0} not found")]
    WorkflowNotFound(String),
    #[error("project {0} not found")]
    ProjectNotFound(String),
    #[error("agent {0} not found")]
    AgentNotFound(String),
    #[error("{0}")]
    InvalidExecutionCwd(String),
    #[error("{0}")]
    ProjectOperation(String),
    #[error("workflow run is not active")]
    NoActiveRun,
    #[error("no execution folder is bound to the current session")]
    NoExecutionCwd,
    #[error("workflow run is not awaiting input")]
    NoAwaitingInput,
    #[error("workflow run has no pending tool approval")]
    NoPendingApproval,
    #[error("expected input for {expected}, got {received}")]
    WrongAwaitingNode { expected: NodeId, received: NodeId },
    #[error("expected approval {expected}, got {received}")]
    WrongApprovalId { expected: String, received: String },
    #[error("workflow run channel closed")]
    RunChannelClosed,
    #[error("file edit preview failed: {0}")]
    PreviewFailed(String),
    #[error("git operation failed: {0}")]
    GitFailed(String),
    #[error("edit batch {0} not found")]
    EditBatchNotFound(String),
    #[error("node {0} cannot be interrupted in its current state")]
    NodeNotInterruptible(String),
    #[error("node {0} is not retryable")]
    NodeNotRetryable(String),
    #[error("no stopped run is available to continue")]
    NoContinuableRun,
    #[error("checkpoint workflow id does not match the current workflow")]
    CheckpointWorkflowMismatch,
}

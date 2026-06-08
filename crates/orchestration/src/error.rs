use crate::provider_config::ProviderConfigError;
use domain::{NodeId, WorkflowValidationError};
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
}

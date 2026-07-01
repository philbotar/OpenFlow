use engine::AgentError;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthoringError {
    #[error("authoring session not found")]
    SessionNotFound,
    #[error("authoring model attempted tool calls")]
    ModelToolCalls,
    #[error("{0}")]
    Agent(String),
    #[error("{0}")]
    MissingDraft(String),
    #[error("invalid workflowDraft: {0}")]
    InvalidDraft(String),
    #[error("layout failed: {0}")]
    LayoutFailed(String),
}

impl AuthoringError {
    #[must_use]
    pub fn is_session_not_found(&self) -> bool {
        matches!(self, Self::SessionNotFound)
    }
}

impl From<AgentError> for AuthoringError {
    fn from(error: AgentError) -> Self {
        Self::Agent(error.to_string())
    }
}

impl From<AuthoringError> for crate::error::BackendError {
    fn from(error: AuthoringError) -> Self {
        Self::AuthoringFailed(error.to_string())
    }
}

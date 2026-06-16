use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ToolError {
    #[error("[not_found] {what} — {hint}")]
    NotFound { what: String, hint: String },
    #[error("[permission_denied] {what} — {hint}")]
    PermissionDenied { what: String, hint: String },
    #[error("[invalid_args] {tool}: {problem} — {hint}")]
    InvalidArgs {
        tool: String,
        problem: String,
        hint: String,
    },
    #[error("[timeout] {tool} timed out after {after_secs}s — {hint}")]
    Timeout {
        tool: String,
        after_secs: u64,
        hint: String,
        partial_output: Option<String>,
    },
    #[error("[cancelled] {tool} was cancelled")]
    Cancelled { tool: String },
    #[error("[failed] {detail}")]
    ExecutionFailed {
        detail: String,
        hint: Option<String>,
    },
}

impl ToolError {
    /// ROADMAP T19: transient (`Timeout`, some `ExecutionFailed`) vs permanent.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Timeout { .. } => true,
            Self::ExecutionFailed { detail, hint } => {
                let combined =
                    format!("{detail} {}", hint.as_deref().unwrap_or_default()).to_lowercase();
                combined.contains("timeout")
                    || combined.contains("timed out")
                    || combined.contains("connection reset")
                    || combined.contains("connection refused")
                    || combined.contains("temporarily unavailable")
                    || combined.contains("503")
                    || combined.contains("502")
                    || combined.contains("429")
            }
            _ => false,
        }
    }

    /// Migration shim mapping to [`Self::ExecutionFailed`].
    #[must_use]
    pub fn failed(msg: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            detail: msg.into(),
            hint: None,
        }
    }

    /// Transient execution failure for adapters (retryable via [`Self::is_retryable`]).
    #[must_use]
    pub fn transient(detail: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            detail: detail.into(),
            hint: Some("transient".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_carries_partial_output() {
        let err = ToolError::Timeout {
            tool: "bash".to_string(),
            after_secs: 1,
            hint: "retry".to_string(),
            partial_output: Some("partial\n".to_string()),
        };
        match &err {
            ToolError::Timeout { partial_output, .. } => {
                assert_eq!(partial_output.as_deref(), Some("partial\n"));
            }
            _ => panic!("expected Timeout variant"),
        }
    }

    #[test]
    fn timeout_is_retryable() {
        assert!(ToolError::Timeout {
            tool: "bash".to_string(),
            after_secs: 300,
            hint: "increase timeout".to_string(),
            partial_output: None,
        }
        .is_retryable());
    }

    #[test]
    fn permanent_errors_are_not_retryable() {
        assert!(!ToolError::NotFound {
            what: "missing".to_string(),
            hint: "use find".to_string(),
        }
        .is_retryable());
        assert!(!ToolError::PermissionDenied {
            what: "denied".to_string(),
            hint: "use relative path".to_string(),
        }
        .is_retryable());
        assert!(!ToolError::InvalidArgs {
            tool: "read".to_string(),
            problem: "bad json".to_string(),
            hint: "path required".to_string(),
        }
        .is_retryable());
        assert!(!ToolError::Cancelled {
            tool: "bash".to_string(),
        }
        .is_retryable());
        assert!(!ToolError::failed("generic").is_retryable());
    }

    #[test]
    fn transient_execution_failed_is_retryable() {
        let err = ToolError::ExecutionFailed {
            detail: "connection reset".to_string(),
            hint: Some("transient".to_string()),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn permanent_execution_failed_is_not_retryable() {
        assert!(!ToolError::failed("file not found").is_retryable());
    }

    #[test]
    fn failed_shim_preserves_message_in_display() {
        let error = ToolError::failed("path escapes execution folder: ../x");
        assert!(error.to_string().contains("path escapes execution folder"));
    }

    #[test]
    fn runner_registry_error_is_not_retryable() {
        use crate::tool::registry::ToolRegistryError;
        use crate::tool::runner::ToolRunnerError;

        let err = ToolRunnerError::Registry(ToolRegistryError::Missing("nope".into()));
        assert!(!err.is_retryable());
    }

    #[test]
    fn runner_tool_timeout_is_retryable() {
        use crate::tool::runner::ToolRunnerError;

        let err = ToolRunnerError::Tool(ToolError::Timeout {
            tool: "bash".into(),
            after_secs: 1,
            hint: "retry".into(),
            partial_output: None,
        });
        assert!(err.is_retryable());
    }

    #[test]
    fn runner_invalid_arguments_is_not_retryable() {
        use crate::tool::runner::ToolRunnerError;

        let err = ToolRunnerError::InvalidArguments("bad json".into());
        assert!(!err.is_retryable());
    }
}

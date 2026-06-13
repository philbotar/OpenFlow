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
        matches!(self, Self::Timeout { .. })
    }

    /// Migration shim mapping to [`Self::ExecutionFailed`].
    #[must_use]
    pub fn failed(msg: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            detail: msg.into(),
            hint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_is_retryable() {
        assert!(ToolError::Timeout {
            tool: "bash".to_string(),
            after_secs: 300,
            hint: "increase timeout".to_string(),
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
    fn failed_shim_preserves_message_in_display() {
        let error = ToolError::failed("path escapes execution folder: ../x");
        assert!(error.to_string().contains("path escapes execution folder"));
    }
}

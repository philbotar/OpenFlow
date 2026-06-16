use crate::tool::errors::ToolError;
use crate::tool::runner::ToolRunnerError;
use engine::RetryPolicy;
use std::future::Future;
use tokio_util::sync::CancellationToken;

/// Execute `run_attempt` up to `policy.max_attempts` retries after the first failure.
/// Only retries when the error is retryable and the cancel token is not set.
pub async fn execute_with_retry<T, F, Fut>(
    policy: &RetryPolicy,
    cancel: &CancellationToken,
    mut on_retry: impl FnMut(u8, std::time::Duration),
    mut run_attempt: F,
) -> Result<T, ToolRunnerError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ToolRunnerError>>,
{
    let mut retry_count: u8 = 0;
    loop {
        if cancel.is_cancelled() {
            return Err(ToolRunnerError::Tool(ToolError::Cancelled {
                tool: "tool".to_string(),
            }));
        }
        match run_attempt().await {
            Ok(value) => return Ok(value),
            Err(error) if error.is_retryable() && retry_count < policy.max_attempts => {
                retry_count += 1;
                let delay = policy.delay_for_attempt(retry_count);
                on_retry(retry_count, delay);
                tokio::select! {
                    biased;
                    () = cancel.cancelled() => {
                        return Err(ToolRunnerError::Tool(ToolError::Cancelled {
                            tool: "tool".to_string(),
                        }));
                    }
                    () = tokio::time::sleep(delay) => {}
                }
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn retries_transient_errors_until_success() {
        let policy = RetryPolicy {
            max_attempts: 2,
            backoff_ms: 1,
        };
        let attempts = Arc::new(AtomicU8::new(0));
        let attempts_cloned = Arc::clone(&attempts);
        let cancel = CancellationToken::new();
        let mut retry_events = 0u8;

        let result = execute_with_retry(
            &policy,
            &cancel,
            |_, _| retry_events += 1,
            || {
                let attempts = Arc::clone(&attempts_cloned);
                async move {
                    let n = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if n < 3 {
                        Err(ToolRunnerError::Tool(ToolError::transient(
                            "connection reset",
                        )))
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await
        .expect("should succeed on third attempt");

        assert_eq!(result, 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(retry_events, 2);
    }

    #[tokio::test]
    async fn permanent_error_does_not_retry() {
        let policy = RetryPolicy::default();
        let attempts = Arc::new(AtomicU8::new(0));
        let cancel = CancellationToken::new();

        let error = execute_with_retry::<(), _, _>(
            &policy,
            &cancel,
            |_, _| {},
            || {
                let attempts = Arc::clone(&attempts);
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(ToolRunnerError::Tool(ToolError::NotFound {
                        what: "missing".into(),
                        hint: "use find".into(),
                    }))
                }
            },
        )
        .await
        .expect_err("permanent");

        assert!(error.to_string().contains("[not_found]"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}

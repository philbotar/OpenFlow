//! Shared retry backoff for model invocation failures.

use crate::graph::RetryPolicy;
use std::time::Duration;

/// Bump `retry_count` and return the backoff delay when another attempt is allowed.
pub(in crate::execution) fn next_retry(
    policy: &RetryPolicy,
    retry_count: &mut u8,
) -> Option<Duration> {
    if *retry_count >= policy.max_attempts {
        return None;
    }
    *retry_count += 1;
    Some(policy.delay_for_attempt(*retry_count))
}

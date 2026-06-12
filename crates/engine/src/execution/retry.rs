//! Shared retry backoff and telemetry for model invocation failures.

use crate::execution::{RunEvent, RunEventKind};
use crate::graph::{NodeId, RetryPolicy};
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

pub(in crate::execution) fn retrying_event(node_id: NodeId, delay: Duration) -> RunEvent {
    RunEvent {
        node_id,
        kind: RunEventKind::Retrying,
        message: format!(
            "retrying after transient failure; backoff_ms={}",
            delay.as_millis()
        ),
        output: None,
    }
}

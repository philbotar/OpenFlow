//! Per-node retry budgets and streaks kept by the interactive engine.
//!
//! The counter set is declared once in [`recovery_counters!`]; the engine fields,
//! checkpoint restore, stale-node-id validation, and retry reset all derive from
//! that list so a new counter cannot be forgotten in one of them.

use super::InteractiveEngineCheckpoint;
use crate::graph::NodeId;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type NodeCounter = BTreeMap<NodeId, u8>;

macro_rules! recovery_counters {
    (
        kept_on_retry: [$($kept:ident),* $(,)?],
        cleared_on_retry: [$($cleared:ident),* $(,)?] $(,)?
    ) => {
        /// Counter maps mirrored one-to-one by [`InteractiveEngineCheckpoint`] fields.
        #[derive(Debug, Default)]
        pub(super) struct RecoveryCounters {
            $(pub(super) $kept: NodeCounter,)*
            $(pub(super) $cleared: NodeCounter,)*
        }

        impl RecoveryCounters {
            /// Checkpoint field names backed by these counters.
            #[cfg(test)]
            pub(super) const CHECKPOINT_FIELDS: &'static [&'static str] =
                &[$(stringify!($kept),)* $(stringify!($cleared),)*];

            /// Move the counters out of a checkpoint that is being restored.
            pub(super) fn take_from(checkpoint: &mut InteractiveEngineCheckpoint) -> Self {
                Self {
                    $($kept: std::mem::take(&mut checkpoint.$kept),)*
                    $($cleared: std::mem::take(&mut checkpoint.$cleared),)*
                }
            }

            /// Collect every node id a checkpoint's counters reference.
            pub(super) fn extend_node_ids(
                checkpoint: &InteractiveEngineCheckpoint,
                ids: &mut BTreeSet<NodeId>,
            ) {
                $(ids.extend(checkpoint.$kept.keys().cloned());)*
                $(ids.extend(checkpoint.$cleared.keys().cloned());)*
            }

            /// Forget every budget and streak except the monotonic retry count.
            pub(super) fn clear_for_retry(&mut self, node_id: &NodeId) {
                $(self.$cleared.remove(node_id);)*
            }
        }
    };
}

recovery_counters! {
    // Monotonic so `model_attempt` only grows and adapters mint a fresh cancellation token.
    kept_on_retry: [retries_by_node],
    cleared_on_retry: [
        // Consecutive transient failures since the node's last successful turn.
        transient_streaks_by_node,
        submit_output_retries_by_node,
        request_input_retries_by_node,
        empty_turn_retries_by_node,
        mixed_tool_turn_retries_by_node,
        output_truncation_retries_by_node,
        // Consecutive auto-continued text-only turns for nodes that disallow user
        // input; reset whenever the node makes tool-call progress.
        auto_continue_streaks_by_node,
    ],
}

/// Spend one retry from `counter` unless the node already used `max`.
///
/// Returns `false` (without counting) when the budget is exhausted.
pub(super) fn try_spend(counter: &mut NodeCounter, node_id: &NodeId, max: u8) -> bool {
    let used = counter.entry(node_id.clone()).or_default();
    if *used >= max {
        return false;
    }
    *used += 1;
    true
}

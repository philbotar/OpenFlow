use super::InteractiveEngine;
use crate::conversation::AgentTranscriptItem;
use crate::execution::tool_results::{denied_tool_result, error_tool_result};
use crate::execution::EngineInputError;
use crate::graph::NodeId;
use crate::tools::ToolResult;
use std::collections::HashMap;

impl InteractiveEngine {
    /// # Errors
    /// Returns an error if no tool calls are pending or the wrong node id is provided.
    pub fn on_tool_results(
        &mut self,
        node_id: &NodeId,
        results: Vec<ToolResult>,
    ) -> Result<(), EngineInputError> {
        let approval_id = self
            .find_pending_tool_batch(node_id, false)
            .ok_or(EngineInputError::NoPendingTools)?;
        let pending_calls = self
            .pending_tool_batches
            .get(&approval_id)
            .map(|batch| batch.tool_calls.clone())
            .ok_or(EngineInputError::NoPendingTools)?;
        let mut by_id: HashMap<String, ToolResult> = results
            .into_iter()
            .map(|result| (result.tool_call_id.clone(), result))
            .collect();
        let transcript = self.transcripts.entry(node_id.clone()).or_default();
        for call in &pending_calls {
            let result = by_id.remove(&call.id).unwrap_or_else(|| {
                error_tool_result(
                    call,
                    "tool execution did not complete (interrupted or cancelled)",
                )
            });
            transcript.push(AgentTranscriptItem::ToolResult { result });
        }
        self.pending_tool_batches.remove(&approval_id);
        Ok(())
    }

    /// # Errors
    /// Returns an error if no matching approval batch is awaiting a decision.
    pub fn on_tool_decision(
        &mut self,
        approval_id: &str,
        allow: bool,
        reason: Option<&str>,
    ) -> Result<(), EngineInputError> {
        let pending = self
            .pending_tool_batches
            .get_mut(approval_id)
            .ok_or_else(|| EngineInputError::UnknownApproval(approval_id.to_string()))?;
        if !pending.requires_approval {
            return Err(EngineInputError::NoPendingTools);
        }
        if allow {
            pending.requires_approval = false;
            return Ok(());
        }

        let node_id = pending.node_id.clone();
        let denied = pending
            .tool_calls
            .iter()
            .map(|call| AgentTranscriptItem::ToolResult {
                result: denied_tool_result(call, reason),
            })
            .collect::<Vec<_>>();
        self.transcripts.entry(node_id).or_default().extend(denied);
        self.pending_tool_batches.remove(approval_id);
        Ok(())
    }

    fn find_pending_tool_batch(&self, node_id: &NodeId, requires_approval: bool) -> Option<String> {
        self.pending_tool_batches
            .iter()
            .find(|(_, batch)| {
                batch.node_id == *node_id && batch.requires_approval == requires_approval
            })
            .map(|(approval_id, _)| approval_id.clone())
    }
}
